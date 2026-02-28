use std::env;
use std::process;
use xcbuild_xcassets::{dump_asset, Asset};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("usage: dump_xcassets <path.xcassets>");
        process::exit(1);
    }

    for path in &args {
        let asset = match Asset::load(path) {
            Some(a) => a,
            None => {
                eprintln!("error: couldn't load '{path}'");
                process::exit(1);
            }
        };

        dump_asset(&asset, 0);
    }
}
