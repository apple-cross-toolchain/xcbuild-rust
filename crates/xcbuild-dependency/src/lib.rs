use std::path::Path;
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Error, Debug)]
pub enum DependencyError {
    #[error("unterminated string in binary dependency info")]
    UnterminatedString,
    #[error("no string after command byte")]
    MissingString,
    #[error("multiple version commands")]
    MultipleVersions,
    #[error("unknown command byte: {0:#x}")]
    UnknownCommand(u8),
    #[error("invalid makefile syntax")]
    InvalidMakefileSyntax,
    #[error("output without inputs")]
    OutputWithoutInputs,
    #[error("path is not a directory")]
    NotADirectory,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unknown format: {0}")]
    UnknownFormat(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyInfoFormat {
    Binary,
    Directory,
    Makefile,
}

impl DependencyInfoFormat {
    pub fn name(&self) -> &'static str {
        match self {
            DependencyInfoFormat::Binary => "binary",
            DependencyInfoFormat::Directory => "directory",
            DependencyInfoFormat::Makefile => "makefile",
        }
    }

    pub fn parse(name: &str) -> Result<Self, DependencyError> {
        match name {
            "binary" => Ok(DependencyInfoFormat::Binary),
            "directory" => Ok(DependencyInfoFormat::Directory),
            "makefile" => Ok(DependencyInfoFormat::Makefile),
            _ => Err(DependencyError::UnknownFormat(name.to_string())),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DependencyInfo {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

// --- Binary format ---

#[derive(Debug, Clone, Default)]
pub struct BinaryDependencyInfo {
    pub version: String,
    pub missing: Vec<String>,
    pub dependency_info: DependencyInfo,
}

impl BinaryDependencyInfo {
    pub fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();

        if !self.version.is_empty() {
            write_command(&mut result, 0x00, &self.version);
        }
        for output in &self.dependency_info.outputs {
            write_command(&mut result, 0x40, output);
        }
        for input in &self.dependency_info.inputs {
            write_command(&mut result, 0x10, input);
        }
        for missing in &self.missing {
            write_command(&mut result, 0x11, missing);
        }

        result
    }

    pub fn deserialize(contents: &[u8]) -> Result<Self, DependencyError> {
        let mut version = String::new();
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut missing = Vec::new();

        let mut i = 0;
        while i < contents.len() {
            let command = contents[i];
            i += 1;

            if i >= contents.len() {
                return Err(DependencyError::MissingString);
            }

            let end = contents[i..]
                .iter()
                .position(|&b| b == 0)
                .ok_or(DependencyError::UnterminatedString)?;

            let s = String::from_utf8_lossy(&contents[i..i + end]).to_string();
            i += end + 1;

            match command {
                0x00 => {
                    if !version.is_empty() {
                        return Err(DependencyError::MultipleVersions);
                    }
                    version = s;
                }
                0x10 => inputs.push(s),
                0x40 => outputs.push(s),
                0x11 => missing.push(s),
                _ => return Err(DependencyError::UnknownCommand(command)),
            }
        }

        Ok(BinaryDependencyInfo {
            version,
            missing,
            dependency_info: DependencyInfo { inputs, outputs },
        })
    }
}

fn write_command(result: &mut Vec<u8>, command: u8, string: &str) {
    result.push(command);
    result.extend_from_slice(string.as_bytes());
    result.push(0);
}

// --- Makefile format ---

#[derive(Debug, Clone, Default)]
pub struct MakefileDependencyInfo {
    pub dependency_info: Vec<DependencyInfo>,
}

fn escape_makefile(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            ' ' | '#' | '%' => {
                result.push('\\');
                result.push(c);
            }
            _ => result.push(c),
        }
    }
    result
}

impl MakefileDependencyInfo {
    pub fn serialize(&self) -> String {
        let mut result = String::new();

        for (di_idx, dep_info) in self.dependency_info.iter().enumerate() {
            for (i, output) in dep_info.outputs.iter().enumerate() {
                if i > 0 {
                    result.push(' ');
                }
                result.push_str(&escape_makefile(output));
            }

            result.push(':');

            for input in &dep_info.inputs {
                result.push_str(" \\\n  ");
                result.push_str(&escape_makefile(input));
            }

            if di_idx + 1 < self.dependency_info.len() {
                result.push_str("\n\n");
            }
        }

        result
    }

    pub fn deserialize(contents: &str) -> Result<Self, DependencyError> {
        #[derive(PartialEq)]
        enum State {
            Begin,
            Comment,
            Output,
            Inputs,
        }

        let mut state = State::Begin;
        let mut current = String::new();
        let mut current_dep_info = DependencyInfo::default();
        let mut dependency_info = Vec::new();

        let chars: Vec<char> = contents.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];
            let escaped = i > 0 && chars[i - 1] == '\\';

            if !escaped && c == '#' {
                state = State::Comment;
                i += 1;
                continue;
            }

            if !escaped && c == '\n' {
                match state {
                    State::Begin => {}
                    State::Output => {
                        return Err(DependencyError::OutputWithoutInputs);
                    }
                    State::Comment | State::Inputs => {
                        if !current.is_empty() {
                            current_dep_info.inputs.push(current.clone());
                            current.clear();
                        }
                        if !current_dep_info.outputs.is_empty() {
                            dependency_info.push(current_dep_info.clone());
                            current_dep_info = DependencyInfo::default();
                        }
                        state = State::Begin;
                    }
                }
                i += 1;
                continue;
            }

            if (!escaped && c.is_ascii_whitespace()) || (escaped && c == '\n') {
                match state {
                    State::Begin | State::Comment | State::Output => {}
                    State::Inputs => {
                        if !current.is_empty() && escaped {
                            current.pop(); // remove backslash
                        }
                        if !current.is_empty() {
                            current_dep_info.inputs.push(current.clone());
                            current.clear();
                        }
                    }
                }
                i += 1;
                continue;
            }

            if !escaped && c == ':' {
                match state {
                    State::Begin => {
                        return Err(DependencyError::InvalidMakefileSyntax);
                    }
                    State::Comment => {}
                    State::Output => {
                        if !current.is_empty() {
                            current_dep_info.outputs.push(current.clone());
                            current.clear();
                        }
                        state = State::Inputs;
                    }
                    State::Inputs => {
                        return Err(DependencyError::InvalidMakefileSyntax);
                    }
                }
                i += 1;
                continue;
            }

            if c == '#' || c == '%' || (escaped && c.is_ascii_whitespace()) {
                match state {
                    State::Begin => {
                        return Err(DependencyError::InvalidMakefileSyntax);
                    }
                    State::Comment => {}
                    State::Output | State::Inputs => {
                        if escaped {
                            // Replace backslash with the escaped char
                            if !current.is_empty() {
                                current.pop();
                            }
                            current.push(c);
                        } else {
                            return Err(DependencyError::InvalidMakefileSyntax);
                        }
                    }
                }
                i += 1;
                continue;
            }

            match state {
                State::Begin => {
                    state = State::Output;
                    current.push(c);
                }
                State::Comment => {}
                State::Output | State::Inputs => {
                    current.push(c);
                }
            }

            i += 1;
        }

        match state {
            State::Begin => {}
            State::Output => {
                return Err(DependencyError::OutputWithoutInputs);
            }
            State::Comment | State::Inputs => {
                if !current.is_empty() {
                    current_dep_info.inputs.push(current);
                }
                if !current_dep_info.outputs.is_empty() {
                    dependency_info.push(current_dep_info);
                }
            }
        }

        Ok(MakefileDependencyInfo { dependency_info })
    }
}

// --- Directory format ---

#[derive(Debug, Clone)]
pub struct DirectoryDependencyInfo {
    pub directory: String,
    pub dependency_info: DependencyInfo,
}

impl DirectoryDependencyInfo {
    pub fn from_directory(directory: &str) -> Result<Self, DependencyError> {
        let path = Path::new(directory);
        if !path.is_dir() {
            return Err(DependencyError::NotADirectory);
        }

        let mut inputs = Vec::new();
        for entry in WalkDir::new(directory).into_iter().filter_map(|e| e.ok()) {
            if entry.path() != path {
                inputs.push(entry.path().to_string_lossy().to_string());
            }
        }

        Ok(DirectoryDependencyInfo {
            directory: directory.to_string(),
            dependency_info: DependencyInfo {
                inputs,
                outputs: Vec::new(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_roundtrip() {
        let info = BinaryDependencyInfo {
            version: "ld64-1".to_string(),
            missing: vec!["/missing/file".to_string()],
            dependency_info: DependencyInfo {
                inputs: vec!["/input/a.o".to_string()],
                outputs: vec!["/output/a.out".to_string()],
            },
        };

        let data = info.serialize();
        let info2 = BinaryDependencyInfo::deserialize(&data).unwrap();
        assert_eq!(info2.version, "ld64-1");
        assert_eq!(info2.dependency_info.inputs, vec!["/input/a.o"]);
        assert_eq!(info2.dependency_info.outputs, vec!["/output/a.out"]);
        assert_eq!(info2.missing, vec!["/missing/file"]);
    }

    #[test]
    fn test_makefile_roundtrip() {
        let info = MakefileDependencyInfo {
            dependency_info: vec![DependencyInfo {
                outputs: vec!["output.o".to_string()],
                inputs: vec!["input.c".to_string(), "header.h".to_string()],
            }],
        };

        let serialized = info.serialize();
        let info2 = MakefileDependencyInfo::deserialize(&serialized).unwrap();
        assert_eq!(info2.dependency_info.len(), 1);
        assert_eq!(info2.dependency_info[0].outputs, vec!["output.o"]);
        assert_eq!(
            info2.dependency_info[0].inputs,
            vec!["input.c", "header.h"]
        );
    }

    #[test]
    fn test_format_parse() {
        assert_eq!(
            DependencyInfoFormat::parse("binary").unwrap(),
            DependencyInfoFormat::Binary
        );
        assert_eq!(
            DependencyInfoFormat::parse("makefile").unwrap(),
            DependencyInfoFormat::Makefile
        );
        assert_eq!(
            DependencyInfoFormat::parse("directory").unwrap(),
            DependencyInfoFormat::Directory
        );
        assert!(DependencyInfoFormat::parse("unknown").is_err());
    }
}
