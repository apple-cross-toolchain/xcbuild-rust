/// Run the builtin-lsRegisterURL command.
/// This is macOS-specific (uses Launch Services) and is a no-op on other platforms.
pub fn run(args: &[String]) -> i32 {
    let mut input: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--input" | "-input" => {
                i += 1;
                if i < args.len() {
                    input = Some(args[i].clone());
                }
            }
            _ => {
                if !args[i].starts_with('-') && input.is_none() {
                    input = Some(args[i].clone());
                }
            }
        }
        i += 1;
    }

    let _input = match input {
        Some(i) => i,
        None => {
            eprintln!("error: no input provided");
            return 1;
        }
    };

    // Launch Services registration is macOS-specific
    eprintln!("warning: lsRegisterURL is not supported on this platform");
    0
}
