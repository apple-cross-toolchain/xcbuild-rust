use anyhow::{Context, Result};
use std::env;
use std::fs;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("usage: {} <file.hmap>", args[0]);
        std::process::exit(1);
    }

    let path = &args[1];
    let contents = fs::read(path).with_context(|| format!("cannot open '{path}', failed to read"))?;

    let hmap = xcbuild_hmap::HeaderMap::read(&contents)
        .map_err(|e| anyhow::anyhow!("cannot open '{path}', not an hmap file: {e}"))?;

    hmap.dump();

    Ok(())
}
