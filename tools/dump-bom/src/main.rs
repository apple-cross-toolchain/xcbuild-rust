use anyhow::{Context, Result};
use std::env;
use std::fs;
use xcbuild_bom::Bom;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("error: missing argument");
        std::process::exit(1);
    }

    let path = &args[1];
    let data = fs::read(path).with_context(|| format!("failed to read {path}"))?;

    let bom = Bom::load(data).with_context(|| "failed to load BOM")?;

    println!(
        "Number of useful index blocks: {}",
        bom.block_count()
    );
    println!();

    println!("variables:");
    for var in bom.variables() {
        println!("\t{}: index {:x}", var.name, var.index);

        if bom.is_tree(var.index) {
            println!("\tFound BOM Tree:");
            if let Ok(entries) = bom.tree_entries(&var.name) {
                for entry in &entries {
                    println!(
                        "\t\tEntry with key of size {} and value of size {}",
                        entry.key.len(),
                        entry.value.len()
                    );
                }
            }
        }
    }
    println!();

    println!("index:");
    for (idx, _index) in bom.indices() {
        if let Some(data) = bom.index_get(idx) {
            println!("\t{}: data ({:x} bytes)", idx, data.len());
        }
    }

    Ok(())
}
