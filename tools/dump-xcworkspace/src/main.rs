use std::env;
use std::process;
use xcbuild_xcworkspace::{dump_items, Workspace};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("usage: dump_xcworkspace <path.xcworkspace>");
        process::exit(1);
    }

    for path in &args {
        let workspace = match Workspace::open(path) {
            Some(w) => w,
            None => {
                eprintln!("error: couldn't open '{path}'");
                process::exit(1);
            }
        };

        println!("Workspace: {}", workspace.name);
        println!("Path: {}", workspace.project_file);
        println!("Base: {}", workspace.base_path);
        println!("Data: {}", workspace.data_file);
        println!();

        println!("Items:");
        dump_items(&workspace.items, &workspace.base_path, 1);
    }
}
