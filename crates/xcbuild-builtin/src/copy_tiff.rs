/// Run the builtin-copyTiff command.
/// Note: TIFF copying is not yet implemented (matches C++ status).
pub fn run(args: &[String]) -> i32 {
    let _ = args;
    eprintln!("error: copy tiff not supported");
    1
}
