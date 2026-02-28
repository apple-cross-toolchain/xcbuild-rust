use byteorder::{BigEndian, ByteOrder, LittleEndian, NativeEndian};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HMapError {
    #[error("file too small to contain header")]
    FileTooSmall,
    #[error("invalid magic number")]
    InvalidMagic,
    #[error("invalid version {0}")]
    InvalidVersion(u16),
    #[error("reserved field is not zero")]
    NonZeroReserved,
    #[error("strings offset {0} exceeds file size {1}")]
    StringsOffsetOutOfBounds(u32, usize),
    #[error("bucket data extends beyond file")]
    BucketsOutOfBounds,
    #[error("string offset {0} out of bounds")]
    StringOutOfBounds(u32),
}

const HMAP_HEADER_MAGIC: u32 = (b'h' as u32) << 24 | (b'm' as u32) << 16 | (b'a' as u32) << 8 | b'p' as u32;
const HMAP_HEADER_MAGIC_SWAPPED: u32 = (b'p' as u32) << 24 | (b'a' as u32) << 16 | (b'm' as u32) << 8 | b'h' as u32;
const HMAP_HEADER_VERSION: u16 = 1;
const HMAP_EMPTY_BUCKET_KEY: u32 = 0;

const HEADER_SIZE: usize = 24;
const BUCKET_SIZE: usize = 12;

#[derive(Debug, Clone)]
struct HMapBucket {
    key: u32,
    prefix: u32,
    suffix: u32,
}

#[derive(Debug, Clone)]
pub struct HeaderMap {
    num_entries: u32,
    num_buckets: u32,
    max_value_length: u32,
    buckets: Vec<HMapBucket>,
    strings: Vec<u8>,
    keys: HashSet<String>,
    offsets: HashMap<String, u32>,
    modified: bool,
}

fn hash_hmap_key(s: &str) -> u32 {
    let mut result: u32 = 0;
    for c in s.bytes() {
        result = result.wrapping_add((c.to_ascii_lowercase() as u32).wrapping_mul(13));
    }
    result
}

fn canonicalize_key(key: &str) -> String {
    key.to_ascii_lowercase()
}

fn get_string(strings: &[u8], offset: u32) -> Option<&str> {
    let offset = offset as usize;
    if offset >= strings.len() {
        return None;
    }
    let end = strings[offset..].iter().position(|&b| b == 0)?;
    std::str::from_utf8(&strings[offset..offset + end]).ok()
}

impl Default for HeaderMap {
    fn default() -> Self {
        Self::new()
    }
}

impl HeaderMap {
    pub fn new() -> Self {
        HeaderMap {
            num_entries: 0,
            num_buckets: 8,
            max_value_length: 0,
            buckets: vec![
                HMapBucket {
                    key: 0,
                    prefix: 0,
                    suffix: 0,
                };
                8
            ],
            strings: Vec::new(),
            keys: HashSet::new(),
            offsets: HashMap::new(),
            modified: false,
        }
    }

    pub fn read(data: &[u8]) -> Result<Self, HMapError> {
        if data.len() < HEADER_SIZE {
            return Err(HMapError::FileTooSmall);
        }

        let magic = BigEndian::read_u32(&data[0..4]);
        let swapped = magic == HMAP_HEADER_MAGIC_SWAPPED;

        if magic != HMAP_HEADER_MAGIC && !swapped {
            return Err(HMapError::InvalidMagic);
        }

        let (version, reserved, strings_offset, num_entries, num_buckets, max_value_length) =
            if swapped {
                (
                    LittleEndian::read_u16(&data[4..6]),
                    LittleEndian::read_u16(&data[6..8]),
                    LittleEndian::read_u32(&data[8..12]),
                    LittleEndian::read_u32(&data[12..16]),
                    LittleEndian::read_u32(&data[16..20]),
                    LittleEndian::read_u32(&data[20..24]),
                )
            } else {
                (
                    BigEndian::read_u16(&data[4..6]),
                    BigEndian::read_u16(&data[6..8]),
                    BigEndian::read_u32(&data[8..12]),
                    BigEndian::read_u32(&data[12..16]),
                    BigEndian::read_u32(&data[16..20]),
                    BigEndian::read_u32(&data[20..24]),
                )
            };

        if version != HMAP_HEADER_VERSION {
            return Err(HMapError::InvalidVersion(version));
        }
        if reserved != 0 {
            return Err(HMapError::NonZeroReserved);
        }
        if strings_offset as usize > data.len() {
            return Err(HMapError::StringsOffsetOutOfBounds(
                strings_offset,
                data.len(),
            ));
        }

        let buckets_size = (num_buckets as usize) * BUCKET_SIZE;
        if HEADER_SIZE + buckets_size > data.len() {
            return Err(HMapError::BucketsOutOfBounds);
        }

        let mut buckets = Vec::with_capacity(num_buckets as usize);
        for i in 0..num_buckets as usize {
            let base = HEADER_SIZE + i * BUCKET_SIZE;
            let (k, p, s) = if swapped {
                (
                    LittleEndian::read_u32(&data[base..base + 4]),
                    LittleEndian::read_u32(&data[base + 4..base + 8]),
                    LittleEndian::read_u32(&data[base + 8..base + 12]),
                )
            } else {
                (
                    BigEndian::read_u32(&data[base..base + 4]),
                    BigEndian::read_u32(&data[base + 4..base + 8]),
                    BigEndian::read_u32(&data[base + 8..base + 12]),
                )
            };
            buckets.push(HMapBucket {
                key: k,
                prefix: p,
                suffix: s,
            });
        }

        let strings = data[strings_offset as usize..].to_vec();

        let mut keys = HashSet::new();
        let mut offsets = HashMap::new();

        for bucket in &buckets {
            if bucket.key == HMAP_EMPTY_BUCKET_KEY {
                continue;
            }
            if bucket.key as usize >= strings.len()
                || bucket.suffix as usize >= strings.len()
                || bucket.prefix as usize >= strings.len()
            {
                continue;
            }

            if let Some(key_str) = get_string(&strings, bucket.key) {
                keys.insert(canonicalize_key(key_str));
                offsets.insert(key_str.to_string(), bucket.key);
            }
            if let Some(prefix_str) = get_string(&strings, bucket.prefix) {
                offsets.insert(prefix_str.to_string(), bucket.prefix);
            }
            if let Some(suffix_str) = get_string(&strings, bucket.suffix) {
                offsets.insert(suffix_str.to_string(), bucket.suffix);
            }
        }

        Ok(HeaderMap {
            num_entries,
            num_buckets,
            max_value_length,
            buckets,
            strings,
            keys,
            offsets,
            modified: false,
        })
    }

    pub fn write(&mut self) -> Vec<u8> {
        if self.modified {
            self.rehash(self.num_buckets);
        }

        let strings_offset = HEADER_SIZE + self.num_buckets as usize * BUCKET_SIZE;
        let total_size = strings_offset + self.strings.len();
        let mut buffer = vec![0u8; total_size];

        NativeEndian::write_u32(&mut buffer[0..4], HMAP_HEADER_MAGIC);
        NativeEndian::write_u16(&mut buffer[4..6], HMAP_HEADER_VERSION);
        NativeEndian::write_u16(&mut buffer[6..8], 0);
        NativeEndian::write_u32(&mut buffer[8..12], strings_offset as u32);
        NativeEndian::write_u32(&mut buffer[12..16], self.num_entries);
        NativeEndian::write_u32(&mut buffer[16..20], self.num_buckets);
        NativeEndian::write_u32(&mut buffer[20..24], self.max_value_length);

        for (i, bucket) in self.buckets.iter().enumerate() {
            let base = HEADER_SIZE + i * BUCKET_SIZE;
            NativeEndian::write_u32(&mut buffer[base..base + 4], bucket.key);
            NativeEndian::write_u32(&mut buffer[base + 4..base + 8], bucket.prefix);
            NativeEndian::write_u32(&mut buffer[base + 8..base + 12], bucket.suffix);
        }

        buffer[strings_offset..].copy_from_slice(&self.strings);

        buffer
    }

    pub fn add(&mut self, key: &str, prefix: &str, suffix: &str) -> bool {
        if key.is_empty() || prefix.is_empty() || suffix.is_empty() {
            return false;
        }

        if self.keys.contains(&canonicalize_key(key)) {
            return false;
        }

        let k_off = self.add_string(key);
        let p_off = self.add_string(prefix);
        let s_off = self.add_string(suffix);

        self.keys.insert(canonicalize_key(key));

        self.grow();

        let hash = hash_hmap_key(key) % self.num_buckets;
        self.set_bucket(hash, k_off, p_off, s_off, false);

        if key.len() as u32 > self.max_value_length {
            self.max_value_length = key.len() as u32;
        }

        self.modified = true;
        true
    }

    pub fn entries(&self) -> Vec<HMapEntry<'_>> {
        let mut result = Vec::new();
        for bucket in &self.buckets {
            if bucket.key == HMAP_EMPTY_BUCKET_KEY {
                continue;
            }
            if let (Some(key), Some(prefix), Some(suffix)) = (
                get_string(&self.strings, bucket.key),
                get_string(&self.strings, bucket.prefix),
                get_string(&self.strings, bucket.suffix),
            ) {
                result.push(HMapEntry {
                    key,
                    prefix,
                    suffix,
                });
            }
        }
        result
    }

    pub fn dump(&self) {
        eprintln!(
            "Num Entries = {} Num Buckets = {} Strings Offset = {:#x}",
            self.num_entries,
            self.num_buckets,
            HEADER_SIZE + self.num_buckets as usize * BUCKET_SIZE
        );

        for (n, bucket) in self.buckets.iter().enumerate() {
            if bucket.key == HMAP_EMPTY_BUCKET_KEY {
                continue;
            }

            if bucket.key as usize >= self.strings.len()
                || bucket.suffix as usize >= self.strings.len()
                || bucket.prefix as usize >= self.strings.len()
            {
                eprintln!("Bucket #{n}: broken");
            } else if let (Some(key), Some(prefix), Some(suffix)) = (
                get_string(&self.strings, bucket.key),
                get_string(&self.strings, bucket.prefix),
                get_string(&self.strings, bucket.suffix),
            ) {
                let hash = hash_hmap_key(key) % self.num_buckets;
                eprintln!(
                    "Bucket #{n}: [{hash}] Key = '{key}' Prefix = '{prefix}' Suffix = '{suffix}'"
                );
            }
        }
    }

    fn add_string(&mut self, s: &str) -> u32 {
        if let Some(&offset) = self.offsets.get(s) {
            return offset;
        }

        if self.strings.is_empty() {
            self.strings.push(0);
        }

        let offset = self.strings.len() as u32;
        self.strings.extend_from_slice(s.as_bytes());
        self.strings.push(0);

        self.offsets.insert(s.to_string(), offset);
        offset
    }

    fn grow(&mut self) {
        if self.num_entries + 1 >= (self.num_buckets * 3) / 4 {
            self.rehash(self.num_buckets * 2);
        }
    }

    fn rehash(&mut self, new_num_buckets: u32) {
        let old_buckets: Vec<HMapBucket> = std::mem::replace(
            &mut self.buckets,
            vec![
                HMapBucket {
                    key: 0,
                    prefix: 0,
                    suffix: 0,
                };
                new_num_buckets as usize
            ],
        );
        let old_num_buckets = self.num_buckets;
        self.num_buckets = new_num_buckets;
        self.num_entries = 0;

        let mut to_rehash: Vec<(u32, HMapBucket)> = Vec::new();
        for bucket in &old_buckets {
            if bucket.key == HMAP_EMPTY_BUCKET_KEY {
                continue;
            }
            if let Some(key_str) = get_string(&self.strings, bucket.key) {
                let hash = hash_hmap_key(key_str) % new_num_buckets;
                to_rehash.push((hash, bucket.clone()));
            }
        }
        drop(old_buckets);
        let _ = old_num_buckets;

        for (hash, bucket) in to_rehash {
            self.set_bucket(hash, bucket.key, bucket.prefix, bucket.suffix, true);
        }

        self.modified = false;
    }

    fn set_bucket(&mut self, hash: u32, key: u32, prefix: u32, suffix: u32, growing: bool) {
        let n = self.num_buckets as usize;
        for i in 0..n {
            let idx = (hash as usize + i) % n;
            if self.buckets[idx].key == HMAP_EMPTY_BUCKET_KEY {
                self.buckets[idx].key = key;
                self.buckets[idx].prefix = prefix;
                self.buckets[idx].suffix = suffix;
                if !growing {
                    self.num_entries += 1;
                }
                return;
            }
        }
    }
}

#[derive(Debug)]
pub struct HMapEntry<'a> {
    pub key: &'a str,
    pub prefix: &'a str,
    pub suffix: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let mut hmap = HeaderMap::new();
        assert!(hmap.add("Foo.h", "/usr/include/", "Foo.h"));
        assert!(hmap.add("Bar.h", "/usr/local/include/", "Bar.h"));
        assert!(!hmap.add("Foo.h", "/other/", "Foo.h")); // duplicate

        let data = hmap.write();
        let hmap2 = HeaderMap::read(&data).unwrap();

        let entries = hmap2.entries();
        assert_eq!(entries.len(), 2);

        let mut found = HashSet::new();
        for e in &entries {
            found.insert(e.key.to_string());
        }
        assert!(found.contains("Foo.h"));
        assert!(found.contains("Bar.h"));
    }

    #[test]
    fn test_hash() {
        assert_eq!(hash_hmap_key("Foo.h"), hash_hmap_key("foo.h"));
    }
}
