// REOX Package Manager
// Manages project dependencies via reox.pkg manifests

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const MANIFEST_FILE: &str = "reox.pkg";

#[derive(Debug, Clone)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum PkgCommand {
    Init,
    Add { name: String, version: Option<String> },
    Remove { name: String },
    List,
    Info,
    Help,
}

impl Manifest {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            author: String::new(),
            description: String::new(),
            dependencies: BTreeMap::new(),
        }
    }

    pub fn parse(content: &str) -> Result<Self, String> {
        let mut manifest = Manifest::new("");
        let mut section = "";

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                section = match &line[1..line.len()-1] {
                    "package" => "package",
                    "dependencies" => "dependencies",
                    other => return Err(format!("unknown section: [{}]", other)),
                };
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');

                match section {
                    "package" => match key {
                        "name" => manifest.name = value.to_string(),
                        "version" => manifest.version = value.to_string(),
                        "author" => manifest.author = value.to_string(),
                        "description" => manifest.description = value.to_string(),
                        _ => {}
                    },
                    "dependencies" => {
                        manifest.dependencies.insert(key.to_string(), value.to_string());
                    },
                    _ => return Err(format!("key '{}' outside of section", key)),
                }
            }
        }

        if manifest.name.is_empty() {
            return Err("missing package name".to_string());
        }

        Ok(manifest)
    }

    pub fn serialize(&self) -> String {
        let mut out = String::new();
        out.push_str("[package]\n");
        out.push_str(&format!("name = \"{}\"\n", self.name));
        out.push_str(&format!("version = \"{}\"\n", self.version));
        if !self.author.is_empty() {
            out.push_str(&format!("author = \"{}\"\n", self.author));
        }
        if !self.description.is_empty() {
            out.push_str(&format!("description = \"{}\"\n", self.description));
        }
        out.push_str("\n[dependencies]\n");
        for (name, version) in &self.dependencies {
            out.push_str(&format!("{} = \"{}\"\n", name, version));
        }
        out
    }
}

fn find_manifest() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("failed to get cwd: {}", e))?;
    let path = cwd.join(MANIFEST_FILE);
    if path.exists() {
        Ok(path)
    } else {
        Err(format!("no {} found in current directory", MANIFEST_FILE))
    }
}

fn read_manifest() -> Result<Manifest, String> {
    let path = find_manifest()?;
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {}", MANIFEST_FILE, e))?;
    Manifest::parse(&content)
}

fn write_manifest(manifest: &Manifest) -> Result<(), String> {
    let path = find_manifest()?;
    fs::write(&path, manifest.serialize())
        .map_err(|e| format!("failed to write {}: {}", MANIFEST_FILE, e))
}

pub fn parse_pkg_args(args: &[String]) -> Result<PkgCommand, String> {
    if args.is_empty() {
        return Ok(PkgCommand::Help);
    }

    match args[0].as_str() {
        "init" => Ok(PkgCommand::Init),
        "add" => {
            if args.len() < 2 {
                return Err("package name required. Usage: reoxc pkg add <name> [version]".to_string());
            }
            let version = args.get(2).cloned();
            Ok(PkgCommand::Add { name: args[1].clone(), version })
        },
        "remove" | "rm" => {
            if args.len() < 2 {
                return Err("package name required. Usage: reoxc pkg remove <name>".to_string());
            }
            Ok(PkgCommand::Remove { name: args[1].clone() })
        },
        "list" | "ls" => Ok(PkgCommand::List),
        "info" => Ok(PkgCommand::Info),
        "help" | "--help" | "-h" => Ok(PkgCommand::Help),
        other => Err(format!("unknown pkg command: '{}'. Run 'reoxc pkg help' for usage.", other)),
    }
}

pub fn handle_pkg(cmd: PkgCommand) -> Result<(), String> {
    match cmd {
        PkgCommand::Init => cmd_init(),
        PkgCommand::Add { name, version } => cmd_add(&name, version.as_deref()),
        PkgCommand::Remove { name } => cmd_remove(&name),
        PkgCommand::List => cmd_list(),
        PkgCommand::Info => cmd_info(),
        PkgCommand::Help => { print_pkg_help(); Ok(()) },
    }
}

fn cmd_init() -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("failed to get cwd: {}", e))?;
    let path = cwd.join(MANIFEST_FILE);

    if path.exists() {
        return Err(format!("{} already exists", MANIFEST_FILE));
    }

    let project_name = cwd.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("myproject")
        .to_string();

    let manifest = Manifest::new(&project_name);
    fs::write(&path, manifest.serialize())
        .map_err(|e| format!("failed to create {}: {}", MANIFEST_FILE, e))?;

    println!("created {}", MANIFEST_FILE);
    println!();
    println!("  [package]");
    println!("  name = \"{}\"", project_name);
    println!("  version = \"0.1.0\"");
    println!();
    println!("  Add dependencies with: reoxc pkg add <name>");

    Ok(())
}

fn cmd_add(name: &str, version: Option<&str>) -> Result<(), String> {
    let mut manifest = read_manifest()?;
    let ver = version.unwrap_or("*").to_string();

    if manifest.dependencies.contains_key(name) {
        let old = manifest.dependencies.get(name).unwrap().clone();
        manifest.dependencies.insert(name.to_string(), ver.clone());
        write_manifest(&manifest)?;
        println!("updated {} {} -> {}", name, old, ver);
    } else {
        manifest.dependencies.insert(name.to_string(), ver.clone());
        write_manifest(&manifest)?;
        println!("added {} = \"{}\"", name, ver);
    }

    Ok(())
}

fn cmd_remove(name: &str) -> Result<(), String> {
    let mut manifest = read_manifest()?;

    if manifest.dependencies.remove(name).is_some() {
        write_manifest(&manifest)?;
        println!("removed {}", name);
    } else {
        return Err(format!("'{}' is not a dependency", name));
    }

    Ok(())
}

fn cmd_list() -> Result<(), String> {
    let manifest = read_manifest()?;

    println!("{} v{}", manifest.name, manifest.version);

    if manifest.dependencies.is_empty() {
        println!("  no dependencies");
    } else {
        println!("  dependencies:");
        for (name, version) in &manifest.dependencies {
            println!("    {} = \"{}\"", name, version);
        }
    }

    Ok(())
}

fn cmd_info() -> Result<(), String> {
    let manifest = read_manifest()?;

    println!("name:    {}", manifest.name);
    println!("version: {}", manifest.version);
    if !manifest.author.is_empty() {
        println!("author:  {}", manifest.author);
    }
    if !manifest.description.is_empty() {
        println!("desc:    {}", manifest.description);
    }
    println!("deps:    {}", manifest.dependencies.len());

    Ok(())
}

fn print_pkg_help() {
    println!("reoxc pkg - REOX Package Manager");
    println!();
    println!("USAGE:");
    println!("    reoxc pkg <COMMAND>");
    println!();
    println!("COMMANDS:");
    println!("    init                   Create a new reox.pkg manifest");
    println!("    add <name> [version]   Add or update a dependency");
    println!("    remove <name>          Remove a dependency");
    println!("    list                   List project dependencies");
    println!("    info                   Show package info");
    println!("    help                   Show this help");
    println!();
    println!("EXAMPLES:");
    println!("    reoxc pkg init");
    println!("    reoxc pkg add ui");
    println!("    reoxc pkg add network 0.8");
    println!("    reoxc pkg remove network");
    println!("    reoxc pkg list");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_new() {
        let m = Manifest::new("test");
        assert_eq!(m.name, "test");
        assert_eq!(m.version, "0.1.0");
        assert!(m.dependencies.is_empty());
    }

    #[test]
    fn test_manifest_serialize_parse_roundtrip() {
        let mut m = Manifest::new("myapp");
        m.version = "1.2.3".to_string();
        m.author = "dev".to_string();
        m.dependencies.insert("ui".to_string(), "1.0".to_string());
        m.dependencies.insert("net".to_string(), "*".to_string());

        let serialized = m.serialize();
        let parsed = Manifest::parse(&serialized).unwrap();

        assert_eq!(parsed.name, "myapp");
        assert_eq!(parsed.version, "1.2.3");
        assert_eq!(parsed.author, "dev");
        assert_eq!(parsed.dependencies.len(), 2);
        assert_eq!(parsed.dependencies.get("ui").unwrap(), "1.0");
        assert_eq!(parsed.dependencies.get("net").unwrap(), "*");
    }

    #[test]
    fn test_manifest_parse_minimal() {
        let content = "[package]\nname = \"hello\"\n\n[dependencies]\n";
        let m = Manifest::parse(content).unwrap();
        assert_eq!(m.name, "hello");
        assert_eq!(m.version, "0.1.0");
        assert!(m.dependencies.is_empty());
    }

    #[test]
    fn test_manifest_parse_error_no_name() {
        let content = "[package]\nversion = \"1.0\"\n\n[dependencies]\n";
        assert!(Manifest::parse(content).is_err());
    }

    #[test]
    fn test_manifest_parse_error_unknown_section() {
        let content = "[unknown]\nfoo = bar\n";
        assert!(Manifest::parse(content).is_err());
    }

    #[test]
    fn test_parse_pkg_args_init() {
        let args = vec!["init".to_string()];
        assert!(matches!(parse_pkg_args(&args), Ok(PkgCommand::Init)));
    }

    #[test]
    fn test_parse_pkg_args_add() {
        let args = vec!["add".to_string(), "ui".to_string()];
        if let Ok(PkgCommand::Add { name, version }) = parse_pkg_args(&args) {
            assert_eq!(name, "ui");
            assert!(version.is_none());
        } else { panic!("expected Add"); }
    }

    #[test]
    fn test_parse_pkg_args_add_with_version() {
        let args = vec!["add".to_string(), "ui".to_string(), "1.0".to_string()];
        if let Ok(PkgCommand::Add { name, version }) = parse_pkg_args(&args) {
            assert_eq!(name, "ui");
            assert_eq!(version.as_deref(), Some("1.0"));
        } else { panic!("expected Add"); }
    }

    #[test]
    fn test_parse_pkg_args_remove() {
        let args = vec!["remove".to_string(), "net".to_string()];
        if let Ok(PkgCommand::Remove { name }) = parse_pkg_args(&args) {
            assert_eq!(name, "net");
        } else { panic!("expected Remove"); }
    }

    #[test]
    fn test_parse_pkg_args_list() {
        let args = vec!["list".to_string()];
        assert!(matches!(parse_pkg_args(&args), Ok(PkgCommand::List)));
    }

    #[test]
    fn test_parse_pkg_args_empty() {
        assert!(matches!(parse_pkg_args(&[]), Ok(PkgCommand::Help)));
    }

    #[test]
    fn test_parse_pkg_args_add_no_name() {
        let args = vec!["add".to_string()];
        assert!(parse_pkg_args(&args).is_err());
    }

    #[test]
    fn test_manifest_comments_and_blank_lines() {
        let content = "# My project\n\n[package]\nname = \"test\"\n# comment\nversion = \"2.0\"\n\n[dependencies]\n# deps here\nui = \"1.0\"\n";
        let m = Manifest::parse(content).unwrap();
        assert_eq!(m.name, "test");
        assert_eq!(m.version, "2.0");
        assert_eq!(m.dependencies.get("ui").unwrap(), "1.0");
    }
}
