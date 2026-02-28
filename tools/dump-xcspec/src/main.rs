use std::env;
use std::process;
use xcbuild_pbxspec::{dump_manager, Manager};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("usage: dump_xcspec <file.xcspec | directory>");
        process::exit(1);
    }

    let mut manager = Manager::new();

    for (i, path) in args.iter().enumerate() {
        let domain_name = format!("arg{i}");
        let p = std::path::Path::new(path);

        if p.is_dir() {
            if !manager.register_domain_dir(&domain_name, path) {
                eprintln!("warning: no specs found in '{path}'");
            }
        } else if p.is_file() {
            if !manager.register_domain(&domain_name, path) {
                eprintln!("error: couldn't load '{path}'");
                process::exit(1);
            }
        } else {
            eprintln!("error: '{path}' not found");
            process::exit(1);
        }
    }

    dump_manager(&manager);
}
