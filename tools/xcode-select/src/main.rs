use std::env;
use std::process;
use xcbuild_sdk::*;

fn help(error: Option<&str>) -> ! {
    if let Some(e) = error {
        eprintln!("error: {e}\n");
    }

    eprintln!("Usage: xcode-select [action]\n");
    eprintln!("Manipulate default developer directory.\n");

    eprintln!("Actions:");
    eprintln!("  -p, --print-path");
    eprintln!("  -r, --reset");
    eprintln!("  -s <path>, --switch <path>");
    eprintln!("  --install");
    eprintln!();

    eprintln!("More information:");
    eprintln!("  -h, --help (this message)");
    eprintln!("  -v, --version");

    process::exit(if error.is_some() { 1 } else { 0 });
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    let mut show_help = false;
    let mut show_version = false;
    let mut print_path = false;
    let mut reset_path = false;
    let mut switch_path: Option<String> = None;
    let mut install = false;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-h" | "--help" => show_help = true,
            "-v" | "--version" | "-version" => show_version = true,
            "-p" | "--print-path" | "-print-path" => print_path = true,
            "-r" | "--reset" => reset_path = true,
            "-s" | "--switch" | "-switch" => {
                i += 1;
                if i >= args.len() {
                    help(Some("missing value for --switch"));
                }
                switch_path = Some(args[i].clone());
            }
            "--install" => install = true,
            _ => {
                help(Some(&format!("unknown argument {arg}")));
            }
        }
        i += 1;
    }

    if show_help {
        help(None);
    }

    if show_version {
        println!("xcode-select version 1 (xcbuild)");
        process::exit(0);
    }

    if print_path {
        match find_developer_root() {
            Some(path) => {
                println!("{path}");
                process::exit(0);
            }
            None => {
                eprintln!("error: no developer directory found");
                process::exit(1);
            }
        }
    }

    if reset_path {
        if !write_developer_root(None) {
            eprintln!("error: failed to reset developer root. are you root?");
            process::exit(1);
        }
        process::exit(0);
    }

    if let Some(path) = &switch_path {
        if !write_developer_root(Some(path)) {
            eprintln!("error: failed to set developer root. are you root?");
            process::exit(1);
        }
        process::exit(0);
    }

    if install {
        #[cfg(target_os = "macos")]
        {
            let status = process::Command::new("/usr/bin/xcode-select")
                .arg("--install")
                .status();
            match status {
                Ok(s) => process::exit(s.code().unwrap_or(1)),
                Err(e) => {
                    eprintln!("error: failed to launch xcode-select --install: {e}");
                    process::exit(1);
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            eprintln!("error: xcode-select --install is only supported on macOS");
            process::exit(1);
        }
    }

    help(Some("no actions provided"));
}
