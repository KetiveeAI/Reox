// REOX Compiler - Main Entry Point

use std::process;

use reoxc::{cli, pkg, templates};

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
        cli::CliCommand::Pkg(args) => {
            let cmd = pkg::parse_pkg_args(&args)?;
            pkg::handle_pkg(cmd)
        }
        cli::CliCommand::Fmt(args) => {
            let code = reoxc::fmt::run(&args)?;
            if code != 0 {
                process::exit(code);
            }
            Ok(())
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
    let template = templates::Template::from_name(template_name)
        .ok_or_else(|| format!(
            "unknown template: '{}'. Available: {:?}",
            template_name,
            templates::Template::list()
        ))?;
    
    let project_name: String = match name {
        Some(name) => name.to_string(),
        // Use current directory name
        None => std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "MyApp".to_string()),
    };
    let project_name = project_name.as_str();
    
    let config = templates::ProjectConfig::new(project_name);
    let base_path = std::env::current_dir()
        .map_err(|e| format!("failed to get current directory: {}", e))?;
    
    templates::create_project(template, &config, &base_path)?;
    
    println!("Created {} project: {}", template_name, project_name);
    println!();
    println!("  Next steps:");
    println!("    cd {}.app", project_name);
    println!("    make");
    println!("    make run");
    
    Ok(())
}

fn new_project(name: &str, template_name: &str) -> Result<(), String> {
    let template = templates::Template::from_name(template_name)
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
    
    println!("Created {} project: {}", template_name, name);
    println!();
    println!("  Next steps:");
    println!("    cd {}", project_dir);
    println!("    make");
    println!("    make run");
    
    Ok(())
}

fn run(args: &cli::Args) -> Result<(), String> {
    let (ast, sources) = match cli::check_file(std::path::Path::new(&args.input)) {
        Ok(checked) => checked,
        Err((diagnostics, sources)) => {
            cli::report_program_diagnostics(&sources, &diagnostics);
            return Err("compilation failed".to_string());
        }
    };

    if let Err(e) = reoxc::interpreter::eval(&ast) {
        cli::report_program_diagnostics(&sources, &[e.into()]);
        return Err("program terminated with a runtime error".to_string());
    }

    Ok(())
}

fn compile(args: &cli::Args) -> Result<(), String> {
    let output_path = args.output.clone()
        .unwrap_or_else(|| {
            let stem = std::path::Path::new(&args.input)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            format!("{}.c", stem)
        });

    let c_code = match cli::compile_file(std::path::Path::new(&args.input)) {
        Ok((c_code, _sources)) => c_code,
        Err((diagnostics, sources)) => {
            cli::report_program_diagnostics(&sources, &diagnostics);
            return Err("compilation failed".to_string());
        }
    };

    std::fs::write(&output_path, c_code)
        .map_err(|e| format!("failed to write '{}': {}", output_path, e))?;

    println!("compiled: {} -> {}", args.input, output_path);

    Ok(())
}
