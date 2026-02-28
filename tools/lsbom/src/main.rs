use anyhow::{Context, Result};
use std::collections::HashMap;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Write};
use xcbuild_bom::paths::{FileKey, PathInfo1, PathInfo2, PathType};
use xcbuild_bom::Bom;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrintItem {
    Checksum,
    FileName,
    FileNameQuoted,
    GroupID,
    GroupName,
    Permissions,
    PermissionsText,
    FileSize,
    FileSizeFormatted,
    ModificationTime,
    ModificationTimeFormatted,
    UserID,
    UserName,
    UserGroupID,
    UserGroupName,
}

struct Options {
    help: bool,
    include_block_devices: bool,
    include_character_devices: bool,
    include_directories: bool,
    include_files: bool,
    include_symbolic_links: bool,
    print_mtime: bool,
    only_path: bool,
    no_modes: bool,
    print_format: Option<Vec<PrintItem>>,
    arch: Option<String>,
    inputs: Vec<String>,
}

/// Resolves UIDs to user names and GIDs to group names by reading /etc/passwd and /etc/group.
struct NameLookup {
    users: HashMap<u32, String>,
    groups: HashMap<u32, String>,
}

impl NameLookup {
    fn new() -> Self {
        let mut lookup = NameLookup {
            users: HashMap::new(),
            groups: HashMap::new(),
        };
        lookup.load_passwd();
        lookup.load_group();
        lookup
    }

    fn load_passwd(&mut self) {
        if let Ok(contents) = fs::read_to_string("/etc/passwd") {
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 {
                    if let Ok(uid) = parts[2].parse::<u32>() {
                        self.users.insert(uid, parts[0].to_string());
                    }
                }
            }
        }
    }

    fn load_group(&mut self) {
        if let Ok(contents) = fs::read_to_string("/etc/group") {
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 {
                    if let Ok(gid) = parts[2].parse::<u32>() {
                        self.groups.insert(gid, parts[0].to_string());
                    }
                }
            }
        }
    }

    fn resolve_user(&self, uid: u32) -> String {
        match self.users.get(&uid) {
            Some(name) => name.clone(),
            None => uid.to_string(),
        }
    }

    fn resolve_group(&self, gid: u32) -> String {
        match self.groups.get(&gid) {
            Some(name) => name.clone(),
            None => gid.to_string(),
        }
    }
}

fn parse_print_format(s: &str) -> Result<Vec<PrintItem>, String> {
    let mut format = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for c in s.chars() {
        if !seen.insert(c) {
            return Err(format!("duplicate format character '{c}'"));
        }
        match c {
            'c' => format.push(PrintItem::Checksum),
            'f' => format.push(PrintItem::FileName),
            'F' => format.push(PrintItem::FileNameQuoted),
            'g' => format.push(PrintItem::GroupID),
            'G' => format.push(PrintItem::GroupName),
            'm' => format.push(PrintItem::Permissions),
            'M' => format.push(PrintItem::PermissionsText),
            's' => format.push(PrintItem::FileSize),
            'S' => format.push(PrintItem::FileSizeFormatted),
            't' => format.push(PrintItem::ModificationTime),
            'T' => format.push(PrintItem::ModificationTimeFormatted),
            'u' => format.push(PrintItem::UserID),
            'U' => format.push(PrintItem::UserName),
            '/' => format.push(PrintItem::UserGroupID),
            '?' => format.push(PrintItem::UserGroupName),
            _ => return Err(format!("invalid print format character '{c}'")),
        }
    }
    Ok(format)
}

fn help(error: Option<&str>) -> ! {
    if let Some(e) = error {
        eprintln!("error: {e}\n");
    }

    eprintln!("Usage: lsbom [options] [inputs]\n");
    eprintln!("List the contents of a BOM archive.\n");
    eprintln!("  -h, --help (this message)\n");
    eprintln!("Options:");
    eprintln!("  -m\t  print modification times");
    eprintln!("  -s\t  print only paths");
    eprintln!("  -x\t  print no modes");
    eprintln!("  --arch [arch]");
    eprintln!("  -p [flags]\n");
    eprintln!("Print flags:");
    eprintln!("  c\t  print checksum");
    eprintln!("  f\t  print file name");
    eprintln!("  F\t  print file name (quoted)");
    eprintln!("  g\t  print group id");
    eprintln!("  G\t  print group name");
    eprintln!("  m\t  print permissions");
    eprintln!("  M\t  print permissions (text)");
    eprintln!("  s\t  print file size");
    eprintln!("  S\t  print file size (formatted)");
    eprintln!("  t\t  print modification time");
    eprintln!("  T\t  print modification time (formatted)");
    eprintln!("  u\t  print user id");
    eprintln!("  U\t  print user name");
    eprintln!("  /\t  print user/group id");
    eprintln!("  ?\t  print user/group name\n");
    eprintln!("Include:");
    eprintln!("  -b\t  include block devices");
    eprintln!("  -c\t  include character devices");
    eprintln!("  -d\t  include directories");
    eprintln!("  -f\t  include files");
    eprintln!("  -l\t  include symbolic links\n");

    std::process::exit(if error.is_some() { 1 } else { 0 });
}

fn parse_options(args: &[String]) -> Options {
    let mut opts = Options {
        help: false,
        include_block_devices: false,
        include_character_devices: false,
        include_directories: false,
        include_files: false,
        include_symbolic_links: false,
        print_mtime: false,
        only_path: false,
        no_modes: false,
        print_format: None,
        arch: None,
        inputs: Vec::new(),
    };

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        if arg == "--help" {
            opts.help = true;
        } else if arg == "--arch" {
            i += 1;
            if i < args.len() {
                opts.arch = Some(args[i].clone());
            }
        } else if let Some(value) = arg.strip_prefix("--arch=") {
            opts.arch = Some(value.to_string());
        } else if arg.is_empty() || !arg.starts_with('-') {
            opts.inputs.push(arg.clone());
        } else {
            let chars: Vec<char> = arg.chars().skip(1).collect();
            let mut j = 0;
            while j < chars.len() {
                match chars[j] {
                    'h' => opts.help = true,
                    'b' => opts.include_block_devices = true,
                    'c' => opts.include_character_devices = true,
                    'd' => opts.include_directories = true,
                    'f' => opts.include_files = true,
                    'l' => opts.include_symbolic_links = true,
                    'm' => opts.print_mtime = true,
                    's' => opts.only_path = true,
                    'x' => opts.no_modes = true,
                    'p' => {
                        if j + 1 < chars.len() {
                            let remaining: String = chars[j + 1..].iter().collect();
                            match parse_print_format(&remaining) {
                                Ok(fmt) => opts.print_format = Some(fmt),
                                Err(e) => help(Some(&e)),
                            }
                            j = chars.len();
                            continue;
                        } else {
                            i += 1;
                            if i < args.len() {
                                match parse_print_format(&args[i]) {
                                    Ok(fmt) => opts.print_format = Some(fmt),
                                    Err(e) => help(Some(&e)),
                                }
                            } else {
                                help(Some("-p requires a format argument"));
                            }
                        }
                    }
                    _ => help(Some(&format!("unknown argument {arg}"))),
                }
                j += 1;
            }
        }
        i += 1;
    }

    opts
}

fn format_permissions_text(mode: u16, path_type: PathType) -> String {
    let mut s = String::with_capacity(10);
    s.push(match path_type {
        PathType::Directory => 'd',
        PathType::Link => 'l',
        PathType::Device => {
            // Check if it's a character device (mode & 0o020000 for char special)
            // or block device. For BOM, block devices typically have the directory bit set.
            if mode & 0x4000 != 0 {
                'b'
            } else {
                'c'
            }
        }
        PathType::File => {
            // Check for socket
            if mode & 0xF000 == 0xC000 {
                's'
            } else {
                '-'
            }
        }
    });

    // Owner read/write
    s.push(if mode & 0o400 != 0 { 'r' } else { '-' });
    s.push(if mode & 0o200 != 0 { 'w' } else { '-' });

    // Owner execute with setuid
    if mode & 0o4000 != 0 {
        s.push(if mode & 0o100 != 0 { 's' } else { 'S' });
    } else {
        s.push(if mode & 0o100 != 0 { 'x' } else { '-' });
    }

    // Group read/write
    s.push(if mode & 0o040 != 0 { 'r' } else { '-' });
    s.push(if mode & 0o020 != 0 { 'w' } else { '-' });

    // Group execute with setgid
    if mode & 0o2000 != 0 {
        s.push(if mode & 0o010 != 0 { 's' } else { 'S' });
    } else {
        s.push(if mode & 0o010 != 0 { 'x' } else { '-' });
    }

    // Other read/write
    s.push(if mode & 0o004 != 0 { 'r' } else { '-' });
    s.push(if mode & 0o002 != 0 { 'w' } else { '-' });

    // Other execute with sticky bit
    if mode & 0o1000 != 0 {
        s.push(if mode & 0o001 != 0 { 't' } else { 'T' });
    } else {
        s.push(if mode & 0o001 != 0 { 'x' } else { '-' });
    }

    s
}

fn format_size_human(size: u32) -> String {
    if size < 1024 {
        format!("{size}")
    } else if size < 1024 * 1024 {
        format!("{:.1}K", size as f64 / 1024.0)
    } else if size < 1024 * 1024 * 1024 {
        format!("{:.1}M", size as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}G", size as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_timestamp(epoch: u32) -> String {
    unsafe {
        let time = epoch as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&time, &mut tm).is_null() {
            return epoch.to_string();
        }
        let mut buf = [0u8; 64];
        let fmt = b"%a %b %e %H:%M:%S %Y\0";
        let len = libc::strftime(
            buf.as_mut_ptr() as *mut libc::c_char,
            buf.len(),
            fmt.as_ptr() as *const libc::c_char,
            &tm,
        );
        if len == 0 {
            return epoch.to_string();
        }
        String::from_utf8_lossy(&buf[..len]).to_string()
    }
}

/// Write a line to stdout, handling BrokenPipe gracefully.
fn write_line(output: &str) {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    if writeln!(handle, "{output}").is_err() {
        // BrokenPipe or other write error - exit cleanly
        std::process::exit(0);
    }
}

/// Write a partial line to stdout, handling BrokenPipe gracefully.
fn write_str(output: &str) {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    if write!(handle, "{output}").is_err() {
        std::process::exit(0);
    }
}

fn print_entry(
    path: &str,
    path_info_2: &PathInfo2,
    format: &[PrintItem],
    name_lookup: &NameLookup,
) {
    let mut output = String::new();
    let pt = path_info_2.path_type();
    let is_file_or_link = pt == PathType::File || pt == PathType::Link;

    for (i, item) in format.iter().enumerate() {
        if i > 0 {
            output.push('\t');
        }
        match item {
            PrintItem::FileName => output.push_str(path),
            PrintItem::FileNameQuoted => {
                output.push('"');
                // Escape embedded double quotes with backslash
                for ch in path.chars() {
                    if ch == '"' {
                        output.push('\\');
                    }
                    output.push(ch);
                }
                output.push('"');
            }
            PrintItem::Checksum => {
                if is_file_or_link {
                    let _ = write!(output, "{}", path_info_2.checksum);
                }
            }
            PrintItem::GroupID => {
                let _ = write!(output, "{}", path_info_2.group);
            }
            PrintItem::GroupName => {
                output.push_str(&name_lookup.resolve_group(path_info_2.group));
            }
            PrintItem::Permissions => {
                let _ = write!(output, "{:o}", path_info_2.mode);
            }
            PrintItem::PermissionsText => {
                output.push_str(&format_permissions_text(
                    path_info_2.mode,
                    path_info_2.path_type(),
                ));
            }
            PrintItem::FileSize => {
                if is_file_or_link {
                    let _ = write!(output, "{}", path_info_2.size);
                }
            }
            PrintItem::FileSizeFormatted => {
                if is_file_or_link {
                    output.push_str(&format_size_human(path_info_2.size));
                }
            }
            PrintItem::ModificationTime => {
                if is_file_or_link {
                    let _ = write!(output, "{}", path_info_2.modtime);
                }
            }
            PrintItem::ModificationTimeFormatted => {
                if is_file_or_link {
                    output.push_str(&format_timestamp(path_info_2.modtime));
                }
            }
            PrintItem::UserID => {
                let _ = write!(output, "{}", path_info_2.user);
            }
            PrintItem::UserName => {
                output.push_str(&name_lookup.resolve_user(path_info_2.user));
            }
            PrintItem::UserGroupID => {
                let _ = write!(output, "{}/{}", path_info_2.user, path_info_2.group);
            }
            PrintItem::UserGroupName => {
                let _ = write!(
                    output,
                    "{}/{}",
                    name_lookup.resolve_user(path_info_2.user),
                    name_lookup.resolve_group(path_info_2.group)
                );
            }
        }
    }
    write_line(&output);
}

fn arch_to_cpu_type(arch: &str) -> Option<u16> {
    // BOM stores architecture as a u16, using compact Mach-O CPU type values.
    // The full Mach-O constants are 32-bit but BOM truncates to u16.
    match arch {
        "i386" => Some(0x07),
        "x86_64" => Some(0x07), // 0x01000007 truncated
        "arm" | "armv7" | "armv7s" | "armv7k" => Some(0x0C),
        "arm64" | "arm64e" => Some(0x0C), // 0x0100000C truncated
        "ppc" => Some(0x12),
        "ppc64" => Some(0x12), // 0x01000012 truncated
        _ => None,
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let options = parse_options(&args);

    if options.help {
        help(None);
    }

    if options.inputs.is_empty() {
        help(Some("input is required"));
    }

    let include_all = !options.include_block_devices
        && !options.include_character_devices
        && !options.include_directories
        && !options.include_files
        && !options.include_symbolic_links;

    // Build name lookup for user/group resolution
    let name_lookup = NameLookup::new();

    for input in &options.inputs {
        let data = fs::read(input).with_context(|| format!("failed to read {input}"))?;
        let bom = Bom::load(data).with_context(|| "failed to load BOM")?;

        let entries = bom
            .tree_entries("Paths")
            .with_context(|| "failed to load paths tree")?;

        // Map from id -> (parent, name)
        let mut files: HashMap<u32, (u32, String)> = HashMap::new();

        // Collect all entries into a vec for sorting
        let mut collected: Vec<(String, PathInfo2)> = Vec::new();

        for entry in &entries {
            let file_key = match FileKey::from_bytes(&entry.key) {
                Some(fk) => fk,
                None => continue,
            };
            let path_info_1 = match PathInfo1::from_bytes(&entry.value) {
                Some(pi) => pi,
                None => continue,
            };

            files.insert(path_info_1.id, (file_key.parent, file_key.name.clone()));

            // Get secondary path info
            let path_info_2_data = match bom.index_get(path_info_1.index) {
                Some(d) => d,
                None => {
                    eprintln!("error: failed to get secondary path info");
                    continue;
                }
            };

            let path_info_2 = match PathInfo2::from_bytes(path_info_2_data) {
                Some(pi) => pi,
                None => continue,
            };

            // Filter by type
            if !include_all {
                match path_info_2.path_type() {
                    PathType::File => {
                        if !options.include_files {
                            continue;
                        }
                    }
                    PathType::Directory => {
                        if !options.include_directories {
                            continue;
                        }
                    }
                    PathType::Link => {
                        if !options.include_symbolic_links {
                            continue;
                        }
                    }
                    PathType::Device => {
                        if path_info_2.mode & 0x4000 != 0 {
                            if !options.include_block_devices {
                                continue;
                            }
                        } else if !options.include_character_devices {
                            continue;
                        }
                    }
                }
            }

            // Filter by architecture
            if let Some(ref arch_name) = options.arch {
                if path_info_2.path_type() == PathType::File {
                    if let Some(cpu_type) = arch_to_cpu_type(arch_name) {
                        if path_info_2.architecture != 0 && path_info_2.architecture != cpu_type {
                            continue;
                        }
                    }
                }
            }

            // Build full path
            let path = xcbuild_bom::paths::resolve_path(&file_key, &files);

            collected.push((path, path_info_2));
        }

        // Sort entries alphabetically by path
        collected.sort_by(|a, b| a.0.cmp(&b.0));

        // Print sorted entries
        for (path, path_info_2) in &collected {
            if let Some(ref format) = options.print_format {
                print_entry(path, path_info_2, format, &name_lookup);
            } else if options.only_path {
                write_line(path);
            } else {
                write_str(path);

                let pt = path_info_2.path_type();

                // no_modes only suppresses modes for Directory and Link entries
                let suppress_modes =
                    options.no_modes && (pt == PathType::Directory || pt == PathType::Link);

                if !suppress_modes {
                    let mode = format!("{:o}", path_info_2.mode);
                    let uid = path_info_2.user.to_string();
                    let gid = path_info_2.group.to_string();
                    write_str(&format!("\t{mode}\t{uid}/{gid}"));
                }

                match pt {
                    PathType::File => {
                        let size = path_info_2.size.to_string();
                        let checksum = path_info_2.checksum.to_string();
                        write_str(&format!("\t{size}\t{checksum}"));
                    }
                    PathType::Link => {
                        let size = path_info_2.size.to_string();
                        let checksum = path_info_2.checksum.to_string();
                        let link_target = &path_info_2.link_name;
                        write_str(&format!("\t{size}\t{checksum}\t{link_target}"));
                    }
                    PathType::Device => {
                        let dev = path_info_2.size; // device number stored in size field
                        write_str(&format!("\t{dev}"));
                    }
                    PathType::Directory => {}
                }

                if options.print_mtime {
                    write_str(&format!("\t{}", path_info_2.modtime));
                }

                write_line("");
            }
        }
    }

    Ok(())
}
