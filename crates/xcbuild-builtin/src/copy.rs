use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Options for the builtin-copy tool.
pub struct CopyOptions {
    pub verbose: bool,
    pub preserve_hfs_data: bool,
    pub ignore_missing_inputs: bool,
    pub resolve_src_symlinks: bool,
    pub strip_debug_symbols: bool,
    pub output: Option<String>,
    pub inputs: Vec<String>,
    pub excludes: Vec<String>,
}

impl CopyOptions {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut opts = CopyOptions {
            verbose: false,
            preserve_hfs_data: false,
            ignore_missing_inputs: false,
            resolve_src_symlinks: false,
            strip_debug_symbols: false,
            output: None,
            inputs: Vec::new(),
            excludes: Vec::new(),
        };

        let mut positional = Vec::new();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--verbose" | "-v" | "-V" => opts.verbose = true,
                "--preserve-hfs-data" | "-preserve-hfs-data" => opts.preserve_hfs_data = true,
                "--ignore-missing-inputs" | "-ignore-missing-inputs" => opts.ignore_missing_inputs = true,
                "--resolve-src-symlinks" | "-resolve-src-symlinks" => opts.resolve_src_symlinks = true,
                "--strip-debug-symbols" | "-strip-debug-symbols" => opts.strip_debug_symbols = true,
                "--output" | "-o" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for --output".into());
                    }
                    opts.output = Some(args[i].clone());
                }
                "--exclude" | "-exclude" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for --exclude".into());
                    }
                    opts.excludes.push(args[i].clone());
                }
                "--strip-tool" | "-strip-tool" => {
                    i += 1; // skip value
                }
                "--bitcode-strip-tool" | "-bitcode-strip-tool" => {
                    i += 1; // skip value
                }
                "--bitcode-strip" | "-bitcode-strip" => {
                    i += 1; // skip value
                }
                _ => {
                    if args[i].starts_with('-') {
                        return Err(format!("unknown option: {}", args[i]));
                    }
                    positional.push(args[i].clone());
                }
            }
            i += 1;
        }

        // If no explicit --output was given, use the last positional arg as output
        if opts.output.is_none() && positional.len() >= 2 {
            opts.output = Some(positional.pop().unwrap());
            opts.inputs = positional;
        } else {
            opts.inputs = positional;
        }

        Ok(opts)
    }
}

fn copy_path(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_symlink() {
        let target = fs::read_link(src)
            .map_err(|e| format!("failed to read symlink {}: {}", src.display(), e))?;
        // Remove existing if any
        let _ = fs::remove_file(dst);
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, dst)
            .map_err(|e| format!("failed to create symlink {}: {}", dst.display(), e))?;
        #[cfg(not(unix))]
        fs::copy(src, dst)
            .map_err(|e| format!("failed to copy {}: {}", src.display(), e))?;
    } else if src.is_dir() {
        copy_dir_recursive(src, dst)?;
    } else {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create dir {}: {}", parent.display(), e))?;
        }
        fs::copy(src, dst)
            .map_err(|e| format!("failed to copy {} -> {}: {}", src.display(), dst.display(), e))?;
        // Make writable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = fs::metadata(dst) {
                let mut perms = metadata.permissions();
                let mode = perms.mode() | 0o200;
                perms.set_mode(mode);
                let _ = fs::set_permissions(dst, perms);
            }
        }
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst)
        .map_err(|e| format!("failed to create dir {}: {}", dst.display(), e))?;

    for entry in WalkDir::new(src).min_depth(1) {
        let entry = entry.map_err(|e| format!("walk error: {}", e))?;
        let relative = entry.path().strip_prefix(src).unwrap();
        let target = dst.join(relative);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)
                .map_err(|e| format!("failed to create dir {}: {}", target.display(), e))?;
        } else if entry.file_type().is_symlink() {
            let link_target = fs::read_link(entry.path())
                .map_err(|e| format!("read symlink: {}", e))?;
            let _ = fs::remove_file(&target);
            #[cfg(unix)]
            std::os::unix::fs::symlink(&link_target, &target)
                .map_err(|e| format!("create symlink: {}", e))?;
            #[cfg(not(unix))]
            fs::copy(entry.path(), &target)
                .map_err(|e| format!("copy: {}", e))?;
        } else {
            fs::copy(entry.path(), &target)
                .map_err(|e| format!("copy: {}", e))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = fs::metadata(&target) {
                    let mut perms = metadata.permissions();
                    let mode = perms.mode() | 0o200;
                    perms.set_mode(mode);
                    let _ = fs::set_permissions(&target, perms);
                }
            }
        }
    }
    Ok(())
}

/// Run the builtin-copy command.
pub fn run(args: &[String]) -> i32 {
    let opts = match CopyOptions::parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    let output = match &opts.output {
        Some(o) => o.clone(),
        None => {
            eprintln!("error: no output path provided");
            return 1;
        }
    };

    if opts.preserve_hfs_data {
        eprintln!("warning: preserve HFS data is not supported");
    }

    let output_path = Path::new(&output);
    if let Err(e) = fs::create_dir_all(output_path) {
        eprintln!("error: failed to create output dir: {e}");
        return 1;
    }

    for input in &opts.inputs {
        let src_original = Path::new(input);

        // Resolve symlinks if requested
        let src_buf;
        let src = if opts.resolve_src_symlinks {
            match fs::canonicalize(src_original) {
                Ok(resolved) => {
                    src_buf = resolved;
                    src_buf.as_path()
                }
                Err(_) => {
                    // If canonicalize fails (e.g., path doesn't exist), use original
                    src_buf = src_original.to_path_buf();
                    src_buf.as_path()
                }
            }
        } else {
            src_buf = src_original.to_path_buf();
            src_buf.as_path()
        };

        if !src.is_dir() && !src.exists() {
            if opts.ignore_missing_inputs {
                continue;
            } else {
                eprintln!("error: missing input '{input}'");
                return 1;
            }
        }

        if opts.verbose {
            println!("verbose: copying {input} -> {output}");
        }

        let file_name = src.file_name().unwrap_or_default();
        let dst = output_path.join(file_name);

        if let Err(e) = copy_path(src, &dst) {
            eprintln!("error: {e}");
            return 1;
        }
    }

    0
}
