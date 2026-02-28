use std::env;
use std::process;
use xcbuild_pbxsetting::{Config, ConfigEntry};

fn dump_config(config: &Config, indent: usize) {
    println!("{:indent$}{}:", "", config.path, indent = indent);
    for entry in &config.entries {
        match entry {
            ConfigEntry::Setting(s) => {
                println!("{:indent$}  {} = {}", "", s.name, s.value, indent = indent);
            }
            ConfigEntry::Include { path, .. } => {
                println!("{:indent$}  #include \"{path}\"", "", indent = indent);
            }
        }
    }
    println!();

    // Recurse into includes
    for entry in &config.entries {
        if let ConfigEntry::Include { config, .. } = entry {
            dump_config(config, indent);
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("usage: dump_xcconfig <filename.xcconfig>");
        process::exit(1);
    }

    for path in &args {
        let config = match Config::load(path) {
            Some(c) => c,
            None => {
                eprintln!("error: couldn't open '{path}'");
                process::exit(1);
            }
        };

        dump_config(&config, 0);

        println!("Resolved settings:");
        for s in config.all_settings() {
            println!("  {} = {}", s.name, s.value);
        }
    }
}
