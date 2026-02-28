use std::fs;
use std::path::Path;

/// Options for the builtin-infoPlistUtility tool.
pub struct InfoPlistOptions {
    pub input: Option<String>,
    pub output: Option<String>,
    pub additional_content_files: Vec<String>,
    pub format: Option<String>,
    pub expand_build_settings: bool,
    pub platform: Option<String>,
    pub required_architectures: Vec<String>,
    pub gen_pkg_info: Option<String>,
    pub resource_rules_file: Option<String>,
    pub info_file_keys: Vec<String>,
    pub info_file_values: Vec<String>,
}

impl InfoPlistOptions {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut opts = InfoPlistOptions {
            input: None,
            output: None,
            additional_content_files: Vec::new(),
            format: None,
            expand_build_settings: false,
            platform: None,
            required_architectures: Vec::new(),
            gen_pkg_info: None,
            resource_rules_file: None,
            info_file_keys: Vec::new(),
            info_file_values: Vec::new(),
        };

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "-input" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for -input".into());
                    }
                    opts.input = Some(args[i].clone());
                }
                "-output" | "-o" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for -output".into());
                    }
                    opts.output = Some(args[i].clone());
                }
                "-additionalcontentfile" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for -additionalcontentfile".into());
                    }
                    opts.additional_content_files.push(args[i].clone());
                }
                "-format" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for -format".into());
                    }
                    opts.format = Some(args[i].clone());
                }
                "-expandbuildsettings" => opts.expand_build_settings = true,
                "-platform" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for -platform".into());
                    }
                    opts.platform = Some(args[i].clone());
                }
                "-requiredArchitecture" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for -requiredArchitecture".into());
                    }
                    opts.required_architectures.push(args[i].clone());
                }
                "-genpkginfo" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for -genpkginfo".into());
                    }
                    opts.gen_pkg_info = Some(args[i].clone());
                }
                "-resourcerulesfile" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for -resourcerulesfile".into());
                    }
                    opts.resource_rules_file = Some(args[i].clone());
                }
                "-infofilekeys" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for -infofilekeys".into());
                    }
                    opts.info_file_keys = args[i].split(';').map(|s| s.to_string()).collect();
                }
                "-infofilevalues" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for -infofilevalues".into());
                    }
                    opts.info_file_values = args[i].split(';').map(|s| s.to_string()).collect();
                }
                _ => {
                    if args[i].starts_with('-') {
                        // Skip unknown flags gracefully
                    } else if opts.input.is_none() {
                        opts.input = Some(args[i].clone());
                    }
                }
            }
            i += 1;
        }

        Ok(opts)
    }
}

/// Run the builtin-infoPlistUtility command.
pub fn run(args: &[String]) -> i32 {
    let opts = match InfoPlistOptions::parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    let input = match &opts.input {
        Some(i) => i.clone(),
        None => {
            eprintln!("error: no input file provided");
            return 1;
        }
    };

    let output = match &opts.output {
        Some(o) => o.clone(),
        None => {
            eprintln!("error: no output file provided");
            return 1;
        }
    };

    // Read the input plist
    let data = match fs::read(&input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: unable to read {input}: {e}");
            return 1;
        }
    };

    let (mut value, _format) = match xcbuild_plist::deserialize(&data) {
        Ok((v, f)) => (v, f),
        Err(e) => {
            eprintln!("error: {input}: {e}");
            return 1;
        }
    };

    // Merge additional content files
    for acf in &opts.additional_content_files {
        let acf_data = match fs::read(acf) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("error: unable to read {acf}: {e}");
                return 1;
            }
        };
        if let Ok((acf_value, _)) = xcbuild_plist::deserialize(&acf_data) {
            if let (plist::Value::Dictionary(ref mut base), plist::Value::Dictionary(extra)) =
                (&mut value, acf_value)
            {
                for (k, v) in extra.into_iter() {
                    base.insert(k, v);
                }
            }
        }
    }

    // Add info file keys/values
    if let plist::Value::Dictionary(ref mut dict) = value {
        for (key, val) in opts.info_file_keys.iter().zip(opts.info_file_values.iter()) {
            if !key.is_empty() {
                dict.insert(key.clone(), plist::Value::String(val.clone()));
            }
        }
    }

    // Determine output format
    let out_format = if let Some(fmt) = &opts.format {
        match fmt.as_str() {
            "binary" => xcbuild_plist::PlistFormat::Binary,
            "xml" => xcbuild_plist::PlistFormat::Xml,
            "ascii" | "openstep" => xcbuild_plist::PlistFormat::Ascii,
            "json" => xcbuild_plist::PlistFormat::Json,
            _ => xcbuild_plist::PlistFormat::Binary,
        }
    } else {
        xcbuild_plist::PlistFormat::Binary
    };

    // Serialize and write
    let output_data = match xcbuild_plist::serialize(&value, out_format) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: serialization failed: {e}");
            return 1;
        }
    };

    if let Some(parent) = Path::new(&output).parent() {
        let _ = fs::create_dir_all(parent);
    }

    if let Err(e) = fs::write(&output, &output_data) {
        eprintln!("error: unable to write {output}: {e}");
        return 1;
    }

    // Generate PkgInfo if requested
    if let Some(pkg_info_path) = &opts.gen_pkg_info {
        if let plist::Value::Dictionary(ref dict) = value {
            let pkg_type = match dict.get("CFBundlePackageType") {
                Some(plist::Value::String(s)) => s.clone(),
                _ => "????".to_string(),
            };
            let signature = match dict.get("CFBundleSignature") {
                Some(plist::Value::String(s)) => s.clone(),
                _ => "????".to_string(),
            };
            let pkg_info = format!("{pkg_type}{signature}");
            if let Some(parent) = Path::new(pkg_info_path).parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Err(e) = fs::write(pkg_info_path, pkg_info.as_bytes()) {
                eprintln!("error: unable to write PkgInfo: {e}");
                return 1;
            }
        }
    }

    0
}
