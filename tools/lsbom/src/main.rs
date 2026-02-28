use anyhow::{Context, Result};
use std::collections::HashMap;
use std::env;
use std::fs;
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
    input: Option<String>,
}

fn parse_print_format(s: &str) -> Option<Vec<PrintItem>> {
    let mut format = Vec::new();
    for c in s.chars() {
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
            _ => return None,
        }
    }
    Some(format)
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
        input: None,
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
        } else if arg.is_empty() || !arg.starts_with('-') {
            opts.input = Some(arg.clone());
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
                            opts.print_format = parse_print_format(&remaining);
                            if opts.print_format.is_none() {
                                help(Some(&format!("invalid print format {remaining}")));
                            }
                            j = chars.len();
                            continue;
                        } else {
                            i += 1;
                            if i < args.len() {
                                opts.print_format = parse_print_format(&args[i]);
                                if opts.print_format.is_none() {
                                    help(Some(&format!("invalid print format {}", args[i])));
                                }
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

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let options = parse_options(&args);

    if options.help {
        help(None);
    }

    let input = match &options.input {
        Some(i) => i.clone(),
        None => help(Some("input is required")),
    };

    if options.arch.is_some() {
        eprintln!("warning: arch argument not yet implemented");
    }

    let include_all = !options.include_block_devices
        && !options.include_character_devices
        && !options.include_directories
        && !options.include_files
        && !options.include_symbolic_links;

    let data = fs::read(&input).with_context(|| format!("failed to read {input}"))?;
    let bom = Bom::load(data).with_context(|| "failed to load BOM")?;

    let entries = bom
        .tree_entries("Paths")
        .with_context(|| "failed to load paths tree")?;

    // Map from id -> (parent, name)
    let mut files: HashMap<u32, (u32, String)> = HashMap::new();

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

        // Build full path
        let path = xcbuild_bom::paths::resolve_path(&file_key, &files);

        if options.only_path {
            println!("{path}");
        } else {
            let mode = format!("{:o}", path_info_2.mode);
            let uid = path_info_2.user.to_string();
            let gid = path_info_2.group.to_string();
            print!("{path}\t{mode}\t{uid}/{gid}");

            if path_info_2.path_type() == PathType::File {
                let size = path_info_2.size.to_string();
                let checksum = path_info_2.checksum.to_string();
                print!("\t{size}\t{checksum}");
            }

            println!();
        }
    }

    Ok(())
}
