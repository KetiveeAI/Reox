// REOX Template: NeolyxOS Application
// Generates complete app structure following APP_STRUCTURE.md

use std::fs;
use std::path::Path;
use super::ProjectConfig;

pub fn generate(config: &ProjectConfig, base_path: &Path) -> Result<(), String> {
    let app_dir = base_path.join(format!("{}.app", config.name));
    
    // Create directory structure
    let dirs = [
        "",
        "bin",
        "lib",
        "resources",
        "resources/icons",
        "resources/themes",
        "include",
        "src",
        "src/ui",
    ];
    
    for dir in &dirs {
        fs::create_dir_all(app_dir.join(dir))
            .map_err(|e| format!("Failed to create {}: {}", dir, e))?;
    }
    
    // main.rx - Entry point
    let main_rx = format!(r#"// {} - NeolyxOS Application
// Main Entry Point
// 
// Copyright (c) 2025 {}. All Rights Reserved.
// PROPRIETARY AND CONFIDENTIAL

import prelude;
import nxrender;
import transition;

// Import app modules
// import src.ui.main_window;

fn main() {{
    // Create application
    let app = app_new("{}");
    
    // Create main window
    let window = app_create_window(app, "{}", 800, 600);
    window_center(window);
    
    // Build UI
    let root = build_ui();
    window_set_root(window, root);
    
    // Run application
    app_run(app);
}}

fn build_ui() -> View {{
    // Main container
    let container = vstack(16.0);
    view_set_padding(container, insets_all(24.0));
    view_set_background(container, color_background());
    
    // Header
    let header = text_view("Welcome to {}");
    text_set_font_size(header, 28.0);
    text_set_font_weight(header, 700);
    text_set_color(header, color_text());
    view_add_child(container, header);
    
    // Subtitle
    let subtitle = text_view("Built with Reox for NeolyxOS");
    text_set_font_size(subtitle, 16.0);
    text_set_color(subtitle, color_text_secondary());
    view_add_child(container, subtitle);
    
    // Spacer
    view_add_child(container, spacer());
    
    // Action button
    let button = button_primary("Get Started");
    view_set_size(button, 200.0, 48.0);
    button_set_on_click(button, 1);
    view_add_child(container, button);
    
    // Animate entrance
    enter_fade(container, 300);
    
    return container;
}}

// Button click handler (callback_id = 1)
fn on_button_click() {{
    println("Button clicked!");
}}
"#, config.name, config.author, config.name, config.name, config.name);

    fs::write(app_dir.join("main.rx"), main_rx)
        .map_err(|e| format!("Failed to write main.rx: {}", e))?;
    
    // manifest.npa
    let manifest = format!(r#"{{
    "name": "{}",
    "version": "{}",
    "bundle_id": "{}",
    "category": "Utilities",
    "description": "{} application for NeolyxOS",
    
    "binary": "bin/{}",
    "library": "lib/lib{}.so",
    "icon": "resources/{}.nxi",
    
    "permissions": [
        "filesystem.read",
        "filesystem.write"
    ],
    
    "requirements": {{
        "os_version": "1.0.0",
        "drivers": ["nxgfx"]
    }},
    
    "author": "{}",
    "copyright": "Copyright (c) 2025 {}",
    "license": "Proprietary"
}}
"#, 
        config.name, 
        config.version, 
        config.bundle_id,
        config.name,
        config.name.to_lowercase(),
        config.name.to_lowercase(),
        config.name.to_lowercase(),
        config.author,
        config.author
    );

    fs::write(app_dir.join("manifest.npa"), manifest)
        .map_err(|e| format!("Failed to write manifest.npa: {}", e))?;
    
    // Makefile
    let makefile = format!(r#"# {} Makefile
# NeolyxOS Application Build

REOXC = reoxc
APP_NAME = {}
BINARY = bin/$(APP_NAME)

SOURCES = main.rx

# Compiler flags
REOX_FLAGS = -O2 --lto

.PHONY: all clean run package

all: $(BINARY)

$(BINARY): $(SOURCES)
	@mkdir -p bin
	$(REOXC) main.rx --emit exe -o $(BINARY) $(REOX_FLAGS)

run: $(BINARY)
	./$(BINARY)

clean:
	rm -rf bin/*.o bin/$(APP_NAME) *.c

# Package as .nxpkg for distribution
package: $(BINARY)
	@echo "Packaging {}.app..."
	@tar -czvf {}.nxpkg -C .. {}.app
	@echo "Created {}.nxpkg"

# Development mode (no optimization)
dev:
	$(REOXC) main.rx --emit exe -o $(BINARY) -O0

# Release build (full optimization + strip)
release:
	$(REOXC) main.rx --emit exe -o $(BINARY) -O3 --lto --strip
"#, 
        config.name,
        config.name.to_lowercase(),
        config.name,
        config.name.to_lowercase(),
        config.name,
        config.name.to_lowercase()
    );

    fs::write(app_dir.join("Makefile"), makefile)
        .map_err(|e| format!("Failed to write Makefile: {}", e))?;
    
    // README.md
    let readme = format!(r#"# {}

A NeolyxOS application built with Reox.

## Project Structure

```
{}.app/
├── main.rx           # Application entry point
├── Makefile          # Build configuration
├── manifest.npa      # App manifest
├── README.md         # This file
│
├── bin/              # Compiled binaries
├── lib/              # Shared libraries
├── resources/        # App resources
│   ├── {}.nxi       # App icon
│   └── themes/       # Theme files
├── include/          # Header files
└── src/              # Source files
    └── ui/           # UI components
```

## Build

```bash
# Development build
make dev

# Release build (optimized)
make release

# Run
make run
```

## Package for Distribution

```bash
make package
```

This creates `{}.nxpkg` for installation via NeolyxOS Installer.

## Requirements

- NeolyxOS 1.0.0+
- Reox Compiler (reoxc)
- NXRender drivers

## License

Copyright (c) 2025 {}. All Rights Reserved.
"#, 
        config.name,
        config.name,
        config.name.to_lowercase(),
        config.name.to_lowercase(),
        config.author
    );

    fs::write(app_dir.join("README.md"), readme)
        .map_err(|e| format!("Failed to write README.md: {}", e))?;
    
    // LICENSE
    let license = format!(r#"{} - NeolyxOS Application

Copyright (c) 2025 {}. All Rights Reserved.

PROPRIETARY AND CONFIDENTIAL

This software and its documentation are proprietary to {}.
No part of this software may be copied, modified, distributed,
or used without express written permission from {}.

For licensing inquiries, contact: support@ketivee.com
"#, config.name, config.author, config.author, config.author);

    fs::write(app_dir.join("LICENSE"), license)
        .map_err(|e| format!("Failed to write LICENSE: {}", e))?;
    
    // src/ui/main_window.rx
    let main_window_rx = format!(r#"// {} - Main Window UI
// UI component for the main application window

import prelude;
import transition;

/// Build the main window content
fn build_main_window() -> View {{
    let root = vstack(12.0);
    view_set_background(root, color_background());
    view_set_padding(root, insets_all(20.0));
    
    // Toolbar
    let toolbar = build_toolbar();
    view_add_child(root, toolbar);
    
    // Content area
    let content = build_content();
    view_add_child(root, content);
    
    // Status bar
    let status = build_status_bar();
    view_add_child(root, status);
    
    return root;
}}

fn build_toolbar() -> View {{
    let bar = hstack(8.0);
    view_set_height(bar, 48.0);
    view_set_background(bar, color_surface());
    view_set_padding(bar, insets_symmetric(8.0, 16.0));
    
    // Title
    let title = text_view("{}");
    text_set_font_weight(title, 600);
    view_add_child(bar, title);
    
    view_add_child(bar, spacer());
    
    // Settings button
    let settings = button_icon("settings");
    view_add_child(bar, settings);
    
    return bar;
}}

fn build_content() -> View {{
    let content = center();
    view_set_flex(content, 1.0, 0.0);
    
    let message = text_view("Content goes here");
    text_set_color(message, color_text_secondary());
    view_add_child(content, message);
    
    return content;
}}

fn build_status_bar() -> View {{
    let bar = hstack(8.0);
    view_set_height(bar, 24.0);
    view_set_padding(bar, insets_symmetric(4.0, 12.0));
    
    let status = text_view("Ready");
    text_set_font_size(status, 12.0);
    text_set_color(status, color_text_disabled());
    view_add_child(bar, status);
    
    return bar;
}}
"#, config.name, config.name);

    fs::write(app_dir.join("src").join("ui").join("main_window.rx"), main_window_rx)
        .map_err(|e| format!("Failed to write main_window.rx: {}", e))?;
    
    // resources/themes/light.theme
    let light_theme = "# Light Theme for NeolyxOS App\n\n[colors]\nbackground = #FFFFFF\nsurface = #F5F5F5\nprimary = #007AFF\ntext = #000000\ntext_secondary = #666666\n\n[fonts]\ndefault = system\nsize = 14\n";

    fs::write(app_dir.join("resources").join("themes").join("light.theme"), light_theme)
        .map_err(|e| format!("Failed to write light.theme: {}", e))?;
    
    // resources/themes/dark.theme
    let dark_theme = "# Dark Theme for NeolyxOS App\n\n[colors]\nbackground = #1C1C1E\nsurface = #2C2C2E\nprimary = #007AFF\ntext = #FFFFFF\ntext_secondary = #AEAEB2\n\n[fonts]\ndefault = system\nsize = 14\n";

    fs::write(app_dir.join("resources").join("themes").join("dark.theme"), dark_theme)
        .map_err(|e| format!("Failed to write dark.theme: {}", e))?;
    
    // .gitignore
    let gitignore = r#"# Build artifacts
bin/
*.o
*.c
*.nxpkg

# IDE
.vscode/
.idea/
*.swp
*.swo

# OS
.DS_Store
Thumbs.db
"#;

    fs::write(app_dir.join(".gitignore"), gitignore)
        .map_err(|e| format!("Failed to write .gitignore: {}", e))?;
    
    Ok(())
}
