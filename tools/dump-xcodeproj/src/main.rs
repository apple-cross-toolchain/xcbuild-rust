use std::env;
use std::process;
use xcbuild_pbxproj::{dump_group, PbxProject};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("usage: dump_xcodeproj <path.xcodeproj>");
        process::exit(1);
    }

    for path in &args {
        let project = match PbxProject::open(path) {
            Some(p) => p,
            None => {
                eprintln!("error: couldn't open '{path}'");
                process::exit(1);
            }
        };

        println!("Project: {}", project.name());
        println!("Path: {}", project.path);
        if let Some(v) = &project.archive_version {
            println!("Archive Version: {v}");
        }
        if let Some(v) = &project.object_version {
            println!("Object Version: {v}");
        }
        println!("Root Object: {}", project.root_object_id);
        println!("Objects: {}", project.objects.len());
        println!();

        // Dump targets
        let target_ids = project.target_ids();
        println!("Targets ({}):", target_ids.len());
        for tid in &target_ids {
            if let Some(obj) = project.object(tid) {
                let name = project.get_string(obj, "name").unwrap_or_default();
                let isa = project.get_string(obj, "isa").unwrap_or_default();
                let product_name = project
                    .get_string(obj, "productName")
                    .unwrap_or_default();
                let product_type = project
                    .get_string(obj, "productType")
                    .unwrap_or_default();
                println!("  {name} ({isa})");
                if !product_name.is_empty() {
                    println!("    productName: {product_name}");
                }
                if !product_type.is_empty() {
                    println!("    productType: {product_type}");
                }

                // Build configurations
                let config_list_id = project.get_string(obj, "buildConfigurationList");
                if let Some(cl_id) = config_list_id {
                    if let Some(cl_obj) = project.object(&cl_id) {
                        let default_config = project
                            .get_string(cl_obj, "defaultConfigurationName")
                            .unwrap_or_default();
                        let configs = project.get_array(cl_obj, "buildConfigurations");
                        println!("    Configurations (default: {default_config}):");
                        for c in &configs {
                            if let plist::Value::String(cid) = c {
                                if let Some(cobj) = project.object(cid) {
                                    let cname = project
                                        .get_string(cobj, "name")
                                        .unwrap_or_default();
                                    println!("      {cname}");
                                }
                            }
                        }
                    }
                }

                // Build phases
                let phases = project.get_array(obj, "buildPhases");
                if !phases.is_empty() {
                    println!("    Build Phases:");
                    for p in &phases {
                        if let plist::Value::String(pid) = p {
                            if let Some(pobj) = project.object(pid) {
                                let pisa = project
                                    .get_string(pobj, "isa")
                                    .unwrap_or_default();
                                let pname = project
                                    .get_string(pobj, "name")
                                    .unwrap_or_else(|| pisa.clone());
                                let files = project.get_array(pobj, "files");
                                println!("      {pname} ({pisa}, {} files)", files.len());
                            }
                        }
                    }
                }

                // Dependencies
                let deps = project.get_array(obj, "dependencies");
                if !deps.is_empty() {
                    println!("    Dependencies:");
                    for d in &deps {
                        if let plist::Value::String(did) = d {
                            if let Some(dobj) = project.object(did) {
                                let dname = project
                                    .get_string(dobj, "name")
                                    .unwrap_or_default();
                                if dname.is_empty() {
                                    println!("      {did}");
                                } else {
                                    println!("      {dname}");
                                }
                            }
                        }
                    }
                }
                println!();
            }
        }

        // Dump main group
        if let Some(group_id) = project.main_group_id() {
            println!("File tree:");
            dump_group(&project, &group_id, 1);
        }
    }
}
