use plist::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// A parsed .pbxproj file.
#[derive(Debug, Clone)]
pub struct PbxProject {
    pub path: String,
    pub root_object_id: String,
    pub objects: HashMap<String, Value>,
    pub archive_version: Option<String>,
    pub object_version: Option<String>,
}

impl PbxProject {
    /// Open a .xcodeproj directory and parse the project.pbxproj file.
    pub fn open(path: &str) -> Option<PbxProject> {
        let pbxproj_path = if path.ends_with(".pbxproj") {
            path.to_string()
        } else {
            format!("{path}/project.pbxproj")
        };

        let data = fs::read(&pbxproj_path).ok()?;
        let (value, _format) = xcbuild_plist::deserialize(&data).ok()?;

        let dict = match &value {
            Value::Dictionary(d) => d,
            _ => return None,
        };

        let root_object_id = match dict.get("rootObject") {
            Some(Value::String(s)) => s.clone(),
            _ => return None,
        };

        let objects = match dict.get("objects") {
            Some(Value::Dictionary(d)) => {
                let mut map = HashMap::new();
                for (k, v) in d.iter() {
                    map.insert(k.clone(), v.clone());
                }
                map
            }
            _ => return None,
        };

        let archive_version = match dict.get("archiveVersion") {
            Some(Value::String(s)) => Some(s.clone()),
            _ => None,
        };

        let object_version = match dict.get("objectVersion") {
            Some(Value::String(s)) => Some(s.clone()),
            _ => None,
        };

        let real_path = Path::new(&pbxproj_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_string_lossy()
            .to_string();

        Some(PbxProject {
            path: real_path,
            root_object_id,
            objects,
            archive_version,
            object_version,
        })
    }

    /// Get an object by its ID.
    pub fn object(&self, id: &str) -> Option<&Value> {
        self.objects.get(id)
    }

    /// Get a string property from an object.
    pub fn get_string(&self, obj: &Value, key: &str) -> Option<String> {
        match obj {
            Value::Dictionary(d) => match d.get(key) {
                Some(Value::String(s)) => Some(s.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    /// Get an array property from an object.
    pub fn get_array(&self, obj: &Value, key: &str) -> Vec<Value> {
        match obj {
            Value::Dictionary(d) => match d.get(key) {
                Some(Value::Array(arr)) => arr.clone(),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    /// Get the root project object.
    pub fn root_object(&self) -> Option<&Value> {
        self.object(&self.root_object_id)
    }

    /// Get the project name (from the directory name).
    pub fn name(&self) -> String {
        Path::new(&self.path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    /// List all target IDs from the root project object.
    pub fn target_ids(&self) -> Vec<String> {
        if let Some(root) = self.root_object() {
            self.get_array(root, "targets")
                .iter()
                .filter_map(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get the main group ID from the root project object.
    pub fn main_group_id(&self) -> Option<String> {
        self.root_object()
            .and_then(|root| self.get_string(root, "mainGroup"))
    }
}

/// Recursively dump a group's children for display.
pub fn dump_group(project: &PbxProject, group_id: &str, indent: usize) {
    let obj = match project.object(group_id) {
        Some(o) => o,
        None => return,
    };

    let isa = project.get_string(obj, "isa").unwrap_or_default();
    let name = project
        .get_string(obj, "name")
        .or_else(|| project.get_string(obj, "path"))
        .unwrap_or_else(|| group_id.to_string());

    let is_variant = isa == "PBXVariantGroup";
    let (open, close) = if is_variant { ('{', '}') } else { ('[', ']') };

    println!("{:indent$}{open}{name}{close}", "", indent = indent * 2);

    let children = project.get_array(obj, "children");
    for child in &children {
        if let Value::String(child_id) = child {
            if let Some(child_obj) = project.object(child_id) {
                let child_isa = project.get_string(child_obj, "isa").unwrap_or_default();
                match child_isa.as_str() {
                    "PBXGroup" | "PBXVariantGroup" => {
                        dump_group(project, child_id, indent + 1);
                    }
                    "PBXFileReference" => {
                        let child_name = project
                            .get_string(child_obj, "name")
                            .or_else(|| project.get_string(child_obj, "path"))
                            .unwrap_or_default();
                        let child_path = project
                            .get_string(child_obj, "path")
                            .unwrap_or_default();
                        println!(
                            "{:indent$}{child_name} [{child_path}]",
                            "",
                            indent = (indent + 1) * 2
                        );
                    }
                    "PBXReferenceProxy" => {
                        let child_name = project
                            .get_string(child_obj, "name")
                            .unwrap_or_default();
                        println!(
                            "{:indent$}{child_name} [proxy]",
                            "",
                            indent = (indent + 1) * 2
                        );
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_string_from_dict() {
        let mut dict = plist::Dictionary::new();
        dict.insert("key".to_string(), Value::String("value".to_string()));
        let project = PbxProject {
            path: String::new(),
            root_object_id: String::new(),
            objects: HashMap::new(),
            archive_version: None,
            object_version: None,
        };
        assert_eq!(
            project.get_string(&Value::Dictionary(dict), "key"),
            Some("value".to_string())
        );
    }
}
