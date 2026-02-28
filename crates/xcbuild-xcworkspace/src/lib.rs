use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs;
use std::path::Path;

/// An item in a workspace (either a group or a file reference).
#[derive(Debug, Clone)]
pub enum WorkspaceItem {
    Group {
        name: String,
        location: String,
        location_type: String,
        items: Vec<WorkspaceItem>,
    },
    FileRef {
        location: String,
        location_type: String,
    },
}

impl WorkspaceItem {
    /// Resolve the item's path relative to the workspace base path.
    pub fn resolve(&self, base_path: &str) -> String {
        let (location, loc_type) = match self {
            WorkspaceItem::Group {
                location,
                location_type,
                ..
            } => (location.as_str(), location_type.as_str()),
            WorkspaceItem::FileRef {
                location,
                location_type,
            } => (location.as_str(), location_type.as_str()),
        };

        match loc_type {
            "group" | "container" => {
                if location.is_empty() {
                    base_path.to_string()
                } else {
                    format!("{base_path}/{location}")
                }
            }
            "absolute" => location.to_string(),
            "developer" => {
                if let Ok(dev_dir) = std::env::var("DEVELOPER_DIR") {
                    format!("{dev_dir}/{location}")
                } else {
                    location.to_string()
                }
            }
            _ => format!("{base_path}/{location}"),
        }
    }

    pub fn location_type_str(&self) -> &str {
        match self {
            WorkspaceItem::Group { location_type, .. } => location_type,
            WorkspaceItem::FileRef { location_type, .. } => location_type,
        }
    }
}

/// A parsed .xcworkspace.
#[derive(Debug, Clone)]
pub struct Workspace {
    pub name: String,
    pub base_path: String,
    pub project_file: String,
    pub data_file: String,
    pub items: Vec<WorkspaceItem>,
}

impl Workspace {
    /// Open an .xcworkspace directory.
    pub fn open(path: &str) -> Option<Workspace> {
        let data_file = format!("{path}/contents.xcworkspacedata");
        if !Path::new(&data_file).is_file() {
            return None;
        }

        let contents = fs::read_to_string(&data_file).ok()?;
        let items = parse_workspace_xml(&contents)?;

        let project_file = path.to_string();
        let base_path = Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());
        let name = Path::new(path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        Some(Workspace {
            name,
            base_path,
            project_file,
            data_file,
            items,
        })
    }
}

fn parse_workspace_xml(xml: &str) -> Option<Vec<WorkspaceItem>> {
    let mut reader = Reader::from_str(xml);

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"Workspace" => {
                return Some(parse_items(&mut reader));
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    None
}

fn parse_items(reader: &mut Reader<&[u8]>) -> Vec<WorkspaceItem> {
    let mut items = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "Group" => {
                        let (location, location_type, name) = parse_location_attrs(e);
                        let children = parse_items(reader);
                        items.push(WorkspaceItem::Group {
                            name: name.unwrap_or_else(|| location.clone()),
                            location,
                            location_type,
                            items: children,
                        });
                    }
                    "FileRef" => {
                        let (location, location_type, _) = parse_location_attrs(e);
                        items.push(WorkspaceItem::FileRef {
                            location,
                            location_type,
                        });
                        // Read until end of FileRef
                        let _ = reader.read_to_end(e.name());
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "FileRef" {
                    let (location, location_type, _) = parse_empty_location_attrs(e);
                    items.push(WorkspaceItem::FileRef {
                        location,
                        location_type,
                    });
                } else if tag == "Group" {
                    let (location, location_type, name) = parse_empty_location_attrs(e);
                    items.push(WorkspaceItem::Group {
                        name: name.unwrap_or_else(|| location.clone()),
                        location,
                        location_type,
                        items: Vec::new(),
                    });
                }
            }
            Ok(Event::End(_)) => break,
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    items
}

fn parse_location_attrs(
    e: &quick_xml::events::BytesStart,
) -> (String, String, Option<String>) {
    let mut location = String::new();
    let mut location_type = "group".to_string();
    let mut name = None;

    for attr in e.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
        let val = String::from_utf8_lossy(&attr.value).to_string();
        match key.as_str() {
            "location" => {
                // Format: "type:path"
                if let Some(colon) = val.find(':') {
                    location_type = val[..colon].to_string();
                    location = val[colon + 1..].to_string();
                } else {
                    location = val;
                }
            }
            "name" => name = Some(val),
            _ => {}
        }
    }

    (location, location_type, name)
}

fn parse_empty_location_attrs(
    e: &quick_xml::events::BytesStart,
) -> (String, String, Option<String>) {
    parse_location_attrs(e)
}

/// Recursively dump workspace items.
pub fn dump_items(items: &[WorkspaceItem], base_path: &str, indent: usize) {
    for item in items {
        match item {
            WorkspaceItem::Group {
                name,
                items: children,
                ..
            } => {
                let resolved = item.resolve(base_path);
                println!(
                    "{:indent$}[{name}] ({resolved} [{loc}])",
                    "",
                    indent = indent * 2,
                    loc = item.location_type_str()
                );
                dump_items(children, base_path, indent + 1);
            }
            WorkspaceItem::FileRef { .. } => {
                let resolved = item.resolve(base_path);
                println!(
                    "{:indent$}{resolved} [{loc}]",
                    "",
                    indent = indent * 2,
                    loc = item.location_type_str()
                );
            }
        }
    }
}
