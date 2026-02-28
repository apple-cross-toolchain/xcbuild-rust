use std::fs;
use std::path::Path;

/// Options for the builtin-copyPlist tool.
pub struct CopyPlistOptions {
    pub output_dir: Option<String>,
    pub validate: bool,
    pub convert_format: Option<String>,
    pub inputs: Vec<String>,
}

impl CopyPlistOptions {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut opts = CopyPlistOptions {
            output_dir: None,
            validate: false,
            convert_format: None,
            inputs: Vec::new(),
        };

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--output-dir" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for --output-dir".into());
                    }
                    opts.output_dir = Some(args[i].clone());
                }
                "--validate" => opts.validate = true,
                "--convert" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for --convert".into());
                    }
                    opts.convert_format = Some(args[i].clone());
                }
                _ => {
                    if args[i].starts_with('-') {
                        return Err(format!("unknown option: {}", args[i]));
                    }
                    opts.inputs.push(args[i].clone());
                }
            }
            i += 1;
        }

        Ok(opts)
    }
}

/// Run the builtin-copyPlist command.
pub fn run(args: &[String]) -> i32 {
    let opts = match CopyPlistOptions::parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    let output_dir = match &opts.output_dir {
        Some(d) => d.clone(),
        None => {
            eprintln!("error: output directory not provided");
            return 1;
        }
    };

    if opts.inputs.is_empty() {
        eprintln!("error: no input files provided");
        return 1;
    }

    let out_path = Path::new(&output_dir);
    if let Err(e) = fs::create_dir_all(out_path) {
        eprintln!("error: failed to create output dir: {e}");
        return 1;
    }

    for input_path in &opts.inputs {
        let data = match fs::read(input_path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("error: unable to read input {input_path}: {e}");
                return 1;
            }
        };

        let output_data = if opts.convert_format.is_some() || opts.validate {
            // Parse and re-serialize
            let (value, format) = match xcbuild_plist::deserialize(&data) {
                Ok((v, f)) => (v, f),
                Err(e) => {
                    eprintln!("error: {input_path}: {e}");
                    return 1;
                }
            };

            let out_format = if let Some(fmt) = &opts.convert_format {
                match fmt.as_str() {
                    "binary1" => xcbuild_plist::PlistFormat::Binary,
                    "xml1" => xcbuild_plist::PlistFormat::Xml,
                    "ascii1" | "openstep1" => xcbuild_plist::PlistFormat::Ascii,
                    "json" => xcbuild_plist::PlistFormat::Json,
                    _ => {
                        eprintln!("error: unknown output format {fmt}");
                        return 1;
                    }
                }
            } else {
                format
            };

            match xcbuild_plist::serialize(&value, out_format) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("error: {input_path}: {e}");
                    return 1;
                }
            }
        } else {
            // Just copy as-is
            data
        };

        let basename = Path::new(input_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let output_file = out_path.join(&basename);

        if let Err(e) = fs::write(&output_file, &output_data) {
            eprintln!(
                "error: could not write to {}: {e}",
                output_file.display()
            );
            return 1;
        }
    }

    0
}
