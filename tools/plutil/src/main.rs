use anyhow::{bail, Context, Result};
use plist::Value;
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use xcbuild_plist::*;

fn help(error: Option<&str>) -> ! {
    if let Some(e) = error {
        eprintln!("error: {e}\n");
    }

    eprintln!("usage: plutil -<command> [options] <files>\n");

    eprintln!("commands:");
    eprintln!("  -lint (default)");
    eprintln!("  -help (this message)");
    eprintln!("  -p");
    eprintln!("  -convert <format>");
    eprintln!("  -insert <key> <value>");
    eprintln!("  -replace <key> <value>");
    eprintln!("  -remove <key>");
    eprintln!("  -extract <key> <format>");

    eprintln!("\nvalues:");
    eprintln!("  -bool <YES|NO>");
    eprintln!("  -integer <number>");
    eprintln!("  -float <number>");
    eprintln!("  -string <utf8>");
    eprintln!("  -data <base64>");
    eprintln!("  -date <iso8601>");
    eprintln!("  -xml <plist>");
    eprintln!("  -json <json>");

    eprintln!("\nformats:");
    eprintln!("  xml1");
    eprintln!("  binary1");
    eprintln!("  openstep1");
    eprintln!("  json");

    std::process::exit(if error.is_some() { 1 } else { 0 });
}

#[derive(Debug, Clone, Copy)]
enum Command {
    Lint,
    Print,
    Help,
}

#[derive(Debug, Clone, Copy)]
enum AdjustmentType {
    Insert,
    Replace,
    Remove,
    Extract,
}

#[derive(Debug)]
struct Adjustment {
    adj_type: AdjustmentType,
    path: String,
    value: Option<Value>,
}

fn parse_value_arg(type_arg: &str, value_str: &str) -> Result<Value> {
    match type_arg {
        "-bool" => {
            let b = value_str == "YES" || value_str == "true";
            Ok(Value::Boolean(b))
        }
        "-integer" => {
            let i: i64 = value_str.parse().context("invalid integer argument")?;
            Ok(Value::Integer(i.into()))
        }
        "-float" => {
            let f: f64 = value_str.parse().context("invalid float argument")?;
            Ok(Value::Real(f))
        }
        "-string" => Ok(Value::String(value_str.to_string())),
        "-date" => Ok(Value::String(value_str.to_string())),
        "-data" => Ok(Value::Data(value_str.as_bytes().to_vec())),
        "-xml" => {
            let data = value_str.as_bytes();
            let value = xcbuild_plist::deserialize_with_format(data, PlistFormat::Xml)
                .context("invalid XML value")?;
            Ok(value)
        }
        "-json" => {
            let data = value_str.as_bytes();
            let value = xcbuild_plist::deserialize_with_format(data, PlistFormat::Json)
                .context("invalid JSON value")?;
            Ok(value)
        }
        _ => bail!("unknown type option {type_arg}"),
    }
}

/// Parse plutil's dot-separated key path.
fn parse_dot_key_path(path: &str) -> Vec<String> {
    path.split('.').map(|s| s.to_string()).collect()
}

fn read_input(path: &str) -> Result<Vec<u8>> {
    if path == "-" {
        let mut buf = Vec::new();
        io::stdin().read_to_end(&mut buf)?;
        Ok(buf)
    } else {
        fs::read(path).with_context(|| format!("unable to read {path}"))
    }
}

fn write_output(path: &str, data: &[u8]) -> Result<()> {
    if path == "-" {
        io::stdout().write_all(data)?;
        Ok(())
    } else {
        fs::write(path, data).with_context(|| format!("unable to write {path}"))
    }
}

fn output_path(output: &Option<String>, extension: &Option<String>, file: &str) -> String {
    if let Some(o) = output {
        return o.clone();
    }
    if file != "-" {
        if let Some(ext) = extension {
            let p = Path::new(file);
            let stem = p.file_stem().unwrap_or_default().to_string_lossy();
            let parent = p.parent().unwrap_or_else(|| Path::new(""));
            return parent.join(format!("{stem}.{ext}")).to_string_lossy().to_string();
        }
    }
    file.to_string()
}

fn perform_adjustment(
    root: &mut Value,
    adjustment: &Adjustment,
) -> Result<Option<Value>> {
    let keys = parse_dot_key_path(&adjustment.path);
    if keys.is_empty() {
        bail!("invalid key path");
    }

    let parent_keys = &keys[..keys.len() - 1];
    let last_key = &keys[keys.len() - 1];

    // Navigate to parent
    let parent = if parent_keys.is_empty() {
        root
    } else {
        let mut current: &mut Value = root;
        for key in parent_keys {
            current = match current {
                Value::Dictionary(dict) => dict
                    .get_mut(key.as_str())
                    .ok_or_else(|| anyhow::anyhow!("invalid key path"))?,
                Value::Array(arr) => {
                    let idx: usize = key.parse().context("invalid key path")?;
                    arr.get_mut(idx)
                        .ok_or_else(|| anyhow::anyhow!("invalid key path"))?
                }
                _ => bail!("invalid key path"),
            };
        }
        current
    };

    match adjustment.adj_type {
        AdjustmentType::Insert => {
            let value = adjustment.value.as_ref().expect("insert requires value");
            match parent {
                Value::Dictionary(dict) => {
                    if dict.get(last_key.as_str()).is_none() {
                        dict.insert(last_key.clone(), value.clone());
                    }
                }
                Value::Array(arr) => {
                    let idx: usize = last_key.parse().unwrap_or(arr.len());
                    if idx < arr.len() {
                        arr.insert(idx, value.clone());
                    } else {
                        arr.push(value.clone());
                    }
                }
                _ => bail!("invalid key path"),
            }
            Ok(None)
        }
        AdjustmentType::Replace => {
            let value = adjustment.value.as_ref().expect("replace requires value");
            match parent {
                Value::Dictionary(dict) => {
                    dict.insert(last_key.clone(), value.clone());
                }
                Value::Array(arr) => {
                    let idx: usize = last_key.parse().unwrap_or(arr.len());
                    if idx < arr.len() {
                        arr[idx] = value.clone();
                    } else {
                        arr.push(value.clone());
                    }
                }
                _ => bail!("invalid key path"),
            }
            Ok(None)
        }
        AdjustmentType::Remove => match parent {
            Value::Dictionary(dict) => {
                dict.remove(last_key.as_str());
                Ok(None)
            }
            Value::Array(arr) => {
                let idx: usize = last_key.parse().context("invalid array index")?;
                if idx < arr.len() {
                    arr.remove(idx);
                }
                Ok(None)
            }
            _ => bail!("invalid key path"),
        },
        AdjustmentType::Extract => {
            let extracted = match parent {
                Value::Dictionary(dict) => dict.get(last_key.as_str()).cloned(),
                Value::Array(arr) => {
                    let idx: usize = last_key.parse().context("invalid array index")?;
                    arr.get(idx).cloned()
                }
                _ => None,
            };
            Ok(extracted)
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    let mut command: Option<Command> = None;
    let mut adjustments: Vec<Adjustment> = Vec::new();
    let mut convert_format: Option<PlistFormat> = None;
    let mut inputs: Vec<String> = Vec::new();
    let mut output: Option<String> = None;
    let mut extension: Option<String> = None;
    let mut silent = false;
    let mut separator = false;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        if separator {
            inputs.push(arg.clone());
            i += 1;
            continue;
        }

        match arg.as_str() {
            "-help" => {
                command = Some(Command::Help);
            }
            "-lint" => {
                command = Some(Command::Lint);
            }
            "-p" => {
                command = Some(Command::Print);
            }
            "-convert" => {
                i += 1;
                if i >= args.len() {
                    help(Some("missing value for -convert"));
                }
                let fmt = PlistFormat::parse(&args[i]);
                if fmt.is_none() {
                    help(Some(&format!("unknown format {}", args[i])));
                }
                convert_format = fmt;
            }
            "-insert" => {
                i += 1;
                if i + 2 >= args.len() {
                    help(Some("missing arguments for -insert"));
                }
                let path = args[i].clone();
                i += 1;
                let type_arg = &args[i];
                i += 1;
                let value_str = &args[i];
                match parse_value_arg(type_arg, value_str) {
                    Ok(value) => adjustments.push(Adjustment {
                        adj_type: AdjustmentType::Insert,
                        path,
                        value: Some(value),
                    }),
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(1);
                    }
                }
            }
            "-replace" => {
                i += 1;
                if i + 2 >= args.len() {
                    help(Some("missing arguments for -replace"));
                }
                let path = args[i].clone();
                i += 1;
                let type_arg = &args[i];
                i += 1;
                let value_str = &args[i];
                match parse_value_arg(type_arg, value_str) {
                    Ok(value) => adjustments.push(Adjustment {
                        adj_type: AdjustmentType::Replace,
                        path,
                        value: Some(value),
                    }),
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(1);
                    }
                }
            }
            "-remove" => {
                i += 1;
                if i >= args.len() {
                    help(Some("missing argument for -remove"));
                }
                adjustments.push(Adjustment {
                    adj_type: AdjustmentType::Remove,
                    path: args[i].clone(),
                    value: None,
                });
            }
            "-extract" => {
                i += 1;
                if i + 1 >= args.len() {
                    help(Some("missing arguments for -extract"));
                }
                let path = args[i].clone();
                i += 1;
                let fmt = PlistFormat::parse(&args[i]);
                if fmt.is_none() {
                    help(Some(&format!("unknown format {}", args[i])));
                }
                convert_format = fmt;
                adjustments.push(Adjustment {
                    adj_type: AdjustmentType::Extract,
                    path,
                    value: None,
                });
            }
            "-o" => {
                i += 1;
                if i >= args.len() {
                    help(Some("missing value for -o"));
                }
                output = Some(args[i].clone());
            }
            "-e" => {
                i += 1;
                if i >= args.len() {
                    help(Some("missing value for -e"));
                }
                extension = Some(args[i].clone());
            }
            "-s" => {
                silent = true;
            }
            "-r" => {
                // human readable - currently a no-op
            }
            "--" => {
                separator = true;
            }
            _ => {
                if arg.starts_with('-') {
                    help(Some(&format!("unknown argument {arg}")));
                }
                inputs.push(arg.clone());
            }
        }

        i += 1;
    }

    let modify = convert_format.is_some() || !adjustments.is_empty();

    // Handle help
    if matches!(command, Some(Command::Help)) {
        help(None);
    }

    // Check conflicts
    if modify && matches!(command, Some(Command::Lint | Command::Print)) {
        help(Some("conflicting options specified"));
    }

    if inputs.is_empty() {
        help(Some("no input files"));
    }

    let mut success = true;

    for file in &inputs {
        let data = match read_input(file) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("error: {e}");
                success = false;
                continue;
            }
        };

        let (root, format) = match xcbuild_plist::deserialize(&data) {
            Ok((v, f)) => (v, f),
            Err(e) => {
                eprintln!("error: {file}: {e}");
                success = false;
                continue;
            }
        };

        if modify {
            let mut root = root;
            let mut write_value: Option<Value> = None;

            for adj in &adjustments {
                match perform_adjustment(&mut root, adj) {
                    Ok(Some(extracted)) => {
                        write_value = Some(extracted);
                    }
                    Ok(None) => {}
                    Err(e) => {
                        eprintln!("error: {e}");
                        success = false;
                    }
                }
            }

            let out_value = write_value.as_ref().unwrap_or(&root);
            let out_format = convert_format.unwrap_or(format);

            match xcbuild_plist::serialize(out_value, out_format) {
                Ok(bytes) => {
                    let out_path = output_path(&output, &extension, file);
                    if let Err(e) = write_output(&out_path, &bytes) {
                        eprintln!("error: {e}");
                        success = false;
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    success = false;
                }
            }
        } else if matches!(command, Some(Command::Print)) {
            match xcbuild_plist::serialize(&root, PlistFormat::Ascii) {
                Ok(bytes) => {
                    if let Err(e) = write_output("-", &bytes) {
                        eprintln!("error: {e}");
                        success = false;
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    success = false;
                }
            }
        } else {
            // Default: lint
            if !silent {
                println!("{file}: OK");
            }
        }
    }

    std::process::exit(if success { 0 } else { 1 });
}
