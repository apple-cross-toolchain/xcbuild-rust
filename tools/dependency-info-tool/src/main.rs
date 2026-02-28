use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::Path;
use xcbuild_dependency::*;

fn help(error: Option<&str>) -> ! {
    if let Some(e) = error {
        eprintln!("error: {e}\n");
    }

    eprintln!("Usage: dependency-info-tool [options] [inputs]\n");
    eprintln!("Converts dependency info to Ninja format.\n");
    eprintln!("Information:");
    eprintln!("  -h, --help");
    eprintln!("  -v, --version\n");
    eprintln!("Conversion Options:");
    eprintln!("  -o, --output");
    eprintln!("  -n, --name\n");
    eprintln!("Inputs:");
    eprintln!("  <format>:<path>");
    eprintln!("  format: makefile, binary, directory\n");

    std::process::exit(if error.is_some() { 1 } else { 0 });
}

fn load_dependency_info(
    path: &str,
    format: &DependencyInfoFormat,
) -> Result<Vec<DependencyInfo>> {
    match format {
        DependencyInfoFormat::Binary => {
            let contents = fs::read(path).with_context(|| format!("failed to open {path}"))?;
            let binary_info = BinaryDependencyInfo::deserialize(&contents)
                .with_context(|| "invalid binary dependency info")?;
            Ok(vec![binary_info.dependency_info])
        }
        DependencyInfoFormat::Directory => {
            if !Path::new(path).is_dir() {
                eprintln!("warning: ignoring non-directory {path}");
                return Ok(vec![]);
            }
            let dir_info = DirectoryDependencyInfo::from_directory(path)
                .with_context(|| "invalid directory")?;
            Ok(vec![dir_info.dependency_info])
        }
        DependencyInfoFormat::Makefile => {
            let contents = fs::read_to_string(path).with_context(|| format!("failed to open {path}"))?;
            let makefile_info = MakefileDependencyInfo::deserialize(&contents)
                .with_context(|| "invalid makefile dependency info")?;
            Ok(makefile_info.dependency_info)
        }
    }
}

fn resolve_relative_path(path: &str, base: &str) -> String {
    let p = Path::new(path);
    if p.is_absolute() {
        path.to_string()
    } else {
        Path::new(base).join(path).to_string_lossy().to_string()
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    let mut inputs: Vec<(DependencyInfoFormat, String)> = Vec::new();
    let mut output: Option<String> = None;
    let mut name: Option<String> = None;
    let mut show_help = false;
    let mut show_version = false;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-h" | "--help" => show_help = true,
            "-v" | "--version" => show_version = true,
            "-o" | "--output" => {
                i += 1;
                if i >= args.len() {
                    help(Some("missing value for -o"));
                }
                output = Some(args[i].clone());
            }
            "-n" | "--name" => {
                i += 1;
                if i >= args.len() {
                    help(Some("missing value for -n"));
                }
                name = Some(args[i].clone());
            }
            _ => {
                if arg.starts_with('-') {
                    help(Some(&format!("unknown argument {arg}")));
                }
                if let Some(colon) = arg.find(':') {
                    if colon == 0 || colon == arg.len() - 1 {
                        help(Some(&format!(
                            "unknown input {arg} (use format:/path/to/input)"
                        )));
                    }
                    let fmt_name = &arg[..colon];
                    let path = &arg[colon + 1..];
                    let format = DependencyInfoFormat::parse(fmt_name)
                        .map_err(|_| anyhow::anyhow!("unknown format {fmt_name}"))?;
                    inputs.push((format, path.to_string()));
                } else {
                    help(Some(&format!(
                        "unknown input {arg} (use format:/path/to/input)"
                    )));
                }
            }
        }
        i += 1;
    }

    if show_help {
        help(None);
    }
    if show_version {
        println!("dependency-info-tool version 1");
        return Ok(());
    }

    if inputs.is_empty() || output.is_none() || name.is_none() {
        help(Some("missing option(s)"));
    }

    let output = output.unwrap();
    let name = name.unwrap();

    let current_dir = env::current_dir()?.to_string_lossy().to_string();

    let mut all_inputs = Vec::new();
    for (format, path) in &inputs {
        let info = load_dependency_info(path, format)?;
        for dep_info in &info {
            all_inputs.extend(dep_info.inputs.iter().cloned());
        }
    }

    // Normalize paths
    let normalized_inputs: Vec<String> = all_inputs
        .iter()
        .map(|input| resolve_relative_path(input, &current_dir))
        .collect();

    // Serialize as makefile format
    let dep_info = DependencyInfo {
        outputs: vec![name],
        inputs: normalized_inputs,
    };
    let makefile_info = MakefileDependencyInfo {
        dependency_info: vec![dep_info],
    };
    let contents = makefile_info.serialize();

    fs::write(&output, contents).with_context(|| format!("failed to write {output}"))?;

    Ok(())
}
