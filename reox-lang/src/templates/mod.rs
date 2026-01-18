// REOX Project Templates
// Generate project scaffolding for NeolyxOS apps

pub mod neolyx_app;

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub enum Template {
    NeolyxApp,
    Cli,
    Library,
}

impl Template {
    pub fn from_str(s: &str) -> Option<Template> {
        match s.to_lowercase().as_str() {
            "neolyx-app" | "neolyx_app" | "app" => Some(Template::NeolyxApp),
            "cli" | "command" => Some(Template::Cli),
            "lib" | "library" => Some(Template::Library),
            _ => None,
        }
    }
    
    pub fn list() -> Vec<&'static str> {
        vec!["neolyx-app", "cli", "library"]
    }
}

pub struct ProjectConfig {
    pub name: String,
    pub author: String,
    pub version: String,
    pub bundle_id: String,
}

impl ProjectConfig {
    pub fn new(name: &str) -> Self {
        let sanitized = name
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>();
        
        ProjectConfig {
            name: name.to_string(),
            author: "KetiveeAI".to_string(),
            version: "1.0.0".to_string(),
            bundle_id: format!("com.neolyx.{}", sanitized.to_lowercase()),
        }
    }
    
    pub fn with_author(mut self, author: &str) -> Self {
        self.author = author.to_string();
        self
    }
}

pub fn create_project(template: Template, config: &ProjectConfig, base_path: &Path) -> Result<(), String> {
    match template {
        Template::NeolyxApp => neolyx_app::generate(config, base_path),
        Template::Cli => generate_cli(config, base_path),
        Template::Library => generate_library(config, base_path),
    }
}

fn generate_cli(config: &ProjectConfig, base_path: &Path) -> Result<(), String> {
    let project_dir = base_path.join(&config.name);
    
    fs::create_dir_all(&project_dir)
        .map_err(|e| format!("Failed to create directory: {}", e))?;
    
    // main.rx
    let main_rx = format!(r#"// {} - Command Line Tool
// NeolyxOS CLI Application

import prelude;
import system;

fn main() {{
    let args = sys_args();
    
    if len(args) < 2 {{
        println("Usage: {} <command>");
        println("");
        println("Commands:");
        println("  help     Show this help");
        println("  version  Show version");
        return;
    }}
    
    let command = args[1];
    
    match command {{
        "help" => show_help(),
        "version" => println("{} v{}"),
        _ => {{
            println("Unknown command: " + command);
            sys_exit(1);
        }}
    }}
}}

fn show_help() {{
    println("{} - Command Line Tool");
    println("Copyright (c) 2025 {}");
}}
"#, config.name, config.name, config.name, config.version, config.name, config.author);

    fs::write(project_dir.join("main.rx"), main_rx)
        .map_err(|e| format!("Failed to write main.rx: {}", e))?;
    
    // Makefile
    let makefile = format!(r#"# {} Makefile

REOXC = reoxc
TARGET = {}
SRC = main.rx

.PHONY: all clean run

all: $(TARGET)

$(TARGET): $(SRC)
	$(REOXC) $(SRC) --emit exe -o $(TARGET) -O2

run: $(TARGET)
	./$(TARGET)

clean:
	rm -f $(TARGET) *.c *.o
"#, config.name, config.name.to_lowercase());

    fs::write(project_dir.join("Makefile"), makefile)
        .map_err(|e| format!("Failed to write Makefile: {}", e))?;
    
    // README.md
    let readme = format!(r#"# {}

A command-line tool for NeolyxOS.

## Build

```bash
make
```

## Usage

```bash
./{} help
./{} version
```

## License

Copyright (c) 2025 {}
"#, config.name, config.name.to_lowercase(), config.name.to_lowercase(), config.author);

    fs::write(project_dir.join("README.md"), readme)
        .map_err(|e| format!("Failed to write README.md: {}", e))?;
    
    Ok(())
}

fn generate_library(config: &ProjectConfig, base_path: &Path) -> Result<(), String> {
    let project_dir = base_path.join(&config.name);
    
    fs::create_dir_all(project_dir.join("src"))
        .map_err(|e| format!("Failed to create directory: {}", e))?;
    
    // lib.rx
    let lib_rx = format!(r#"// {} - Reox Library
// Copyright (c) 2025 {}

/// Library version
fn version() -> string {{
    return "{}";
}}

/// Add two integers
fn add(a: int, b: int) -> int {{
    return a + b;
}}

/// Multiply two integers
fn multiply(a: int, b: int) -> int {{
    return a * b;
}}
"#, config.name, config.author, config.version);

    fs::write(project_dir.join("src").join("lib.rx"), lib_rx)
        .map_err(|e| format!("Failed to write lib.rx: {}", e))?;
    
    // Makefile
    let makefile = format!(r#"# {} Library Makefile

REOXC = reoxc
LIB = lib{}.a
SRC = src/lib.rx

.PHONY: all clean

all: $(LIB)

$(LIB): $(SRC)
	$(REOXC) $(SRC) --emit obj -o lib.o
	ar rcs $(LIB) lib.o

clean:
	rm -f $(LIB) *.o *.c
"#, config.name, config.name.to_lowercase());

    fs::write(project_dir.join("Makefile"), makefile)
        .map_err(|e| format!("Failed to write Makefile: {}", e))?;
    
    // README.md
    let readme = format!(r#"# {}

A Reox library for NeolyxOS.

## Build

```bash
make
```

## Usage

```reox
import {};

let result = add(10, 20);
println(result);  // 30
```

## License

Copyright (c) 2025 {}
"#, config.name, config.name.to_lowercase(), config.author);

    fs::write(project_dir.join("README.md"), readme)
        .map_err(|e| format!("Failed to write README.md: {}", e))?;
    
    Ok(())
}
