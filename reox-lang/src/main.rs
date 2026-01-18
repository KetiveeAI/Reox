// REOX Compiler - Main Entry Point
// Zero external dependencies

#![allow(unused_imports)]

mod lexer;
mod parser;
mod typechecker;
mod codegen;
mod cli;
mod interpreter;
mod stdlib;
mod templates;

use std::env;
use std::process;
use std::path::Path;

fn main() {
    let result = cli::parse_cli();

    match result {
        Ok(cmd) => {
            if let Err(e) = handle_command(cmd) {
                eprintln!("error: {}", e);
                process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("error: {}", e);
            eprintln!();
            cli::print_usage();
            process::exit(1);
        }
    }
}

fn handle_command(cmd: cli::CliCommand) -> Result<(), String> {
    match cmd {
        cli::CliCommand::Compile(args) => {
            if args.run {
                run(&args)
            } else {
                compile(&args)
            }
        }
        cli::CliCommand::Init { template, name } => {
            init_project(&template, name.as_deref())
        }
        cli::CliCommand::New { name, template } => {
            new_project(&name, &template)
        }
        cli::CliCommand::Help => {
            cli::print_usage();
            Ok(())
        }
        cli::CliCommand::Version => {
            println!("reoxc {} ({} {})", 
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH
            );
            Ok(())
        }
    }
}

fn init_project(template_name: &str, name: Option<&str>) -> Result<(), String> {
    let template = templates::Template::from_str(template_name)
        .ok_or_else(|| format!(
            "unknown template: '{}'. Available: {:?}",
            template_name,
            templates::Template::list()
        ))?;
    
    let project_name = name.unwrap_or_else(|| {
        // Use current directory name
        std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "MyApp".to_string())
            .leak()
    });
    
    let config = templates::ProjectConfig::new(project_name);
    let base_path = std::env::current_dir()
        .map_err(|e| format!("failed to get current directory: {}", e))?;
    
    templates::create_project(template, &config, &base_path)?;
    
    println!("✓ Created {} project: {}", template_name, project_name);
    println!();
    println!("  Next steps:");
    println!("    cd {}.app", project_name);
    println!("    make");
    println!("    make run");
    
    Ok(())
}

fn new_project(name: &str, template_name: &str) -> Result<(), String> {
    let template = templates::Template::from_str(template_name)
        .ok_or_else(|| format!(
            "unknown template: '{}'. Available: {:?}",
            template_name,
            templates::Template::list()
        ))?;
    
    let config = templates::ProjectConfig::new(name);
    let base_path = std::env::current_dir()
        .map_err(|e| format!("failed to get current directory: {}", e))?;
    
    templates::create_project(template, &config, &base_path)?;
    
    let project_dir = match template {
        templates::Template::NeolyxApp => format!("{}.app", name),
        _ => name.to_string(),
    };
    
    println!("✓ Created {} project: {}", template_name, name);
    println!();
    println!("  Next steps:");
    println!("    cd {}", project_dir);
    println!("    make");
    println!("    make run");
    
    Ok(())
}

fn run(args: &cli::Args) -> Result<(), String> {
    // Read source file
    let source = std::fs::read_to_string(&args.input)
        .map_err(|e| format!("failed to read '{}': {}", args.input, e))?;

    // Lexical analysis
    let tokens = lexer::tokenize(&source)
        .map_err(|e| e.display())?;

    // Parse
    let ast = parser::parse(&tokens);

    // Run interpreter
    if let Err(e) = interpreter::eval(&ast) {
        return Err(format!("runtime error: {}", e.message));
    }

    Ok(())
}

fn compile(args: &cli::Args) -> Result<(), String> {
    // Read source file
    let source = std::fs::read_to_string(&args.input)
        .map_err(|e| format!("failed to read '{}': {}", args.input, e))?;

    // Lexical analysis
    let tokens = lexer::tokenize(&source)
        .map_err(|e| e.display())?;

    // Parse
    let ast = parser::parse(&tokens);

    // Type check
    let checked_ast = typechecker::check(&ast);

    // Generate code
    let output_path = args.output.clone()
        .unwrap_or_else(|| {
            let stem = std::path::Path::new(&args.input)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            format!("{}.c", stem)
        });

    codegen::generate(&checked_ast, &output_path)
        .map_err(|e| format!("code generation failed: {}", e))?;

    println!("compiled: {} -> {}", args.input, output_path);

    Ok(())
}
