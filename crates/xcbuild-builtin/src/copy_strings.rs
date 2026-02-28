use std::fs;
use std::path::Path;

/// Options for the builtin-copyStrings tool.
pub struct CopyStringsOptions {
    pub output_dir: Option<String>,
    pub validate: bool,
    pub input_encoding: Option<String>,
    pub output_encoding: Option<String>,
    pub inputs: Vec<String>,
}

impl CopyStringsOptions {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut opts = CopyStringsOptions {
            output_dir: None,
            validate: false,
            input_encoding: None,
            output_encoding: None,
            inputs: Vec::new(),
        };

        let mut i = 0;
        let mut separator = false;
        while i < args.len() {
            if separator {
                opts.inputs.push(args[i].clone());
                i += 1;
                continue;
            }
            match args[i].as_str() {
                "--" => {
                    separator = true;
                }
                "--output-dir" | "--outdir" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for --output-dir".into());
                    }
                    opts.output_dir = Some(args[i].clone());
                }
                "--validate" => opts.validate = true,
                "--input-encoding" | "--inputencoding" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for --input-encoding".into());
                    }
                    opts.input_encoding = Some(args[i].clone());
                }
                "--output-encoding" | "--outputencoding" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for --output-encoding".into());
                    }
                    opts.output_encoding = Some(args[i].clone());
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

/// Run the builtin-copyStrings command.
/// This copies .strings and .stringsdict files, optionally converting between
/// plist formats (binary, XML) which handles encoding conversion.
pub fn run(args: &[String]) -> i32 {
    let opts = match CopyStringsOptions::parse(args) {
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
                eprintln!("error: unable to read {input_path}: {e}");
                return 1;
            }
        };

        // For strings files, parse as plist and re-serialize
        // (the standard behavior for Xcode's builtin-copyStrings)
        let output_data = match xcbuild_plist::deserialize(&data) {
            Ok((value, _format)) => {
                if opts.validate {
                    if let plist::Value::Dictionary(ref d) = value {
                        for (key, val) in d.iter() {
                            if !matches!(val, plist::Value::String(_) | plist::Value::Dictionary(_)) {
                                eprintln!("error: {input_path}: invalid value for key '{key}'");
                                return 1;
                            }
                        }
                    } else {
                        eprintln!("error: {input_path}: not a valid strings dictionary");
                        return 1;
                    }
                }
                let out_format = match opts.output_encoding.as_deref() {
                    Some("binary") => xcbuild_plist::PlistFormat::Binary,
                    Some("utf-8") | Some("utf-16") | Some("utf-32") => xcbuild_plist::PlistFormat::Xml,
                    None => xcbuild_plist::PlistFormat::Binary,
                    Some(enc) => {
                        eprintln!("error: unknown output encoding '{enc}'");
                        return 1;
                    }
                };
                match xcbuild_plist::serialize(&value, out_format) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("error: {input_path}: {e}");
                        return 1;
                    }
                }
            }
            Err(e) => {
                eprintln!("error: {input_path}: failed to parse: {e}");
                return 1;
            }
        };

        let basename = Path::new(input_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let output_file = out_path.join(&basename);

        if let Err(e) = fs::write(&output_file, &output_data) {
            eprintln!("error: could not write to {}: {e}", output_file.display());
            return 1;
        }
    }

    0
}
