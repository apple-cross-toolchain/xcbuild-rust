use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs;
use std::path::Path;

/// A reference to a buildable target.
#[derive(Debug, Clone)]
pub struct BuildableReference {
    pub build_product_type: Option<String>,
    pub blueprint_identifier: String,
    pub buildable_name: String,
    pub blueprint_name: String,
    pub referenced_container: String,
}

/// An entry in a build action.
#[derive(Debug, Clone)]
pub struct BuildActionEntry {
    pub build_for_running: bool,
    pub build_for_testing: bool,
    pub build_for_profiling: bool,
    pub build_for_archiving: bool,
    pub build_for_analyzing: bool,
    pub buildable_reference: Option<BuildableReference>,
}

/// Build action configuration.
#[derive(Debug, Clone)]
pub struct BuildAction {
    pub parallelize_buildables: bool,
    pub build_implicit_dependencies: bool,
    pub entries: Vec<BuildActionEntry>,
}

/// A testable reference in a test action.
#[derive(Debug, Clone)]
pub struct TestableReference {
    pub skipped: bool,
    pub buildable_reference: Option<BuildableReference>,
}

/// Test action configuration.
#[derive(Debug, Clone)]
pub struct TestAction {
    pub build_configuration: String,
    pub should_use_launch_scheme_args_env: bool,
    pub testables: Vec<TestableReference>,
}

/// Launch action configuration.
#[derive(Debug, Clone)]
pub struct LaunchAction {
    pub build_configuration: String,
    pub selected_debugger_identifier: Option<String>,
    pub selected_launcher_identifier: Option<String>,
    pub launch_style: Option<String>,
    pub use_custom_working_directory: bool,
    pub ignore_pers_debug_settings: bool,
    pub debug_document_versioning: bool,
    pub buildable_product_runnable: Option<BuildableReference>,
}

/// Profile action configuration.
#[derive(Debug, Clone)]
pub struct ProfileAction {
    pub build_configuration: String,
    pub should_use_launch_scheme_args_env: bool,
    pub buildable_product_runnable: Option<BuildableReference>,
}

/// Analyze action configuration.
#[derive(Debug, Clone)]
pub struct AnalyzeAction {
    pub build_configuration: String,
}

/// Archive action configuration.
#[derive(Debug, Clone)]
pub struct ArchiveAction {
    pub build_configuration: String,
    pub reveal_archive_in_organizer: bool,
}

/// A parsed .xcscheme file.
#[derive(Debug, Clone)]
pub struct Scheme {
    pub name: String,
    pub path: String,
    pub build_action: Option<BuildAction>,
    pub test_action: Option<TestAction>,
    pub launch_action: Option<LaunchAction>,
    pub profile_action: Option<ProfileAction>,
    pub analyze_action: Option<AnalyzeAction>,
    pub archive_action: Option<ArchiveAction>,
}

impl Scheme {
    /// Open and parse a .xcscheme file.
    pub fn open(path: &str) -> Option<Scheme> {
        let contents = fs::read_to_string(path).ok()?;
        let name = Path::new(path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut scheme = Scheme {
            name,
            path: path.to_string(),
            build_action: None,
            test_action: None,
            launch_action: None,
            profile_action: None,
            analyze_action: None,
            archive_action: None,
        };

        let mut reader = Reader::from_str(&contents);

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    match tag.as_str() {
                        "BuildAction" => {
                            scheme.build_action = Some(parse_build_action(e, &mut reader));
                        }
                        "TestAction" => {
                            scheme.test_action = Some(parse_test_action(e, &mut reader));
                        }
                        "LaunchAction" => {
                            scheme.launch_action = Some(parse_launch_action(e, &mut reader));
                        }
                        "ProfileAction" => {
                            scheme.profile_action = Some(parse_profile_action(e, &mut reader));
                        }
                        "AnalyzeAction" => {
                            scheme.analyze_action = Some(parse_analyze_action(e, &mut reader));
                        }
                        "ArchiveAction" => {
                            scheme.archive_action = Some(parse_archive_action(e, &mut reader));
                        }
                        _ => {}
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
        }

        Some(scheme)
    }
}

fn get_attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == key.as_bytes() {
            return Some(String::from_utf8_lossy(&attr.value).to_string());
        }
    }
    None
}

fn attr_bool(e: &quick_xml::events::BytesStart, key: &str, default: bool) -> bool {
    match get_attr(e, key).as_deref() {
        Some("YES") => true,
        Some("NO") => false,
        _ => default,
    }
}

fn parse_buildable_reference(reader: &mut Reader<&[u8]>) -> Option<BuildableReference> {
    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "BuildableReference" {
                    let br = BuildableReference {
                        build_product_type: get_attr(e, "BuildProductType"),
                        blueprint_identifier: get_attr(e, "BlueprintIdentifier")
                            .unwrap_or_default(),
                        buildable_name: get_attr(e, "BuildableName").unwrap_or_default(),
                        blueprint_name: get_attr(e, "BlueprintName").unwrap_or_default(),
                        referenced_container: get_attr(e, "ReferencedContainer")
                            .unwrap_or_default(),
                    };
                    return Some(br);
                }
            }
            Ok(Event::End(_)) => return None,
            Ok(Event::Eof) => return None,
            Err(_) => return None,
            _ => {}
        }
    }
}

fn parse_build_action(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> BuildAction {
    let parallelize = attr_bool(e, "parallelizeBuildables", true);
    let implicit_deps = attr_bool(e, "buildImplicitDependencies", true);
    let mut entries = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "BuildActionEntry" {
                    let entry = BuildActionEntry {
                        build_for_running: attr_bool(e, "buildForRunning", true),
                        build_for_testing: attr_bool(e, "buildForTesting", true),
                        build_for_profiling: attr_bool(e, "buildForProfiling", true),
                        build_for_archiving: attr_bool(e, "buildForArchiving", true),
                        build_for_analyzing: attr_bool(e, "buildForAnalyzing", true),
                        buildable_reference: parse_buildable_reference(reader),
                    };
                    entries.push(entry);
                }
            }
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == b"BuildAction" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    BuildAction {
        parallelize_buildables: parallelize,
        build_implicit_dependencies: implicit_deps,
        entries,
    }
}

fn parse_test_action(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> TestAction {
    let build_configuration = get_attr(e, "buildConfiguration").unwrap_or_else(|| "Debug".into());
    let should_use = attr_bool(e, "shouldUseLaunchSchemeArgsEnv", true);
    let mut testables = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "TestableReference" {
                    let skipped = attr_bool(e, "skipped", false);
                    let br = parse_buildable_reference(reader);
                    testables.push(TestableReference {
                        skipped,
                        buildable_reference: br,
                    });
                }
            }
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == b"TestAction" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    TestAction {
        build_configuration,
        should_use_launch_scheme_args_env: should_use,
        testables,
    }
}

fn parse_launch_action(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> LaunchAction {
    let build_configuration = get_attr(e, "buildConfiguration").unwrap_or_else(|| "Debug".into());
    let debugger = get_attr(e, "selectedDebuggerIdentifier");
    let launcher = get_attr(e, "selectedLauncherIdentifier");
    let launch_style = get_attr(e, "launchStyle");
    let use_custom_wd = attr_bool(e, "useCustomWorkingDirectory", false);
    let ignore_pers = attr_bool(e, "ignoresPersistentStateOnLaunch", false);
    let debug_doc = attr_bool(e, "debugDocumentVersioning", true);
    let mut runnable = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "BuildableProductRunnable" || tag == "PathRunnable" {
                    runnable = parse_buildable_reference(reader);
                }
            }
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == b"LaunchAction" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    LaunchAction {
        build_configuration,
        selected_debugger_identifier: debugger,
        selected_launcher_identifier: launcher,
        launch_style,
        use_custom_working_directory: use_custom_wd,
        ignore_pers_debug_settings: ignore_pers,
        debug_document_versioning: debug_doc,
        buildable_product_runnable: runnable,
    }
}

fn parse_profile_action(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> ProfileAction {
    let build_configuration =
        get_attr(e, "buildConfiguration").unwrap_or_else(|| "Release".into());
    let should_use = attr_bool(e, "shouldUseLaunchSchemeArgsEnv", true);
    let mut runnable = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "BuildableProductRunnable" {
                    runnable = parse_buildable_reference(reader);
                }
            }
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == b"ProfileAction" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    ProfileAction {
        build_configuration,
        should_use_launch_scheme_args_env: should_use,
        buildable_product_runnable: runnable,
    }
}

fn parse_analyze_action(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> AnalyzeAction {
    let build_configuration = get_attr(e, "buildConfiguration").unwrap_or_else(|| "Debug".into());

    // Skip to end of AnalyzeAction
    loop {
        match reader.read_event() {
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == b"AnalyzeAction" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    AnalyzeAction {
        build_configuration,
    }
}

fn parse_archive_action(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> ArchiveAction {
    let build_configuration =
        get_attr(e, "buildConfiguration").unwrap_or_else(|| "Release".into());
    let reveal = attr_bool(e, "revealArchiveInOrganizer", true);

    // Skip to end of ArchiveAction
    loop {
        match reader.read_event() {
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == b"ArchiveAction" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    ArchiveAction {
        build_configuration,
        reveal_archive_in_organizer: reveal,
    }
}

/// Find all .xcscheme files in a scheme directory.
pub fn find_schemes(schemes_dir: &str) -> Vec<String> {
    let path = Path::new(schemes_dir);
    if !path.is_dir() {
        return Vec::new();
    }
    let mut schemes = Vec::new();
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().map(|e| e == "xcscheme").unwrap_or(false) {
                schemes.push(p.to_string_lossy().to_string());
            }
        }
    }
    schemes.sort();
    schemes
}

/// Dump a scheme for display.
pub fn dump_scheme(scheme: &Scheme) {
    println!("Scheme: {} ({})", scheme.name, scheme.path);

    if let Some(build) = &scheme.build_action {
        println!("  BuildAction:");
        println!(
            "    parallelizeBuildables: {}",
            build.parallelize_buildables
        );
        println!(
            "    buildImplicitDependencies: {}",
            build.build_implicit_dependencies
        );
        for entry in &build.entries {
            print!("    Entry:");
            if entry.build_for_running {
                print!(" run");
            }
            if entry.build_for_testing {
                print!(" test");
            }
            if entry.build_for_profiling {
                print!(" profile");
            }
            if entry.build_for_archiving {
                print!(" archive");
            }
            if entry.build_for_analyzing {
                print!(" analyze");
            }
            println!();
            if let Some(br) = &entry.buildable_reference {
                println!(
                    "      {} ({}) in {}",
                    br.blueprint_name, br.buildable_name, br.referenced_container
                );
            }
        }
    }

    if let Some(test) = &scheme.test_action {
        println!("  TestAction: config={}", test.build_configuration);
        for tr in &test.testables {
            let skip = if tr.skipped { " [skipped]" } else { "" };
            if let Some(br) = &tr.buildable_reference {
                println!("    Testable: {}{}", br.blueprint_name, skip);
            }
        }
    }

    if let Some(launch) = &scheme.launch_action {
        println!("  LaunchAction: config={}", launch.build_configuration);
        if let Some(br) = &launch.buildable_product_runnable {
            println!("    Runnable: {} ({})", br.blueprint_name, br.buildable_name);
        }
    }

    if let Some(profile) = &scheme.profile_action {
        println!("  ProfileAction: config={}", profile.build_configuration);
    }

    if let Some(analyze) = &scheme.analyze_action {
        println!("  AnalyzeAction: config={}", analyze.build_configuration);
    }

    if let Some(archive) = &scheme.archive_action {
        println!(
            "  ArchiveAction: config={}, reveal={}",
            archive.build_configuration, archive.reveal_archive_in_organizer
        );
    }
}
