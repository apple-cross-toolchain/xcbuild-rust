use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, BufRead};
use std::os::unix::fs::MetadataExt;
use xcbuild_bom::paths::{FileKey, PathInfo1, PathInfo2};
use xcbuild_bom::BomWriter;

fn help(error: Option<&str>) -> ! {
    if let Some(e) = error {
        eprintln!("error: {e}\n");
    }

    eprintln!("Usage: mkbom [-s] [-i filelist] directory bom");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -s          create a simplified BOM with only paths");
    eprintln!("  -i filelist read entries from lsbom output instead of scanning a directory");
    eprintln!("  -h, --help  show this help");

    std::process::exit(if error.is_some() { 1 } else { 0 });
}

struct Entry {
    path: String,
    path_type: u8,
    mode: u16,
    user: u32,
    group: u32,
    size: u32,
    checksum: u32,
    modtime: u32,
}

fn crc32_of_file(path: &std::path::Path) -> u32 {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => return 0,
    };
    // Simple CRC32 (IEEE/ISO 3309)
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in &data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    crc ^ 0xFFFFFFFF
}

fn scan_directory(dir: &str, simplified: bool) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();

    // Add root entry
    let root_meta = fs::metadata(dir).with_context(|| format!("cannot stat {dir}"))?;
    entries.push(Entry {
        path: ".".to_string(),
        path_type: 2, // directory
        mode: (root_meta.mode() & 0o7777) as u16,
        user: root_meta.uid(),
        group: root_meta.gid(),
        size: 0,
        checksum: 0,
        modtime: root_meta.mtime() as u32,
    });

    for entry in walkdir::WalkDir::new(dir).min_depth(1).sort_by_file_name() {
        let entry = entry.with_context(|| "error walking directory")?;
        let rel_path = entry
            .path()
            .strip_prefix(dir)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .to_string();

        let meta = entry.metadata().with_context(|| {
            format!("cannot stat {}", entry.path().display())
        })?;

        let (path_type, size, checksum) = if meta.is_dir() {
            (2u8, 0u32, 0u32)
        } else if meta.is_symlink() {
            (3u8, 0u32, 0u32)
        } else {
            let size = meta.len() as u32;
            let cksum = if simplified {
                0
            } else {
                crc32_of_file(entry.path())
            };
            (1u8, size, cksum)
        };

        entries.push(Entry {
            path: format!("./{rel_path}"),
            path_type,
            mode: (meta.mode() & 0o7777) as u16,
            user: if simplified { 0 } else { meta.uid() },
            group: if simplified { 0 } else { meta.gid() },
            size,
            checksum,
            modtime: if simplified {
                0
            } else {
                meta.mtime() as u32
            },
        });
    }

    Ok(entries)
}

fn parse_filelist(filelist: &str) -> Result<Vec<Entry>> {
    let reader: Box<dyn BufRead> = if filelist == "-" {
        Box::new(io::BufReader::new(io::stdin()))
    } else {
        let file = fs::File::open(filelist)
            .with_context(|| format!("cannot open {filelist}"))?;
        Box::new(io::BufReader::new(file))
    };

    let mut entries = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // lsbom output format: path\tmode\tuid/gid[\tsize\tchecksum]
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.is_empty() {
            continue;
        }

        let path = parts[0].to_string();
        let mode: u16 = if parts.len() > 1 {
            u16::from_str_radix(parts[1], 8).unwrap_or(0)
        } else {
            0
        };

        let (user, group) = if parts.len() > 2 {
            let ug: Vec<&str> = parts[2].split('/').collect();
            let u: u32 = ug.first().and_then(|s| s.parse().ok()).unwrap_or(0);
            let g: u32 = ug.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            (u, g)
        } else {
            (0, 0)
        };

        let size: u32 = if parts.len() > 3 {
            parts[3].parse().unwrap_or(0)
        } else {
            0
        };

        let checksum: u32 = if parts.len() > 4 {
            parts[4].parse().unwrap_or(0)
        } else {
            0
        };

        // Determine type from mode
        let path_type = if mode & 0o40000 != 0 {
            2 // directory
        } else if mode & 0o120000 == 0o120000 {
            3 // symlink
        } else {
            1 // file
        };

        entries.push(Entry {
            path,
            path_type,
            mode: mode & 0o7777,
            user,
            group,
            size,
            checksum,
            modtime: 0,
        });
    }

    Ok(entries)
}

fn build_bom(entries: &[Entry]) -> Result<Vec<u8>> {
    let mut writer = BomWriter::new();

    // Assign IDs to each path component.
    // BOM uses a parent-child tree: each entry has an ID and a parent ID.
    let mut path_id_map: HashMap<String, u32> = HashMap::new();
    let mut next_id: u32 = 0;

    // Collect tree entries (FileKey -> PathInfo1)
    let mut tree_entries: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();

    for entry in entries {
        // Determine parent path and file name
        let path = &entry.path;
        let (parent_path, name) = if path == "." {
            (String::new(), ".".to_string())
        } else if let Some(pos) = path.rfind('/') {
            let parent = &path[..pos];
            let name = &path[pos + 1..];
            if parent.is_empty() {
                (".".to_string(), name.to_string())
            } else {
                (parent.to_string(), name.to_string())
            }
        } else {
            (".".to_string(), path.clone())
        };

        let my_id = next_id;
        next_id += 1;
        path_id_map.insert(path.clone(), my_id);

        let parent_id = if path == "." {
            0 // root has no real parent
        } else {
            *path_id_map.get(&parent_path).unwrap_or(&0)
        };

        // Create PathInfo2 and store as a block
        let info2 = PathInfo2 {
            path_type: entry.path_type,
            architecture: 0,
            mode: entry.mode,
            user: entry.user,
            group: entry.group,
            modtime: entry.modtime,
            size: entry.size,
            checksum: entry.checksum,
            link_name: String::new(),
        };
        let info2_idx = writer.add_block(info2.to_bytes());

        let info1 = PathInfo1 {
            id: my_id,
            index: info2_idx,
        };

        let file_key = FileKey {
            parent: parent_id,
            name,
        };

        tree_entries.push((file_key.to_bytes(), info1.to_bytes()));
    }

    let tree_idx = writer.build_tree(&tree_entries);
    writer.add_variable("Paths", tree_idx);

    // Add empty trees for standard BOM variables
    let empty_tree_idx = writer.build_tree(&[]);
    writer.add_variable("HLIndex", empty_tree_idx);

    let empty_tree_idx2 = writer.build_tree(&[]);
    writer.add_variable("Size64", empty_tree_idx2);

    // VIndex - store a simple block with entry count
    let vindex_data = (entries.len() as u32).to_be_bytes().to_vec();
    let vindex_idx = writer.add_block(vindex_data);
    writer.add_variable("VIndex", vindex_idx);

    Ok(writer.serialize())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    let mut simplified = false;
    let mut filelist: Option<String> = None;
    let mut positional: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => help(None),
            "-s" => simplified = true,
            "-i" => {
                i += 1;
                if i >= args.len() {
                    help(Some("missing argument for -i"));
                }
                filelist = Some(args[i].clone());
            }
            _ => {
                if args[i].starts_with('-') {
                    help(Some(&format!("unknown argument {}", args[i])));
                }
                positional.push(args[i].clone());
            }
        }
        i += 1;
    }

    if filelist.is_some() {
        // mkbom [-s] -i filelist bom
        if positional.len() != 1 {
            help(Some("expected: mkbom [-s] -i filelist bom"));
        }
        let bom_path = &positional[0];
        let entries = parse_filelist(filelist.as_ref().unwrap())?;
        if entries.is_empty() {
            bail!("no entries found in filelist");
        }
        let data = build_bom(&entries)?;
        fs::write(bom_path, data).with_context(|| format!("failed to write {bom_path}"))?;
    } else {
        // mkbom [-s] directory bom
        if positional.len() != 2 {
            help(Some("expected: mkbom [-s] directory bom"));
        }
        let dir = &positional[0];
        let bom_path = &positional[1];
        let entries = scan_directory(dir, simplified)?;
        if entries.is_empty() {
            bail!("no entries found in directory");
        }
        let data = build_bom(&entries)?;
        fs::write(bom_path, data).with_context(|| format!("failed to write {bom_path}"))?;
    }

    Ok(())
}
