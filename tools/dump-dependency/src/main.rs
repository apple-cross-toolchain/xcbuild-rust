use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::Path;
use xcbuild_dependency::*;

fn dump_dependency_info(info: &DependencyInfo) {
    println!("input:");
    for input in &info.inputs {
        println!("  {input}");
    }
    println!("output:");
    for output in &info.outputs {
        println!("  {output}");
    }
}

fn dump(path: &str) -> Result<()> {
    if Path::new(path).is_dir() {
        let dir_info = DirectoryDependencyInfo::from_directory(path)
            .with_context(|| format!("failed to read directory {path}"))?;
        println!("directory dependency info");
        println!("directory: {}", dir_info.directory);
        dump_dependency_info(&dir_info.dependency_info);
        return Ok(());
    }

    let contents = fs::read(path).with_context(|| format!("failed to open {path}"))?;

    if let Ok(binary_info) = BinaryDependencyInfo::deserialize(&contents) {
        println!("binary dependency info");
        println!("version: {}", binary_info.version);
        dump_dependency_info(&binary_info.dependency_info);
        println!("missing:");
        for missing in &binary_info.missing {
            println!("  {missing}");
        }
    } else if let Ok(makefile_info) =
        MakefileDependencyInfo::deserialize(&String::from_utf8_lossy(&contents))
    {
        println!("makefile dependency info");
        for dep_info in &makefile_info.dependency_info {
            dump_dependency_info(dep_info);
        }
    } else {
        anyhow::bail!("unknown dependency info type");
    }

    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("usage: dump_dependency <inputs...>");
        std::process::exit(1);
    }

    for input in &args {
        dump(input)?;
    }

    Ok(())
}
