use plist::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// A parsed specification from an .xcspec file.
#[derive(Debug, Clone)]
pub struct Specification {
    pub spec_type: String,
    pub identifier: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub based_on: Option<String>,
    pub properties: Value,
}

impl Specification {
    /// Parse a specification from a plist dictionary.
    pub fn from_value(value: &Value) -> Option<Specification> {
        let dict = match value {
            Value::Dictionary(d) => d,
            _ => return None,
        };

        let spec_type = match dict.get("Type") {
            Some(Value::String(s)) => s.clone(),
            _ => return None,
        };

        let identifier = match dict.get("Identifier") {
            Some(Value::String(s)) => s.clone(),
            _ => return None,
        };

        let name = match dict.get("Name") {
            Some(Value::String(s)) => Some(s.clone()),
            _ => None,
        };

        let description = match dict.get("Description") {
            Some(Value::String(s)) => Some(s.clone()),
            _ => None,
        };

        let based_on = match dict.get("BasedOn") {
            Some(Value::String(s)) => Some(s.clone()),
            _ => None,
        };

        Some(Specification {
            spec_type,
            identifier,
            name,
            description,
            based_on,
            properties: value.clone(),
        })
    }
}

/// A domain containing loaded specifications.
#[derive(Debug, Clone)]
pub struct SpecDomain {
    pub name: String,
    pub specs: Vec<Specification>,
}

/// A manager that loads and organizes build specifications.
#[derive(Debug, Clone)]
pub struct Manager {
    pub domains: HashMap<String, SpecDomain>,
}

impl Manager {
    pub fn new() -> Manager {
        Manager {
            domains: HashMap::new(),
        }
    }

    /// Register specifications from a file into a named domain.
    pub fn register_domain(&mut self, domain_name: &str, path: &str) -> bool {
        let specs = load_specs_from_file(path);
        if specs.is_empty() {
            return false;
        }

        let domain = self
            .domains
            .entry(domain_name.to_string())
            .or_insert_with(|| SpecDomain {
                name: domain_name.to_string(),
                specs: Vec::new(),
            });
        domain.specs.extend(specs);
        true
    }

    /// Register all .xcspec files from a directory into a named domain.
    pub fn register_domain_dir(&mut self, domain_name: &str, dir: &str) -> bool {
        let dir_path = Path::new(dir);
        if !dir_path.is_dir() {
            return false;
        }

        let mut found = false;
        if let Ok(entries) = fs::read_dir(dir_path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().map(|e| e == "xcspec").unwrap_or(false) {
                    if self.register_domain(domain_name, &p.to_string_lossy()) {
                        found = true;
                    }
                }
            }
        }
        found
    }

    /// Find a specification by type and identifier across all domains.
    pub fn find_spec(&self, spec_type: &str, identifier: &str) -> Option<&Specification> {
        for domain in self.domains.values() {
            for spec in &domain.specs {
                if spec.spec_type == spec_type && spec.identifier == identifier {
                    return Some(spec);
                }
            }
        }
        None
    }

    /// Get all specifications of a given type.
    pub fn specs_of_type(&self, spec_type: &str) -> Vec<&Specification> {
        let mut result = Vec::new();
        for domain in self.domains.values() {
            for spec in &domain.specs {
                if spec.spec_type == spec_type {
                    result.push(spec);
                }
            }
        }
        result
    }
}

impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}

/// Load specifications from an .xcspec file.
///
/// xcspec files can contain either a single dictionary or an array of dictionaries.
fn load_specs_from_file(path: &str) -> Vec<Specification> {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let value = match xcbuild_plist::deserialize(&data) {
        Ok((v, _)) => v,
        Err(_) => return Vec::new(),
    };

    let mut specs = Vec::new();

    match &value {
        Value::Array(arr) => {
            for item in arr {
                if let Some(spec) = Specification::from_value(item) {
                    specs.push(spec);
                }
            }
        }
        Value::Dictionary(_) => {
            if let Some(spec) = Specification::from_value(&value) {
                specs.push(spec);
            }
        }
        _ => {}
    }

    specs
}

/// Dump all specifications in a manager.
pub fn dump_manager(manager: &Manager) {
    for (domain_name, domain) in &manager.domains {
        println!("Domain: {domain_name}");
        for spec in &domain.specs {
            println!(
                "  [{type}] {id}",
                type = spec.spec_type,
                id = spec.identifier
            );
            if let Some(name) = &spec.name {
                println!("    Name: {name}");
            }
            if let Some(desc) = &spec.description {
                println!("    Description: {desc}");
            }
            if let Some(based_on) = &spec.based_on {
                println!("    BasedOn: {based_on}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_spec_from_dict() {
        let mut dict = plist::Dictionary::new();
        dict.insert("Type".to_string(), Value::String("Compiler".to_string()));
        dict.insert(
            "Identifier".to_string(),
            Value::String("com.apple.compilers.gcc".to_string()),
        );
        dict.insert("Name".to_string(), Value::String("GCC".to_string()));

        let spec = Specification::from_value(&Value::Dictionary(dict)).unwrap();
        assert_eq!(spec.spec_type, "Compiler");
        assert_eq!(spec.identifier, "com.apple.compilers.gcc");
        assert_eq!(spec.name.as_deref(), Some("GCC"));
    }

    #[test]
    fn test_manager_find() {
        let mut manager = Manager::new();
        let domain = manager
            .domains
            .entry("test".to_string())
            .or_insert_with(|| SpecDomain {
                name: "test".to_string(),
                specs: Vec::new(),
            });

        let mut dict = plist::Dictionary::new();
        dict.insert("Type".to_string(), Value::String("Tool".to_string()));
        dict.insert(
            "Identifier".to_string(),
            Value::String("com.apple.tools.cp".to_string()),
        );
        let spec = Specification::from_value(&Value::Dictionary(dict)).unwrap();
        domain.specs.push(spec);

        assert!(manager
            .find_spec("Tool", "com.apple.tools.cp")
            .is_some());
        assert!(manager.find_spec("Tool", "nonexistent").is_none());
    }
}
