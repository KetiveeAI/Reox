// REOX Compiler - CLI Argument Parser
// Self-built, zero external dependencies
// Supports full compilation pipeline with LTO + Project Templates

use std::env;
use std::process::Command;

use std::path::Path;

use crate::diagnostic::{render_all, Diagnostic};
use crate::modules::{self, SourceMap};
use crate::parser::Ast;

/// CLI Command
#[derive(Debug, Clone)]
pub enum CliCommand {
    Compile(Args),
    Init { template: String, name: Option<String> },
    New { name: String, template: String },
    Pkg(Vec<String>),
    Fmt(Vec<String>),
    Help,
    Version,
}

/// Compiler arguments
#[derive(Debug, Clone)]
pub struct Args {
    pub input: String,
    pub output: Option<String>,
    pub emit: EmitType,
    pub opt_level: OptLevel,
    pub lto: bool,
    pub strip: bool,
    pub verbose: bool,
    pub runtime_path: Option<String>,
    pub run: bool,
    pub target: Target,
}

/// Platform the generated C is built for.
///
/// `Host` keeps the hosted prologue (`reox_runtime.h` plus the NXRender
/// bridge) that links against `librx_linux.a`. `Neolyx` emits C for the
/// NeolyxOS freestanding userspace: only `<stdint.h>`, `<stdbool.h>`,
/// `<stddef.h>` and `rx_core.h`, linked against `librx_neolyx.a` by the
/// NeolyxOS SDK (`sdk/reox.mk`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Target {
    #[default]
    Host,
    Neolyx,
}

impl Target {
    pub fn parse(name: &str) -> Result<Target, String> {
        match name {
            "host" => Ok(Target::Host),
            "neolyx" | "neolyxos" => Ok(Target::Neolyx),
            other => Err(format!("unknown target: '{}' (expected host or neolyx)", other)),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Target::Host => "host",
            Target::Neolyx => "neolyx",
        }
    }
}

/// Output type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmitType {
    #[default]
    C,      // Generate C code only
    Obj,    // Compile to object file
    Exe,    // Compile to executable
}

/// Optimization level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptLevel {
    O0,  // No optimization
    O1,  // Basic optimization
    #[default]
    O2,  // Standard optimization
    O3,  // Aggressive optimization
    Os,  // Size optimization
}

impl OptLevel {
    pub fn to_flag(&self) -> &'static str {
        match self {
            OptLevel::O0 => "-O0",
            OptLevel::O1 => "-O1",
            OptLevel::O2 => "-O2",
            OptLevel::O3 => "-O3",
            OptLevel::Os => "-Os",
        }
    }
}

/// Parse command line arguments
pub fn parse_cli() -> Result<CliCommand, String> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        return Ok(CliCommand::Help);
    }

    // Check for subcommands first
    match args[1].as_str() {
        "init" => return parse_init(&args[2..]),
        "new" => return parse_new(&args[2..]),
        "pkg" => return Ok(CliCommand::Pkg(args[2..].to_vec())),
        "fmt" => return Ok(CliCommand::Fmt(args[2..].to_vec())),
        "help" | "--help" | "-h" => return Ok(CliCommand::Help),
        "version" | "--version" | "-V" => return Ok(CliCommand::Version),
        // `reoxc compile <file>` and `reoxc run <file>` spell out the mode.
        "compile" => return parse_compile_args(&args[2..]).map(CliCommand::Compile),
        "run" => {
            return parse_compile_args(&args[2..]).map(|mut a| {
                a.run = true;
                CliCommand::Compile(a)
            })
        }
        _ => {}
    }

    // Fall through to compile command
    parse_compile_args(&args[1..]).map(CliCommand::Compile)
}

fn parse_init(args: &[String]) -> Result<CliCommand, String> {
    let mut template = "neolyx-app".to_string();
    let mut name: Option<String> = None;
    
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-t" | "--template" => {
                i += 1;
                if i >= args.len() {
                    return Err("expected template name after --template".to_string());
                }
                template = args[i].clone();
            }
            "--help" | "-h" => {
                print_init_help();
                std::process::exit(0);
            }
            _ => {
                if !args[i].starts_with('-') && name.is_none() {
                    name = Some(args[i].clone());
                }
            }
        }
        i += 1;
    }
    
    Ok(CliCommand::Init { template, name })
}

fn parse_new(args: &[String]) -> Result<CliCommand, String> {
    if args.is_empty() {
        return Err("project name required. Usage: reoxc new <name> [--template <type>]".to_string());
    }
    
    let mut template = "neolyx-app".to_string();
    let mut name: Option<String> = None;
    
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-t" | "--template" => {
                i += 1;
                if i >= args.len() {
                    return Err("expected template name after --template".to_string());
                }
                template = args[i].clone();
            }
            "--help" | "-h" => {
                print_new_help();
                std::process::exit(0);
            }
            _ => {
                if !args[i].starts_with('-') && name.is_none() {
                    name = Some(args[i].clone());
                }
            }
        }
        i += 1;
    }
    
    let name = name.ok_or("project name required")?;
    Ok(CliCommand::New { name, template })
}

fn parse_compile_args(args: &[String]) -> Result<Args, String> {
    let mut input: Option<String> = None;
    let mut output: Option<String> = None;
    let mut emit = EmitType::C;
    let mut opt_level = OptLevel::O2;
    let mut lto = false;
    let mut strip = false;
    let mut verbose = false;
    let mut runtime_path: Option<String> = None;
    let mut run = false;
    let mut target = Target::Host;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        match arg.as_str() {
            "-o" | "--output" => {
                i += 1;
                if i >= args.len() {
                    return Err("expected output file after -o".to_string());
                }
                output = Some(args[i].clone());
            }
            "--emit" => {
                i += 1;
                if i >= args.len() {
                    return Err("expected emit type after --emit".to_string());
                }
                emit = match args[i].as_str() {
                    "c" => EmitType::C,
                    "obj" => EmitType::Obj,
                    "exe" => EmitType::Exe,
                    other => return Err(format!("unknown emit type: {}", other)),
                };
            }
            "--target" => {
                i += 1;
                if i >= args.len() {
                    return Err("expected target after --target".to_string());
                }
                target = Target::parse(&args[i])?;
            }
            other if other.starts_with("--target=") => {
                target = Target::parse(&other["--target=".len()..])?;
            }
            "-O0" => opt_level = OptLevel::O0,
            "-O1" => opt_level = OptLevel::O1,
            "-O2" => opt_level = OptLevel::O2,
            "-O3" => opt_level = OptLevel::O3,
            "-Os" => opt_level = OptLevel::Os,
            "--lto" | "-flto" => lto = true,
            "--strip" | "-s" => strip = true,
            "--runtime" => {
                i += 1;
                if i >= args.len() {
                    return Err("expected runtime path after --runtime".to_string());
                }
                runtime_path = Some(args[i].clone());
            }
            "--run" | "-r" => run = true,
            "-v" | "--verbose" => verbose = true,
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("reoxc {} ({} {})", 
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::OS,
                    std::env::consts::ARCH
                );
                std::process::exit(0);
            }
            _ => {
                if arg.starts_with('-') {
                    return Err(format!("unknown option: {}", arg));
                }
                if input.is_some() {
                    return Err("multiple input files not supported".to_string());
                }
                input = Some(arg.clone());
            }
        }

        i += 1;
    }

    let input = input.ok_or("no input file specified")?;
    
    // Validate file extension (.rx or .reox)
    if !input.ends_with(".rx") && !input.ends_with(".reox") {
        return Err(format!(
            "invalid file extension: '{}'. Expected .rx or .reox",
            input
        ));
    }

    Ok(Args {
        input,
        output,
        emit,
        opt_level,
        lto,
        strip,
        verbose,
        runtime_path,
        run,
        target,
    })
}

/// Legacy function for backwards compatibility
pub fn parse_args() -> Result<Args, String> {
    match parse_cli()? {
        CliCommand::Compile(args) => Ok(args),
        CliCommand::Help => {
            print_usage();
            std::process::exit(0);
        }
        CliCommand::Version => {
            println!("reoxc {} ({} {})", 
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH
            );
            std::process::exit(0);
        }
        _ => Err("use parse_cli() for init/new commands".to_string()),
    }
}

fn print_init_help() {
    println!("reoxc init - Initialize a new Reox project");
    println!();
    println!("USAGE:");
    println!("    reoxc init [OPTIONS] [NAME]");
    println!();
    println!("OPTIONS:");
    println!("    -t, --template <TYPE>  Project template (default: neolyx-app)");
    println!("    -h, --help             Show this help");
    println!();
    println!("TEMPLATES:");
    println!("    neolyx-app    NeolyxOS GUI application (default)");
    println!("    cli           Command-line tool");
    println!("    library       Reusable library");
    println!();
    println!("EXAMPLES:");
    println!("    reoxc init                           # Create neolyx-app in current dir");
    println!("    reoxc init MyApp                     # Create MyApp.app");
    println!("    reoxc init --template cli MyCLI      # Create CLI project");
}

fn print_new_help() {
    println!("reoxc new - Create a new named Reox project");
    println!();
    println!("USAGE:");
    println!("    reoxc new <NAME> [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    -t, --template <TYPE>  Project template (default: neolyx-app)");
    println!("    -h, --help             Show this help");
    println!();
    println!("TEMPLATES:");
    println!("    neolyx-app    NeolyxOS GUI application (default)");
    println!("    cli           Command-line tool");
    println!("    library       Reusable library");
    println!();
    println!("EXAMPLES:");
    println!("    reoxc new MyApp                      # Create MyApp.app");
    println!("    reoxc new MyCLI --template cli       # Create CLI project");
    println!("    reoxc new MyLib --template library   # Create library project");
}

/// Print usage information
pub fn print_usage() {
    println!("reoxc - REOX Language Compiler for NeolyxOS");
    println!();
    println!("USAGE:");
    println!("    reoxc <COMMAND>");
    println!("    reoxc [OPTIONS] <INPUT>");
    println!();
    println!("COMMANDS:");
    println!("    compile <file> Compile a program (and its imports) to C");
    println!("    run <file>     Run a program in the interpreter");
    println!("    init          Initialize a new project in current directory");
    println!("    new <name>    Create a new named project");
    println!("    pkg           Package manager (add, remove, list)");
    println!("    fmt           Format source files (--check to verify)");
    println!("    help          Show this help message");
    println!("    version       Show version information");
    println!();
    println!("COMPILE OPTIONS:");
    println!("    -o, --output <FILE>    Output file path");
    println!("    --emit <TYPE>          Output type: c, obj, exe (default: c)");
    println!("    --target <TARGET>      host (default) or neolyx (freestanding NeolyxOS C;");
    println!("                           only --emit c, built by the NeolyxOS SDK)");
    println!();
    println!("  Optimization:");
    println!("    -O0                    No optimization");
    println!("    -O1                    Basic optimization");
    println!("    -O2                    Standard optimization (default)");
    println!("    -O3                    Aggressive optimization");
    println!("    -Os                    Optimize for size");
    println!("    --lto                  Enable Link-Time Optimization");
    println!("    --strip                Strip symbols from output");
    println!();
    println!("  Other:");
    println!("    --runtime <PATH>       Path to runtime library");
    println!("    --run, -r              Run immediately (interpreter mode)");
    println!("    -v, --verbose          Verbose output");
    println!("    -h, --help             Print help information");
    println!("    -V, --version          Print version information");
    println!();
    println!("EXAMPLES:");
    println!("    reoxc new MyApp                      Create NeolyxOS app project");
    println!("    reoxc init --template cli            Initialize CLI project");
    println!("    reoxc main.rx -o main.c              Generate C code");
    println!("    reoxc app.reox --emit exe -o app     Compile to executable");
    println!("    reoxc main.rx --emit exe -O3 --lto   Full optimization");
    println!("    reoxc compile --emit c --target neolyx -o app.c app.rx");
    println!();
    println!("FILE EXTENSIONS:");
    println!("    .rx      REOX source file (short form)");
    println!("    .reox    REOX source file (full form)");
}

// === Pipeline drivers ===

/// Run the front end (lexer, parser, type checker) over `source`.
/// Every failure is returned as diagnostics; nothing is printed.
pub fn check_source(source: &str) -> Result<Ast, Vec<Diagnostic>> {
    let tokens = crate::lexer::tokenize(source).map_err(|e| vec![Diagnostic::from(e)])?;
    let ast = crate::parser::parse(&tokens).map_err(|e| vec![Diagnostic::from(e)])?;
    crate::typechecker::check(&ast)
        .map_err(|errors| errors.into_iter().map(Diagnostic::from).collect())
}

/// Run the whole compiler over `source` and return the generated C code.
pub fn compile_source(source: &str) -> Result<String, Vec<Diagnostic>> {
    compile_source_for(source, Target::Host)
}

/// `compile_source` for an explicit target.
pub fn compile_source_for(source: &str, target: Target) -> Result<String, Vec<Diagnostic>> {
    let ast = check_source(source)?;
    crate::codegen::generate_string_for(&ast, target).map_err(|e| vec![Diagnostic::from(e)])
}

/// Load the program rooted at `path` (resolving every `import`) and run the
/// front end over it. Both outcomes carry the source map, so diagnostics can
/// be rendered against the file they belong to.
pub fn check_file(path: &Path) -> Result<(Ast, SourceMap), (Vec<Diagnostic>, SourceMap)> {
    let program = modules::load_program(path).map_err(|e| (e.diagnostics, e.sources))?;
    check_program(program)
}

/// Like `check_file`, with the entry file's contents given instead of read.
pub fn check_file_with_source(path: &Path, source: &str) -> Result<(Ast, SourceMap), (Vec<Diagnostic>, SourceMap)> {
    let program = modules::load_program_from_source(path, source).map_err(|e| (e.diagnostics, e.sources))?;
    check_program(program)
}

fn check_program(program: modules::Program) -> Result<(Ast, SourceMap), (Vec<Diagnostic>, SourceMap)> {
    match crate::typechecker::check_modules(&program.ast, &program.modules) {
        Ok(ast) => Ok((ast, program.sources)),
        Err(errors) => Err((errors.into_iter().map(Diagnostic::from).collect(), program.sources)),
    }
}

/// Compile the program rooted at `path` and its imports to C.
pub fn compile_file(path: &Path) -> Result<(String, SourceMap), (Vec<Diagnostic>, SourceMap)> {
    compile_file_for(path, Target::Host)
}

/// `compile_file` for an explicit target.
pub fn compile_file_for(path: &Path, target: Target) -> Result<(String, SourceMap), (Vec<Diagnostic>, SourceMap)> {
    let (ast, sources) = check_file(path)?;
    match crate::codegen::generate_string_for(&ast, target) {
        Ok(c) => Ok((c, sources)),
        Err(e) => Err((vec![Diagnostic::from(e)], sources)),
    }
}

/// Print diagnostics of a multi-file program to stderr, each rendered
/// against its own file.
pub fn report_program_diagnostics(sources: &SourceMap, diagnostics: &[Diagnostic]) {
    if diagnostics.is_empty() {
        return;
    }
    eprintln!("{}", sources.render(diagnostics));
    let errors = diagnostics.iter().filter(|d| d.is_error()).count();
    if errors > 0 {
        eprintln!();
        eprintln!("{} error{} generated", errors, if errors == 1 { "" } else { "s" });
    }
}

/// Print diagnostics to stderr as `file:line:col: severity: message`, each
/// followed by the offending source line and a caret.
pub fn report_diagnostics(file: &str, source: &str, diagnostics: &[Diagnostic]) {
    if diagnostics.is_empty() {
        return;
    }
    eprintln!("{}", render_all(file, source, diagnostics));
    let errors = diagnostics.iter().filter(|d| d.is_error()).count();
    if errors > 0 {
        eprintln!();
        eprintln!("{} error{} generated", errors, if errors == 1 { "" } else { "s" });
    }
}

/// Compile C to executable
pub fn compile_c_to_exe(
    c_file: &str,
    output: &str,
    args: &Args,
) -> Result<(), String> {
    let mut cmd = Command::new("gcc");
    
    // Add optimization flags
    cmd.arg(args.opt_level.to_flag());
    
    if args.lto {
        cmd.arg("-flto");
    }
    
    // Add runtime include path
    if let Some(ref runtime) = args.runtime_path {
        cmd.arg("-I").arg(runtime);
        cmd.arg("-L").arg(runtime);
    }
    
    // Input and output
    cmd.arg("-o").arg(output);
    cmd.arg(c_file);
    
    // Link runtime and math
    if let Some(ref runtime) = args.runtime_path {
        cmd.arg(format!("{}/libreox_runtime.a", runtime));
    }
    cmd.arg("-lm");
    
    // Section garbage collection
    cmd.arg("-Wl,--gc-sections");
    
    if args.verbose {
        println!("[gcc] {:?}", cmd);
    }
    
    let status = cmd.status()
        .map_err(|e| format!("failed to run gcc: {}", e))?;
    
    if !status.success() {
        return Err(format!("gcc compilation failed with {}", status));
    }
    
    // Strip if requested
    if args.strip {
        let status = Command::new("strip")
            .arg(output)
            .status()
            .map_err(|e| format!("failed to run strip: {}", e))?;
        
        if !status.success() {
            return Err(format!("strip failed with {}", status));
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emit_type_default() {
        assert_eq!(EmitType::default(), EmitType::C);
    }

    #[test]
    fn test_opt_level_flags() {
        assert_eq!(OptLevel::O3.to_flag(), "-O3");
        assert_eq!(OptLevel::Os.to_flag(), "-Os");
    }

    #[test]
    fn check_source_accepts_valid_program() {
        let ast = check_source("fn main() -> int { return 1; }").expect("valid program");
        assert_eq!(ast.declarations.len(), 1);
    }

    #[test]
    fn check_source_reports_lex_error_with_span() {
        let diags = check_source("fn main() { let s = \"open; }").expect_err("unterminated string");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("unterminated string"));
        assert_eq!((diags[0].span.line, diags[0].span.column), (1, 21));
    }

    #[test]
    fn check_source_reports_parse_error_with_hint() {
        let diags = check_source("fn main() {").expect_err("unclosed block");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("expected '}'"), "{}", diags[0].message);
        assert!(diags[0].hint.is_some());
    }

    #[test]
    fn check_source_reports_every_type_error() {
        let diags = check_source("fn main() { let a: int = \"x\"; let b: bool = 1; }")
            .expect_err("two type errors");
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().all(|d| d.is_error()));
    }

    #[test]
    fn compile_source_produces_c() {
        let c = compile_source("fn main() -> int { return 42; }").expect("compiles");
        assert!(c.contains("int main(void)"));
        assert!(c.contains("return (int)(42);"));
    }

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn target_defaults_to_host() {
        let a = parse_compile_args(&strings(&["app.rx"])).expect("parses");
        assert_eq!(a.target, Target::Host);
    }

    #[test]
    fn target_flag_selects_neolyx() {
        let a = parse_compile_args(&strings(&["--emit", "c", "--target", "neolyx", "-o", "a.c", "app.rx"])).expect("parses");
        assert_eq!(a.target, Target::Neolyx);
        assert_eq!(a.output.as_deref(), Some("a.c"));
        let b = parse_compile_args(&strings(&["--target=neolyx", "app.rx"])).expect("parses");
        assert_eq!(b.target, Target::Neolyx);
    }

    #[test]
    fn target_flag_rejects_unknown_and_missing_values() {
        let e = parse_compile_args(&strings(&["--target", "windows", "app.rx"])).expect_err("unknown target");
        assert!(e.contains("unknown target"), "{}", e);
        assert!(parse_compile_args(&strings(&["app.rx", "--target"])).is_err());
    }

    #[test]
    fn neolyx_prologue_is_freestanding() {
        let c = compile_source_for("fn main() -> int { return 0; }", Target::Neolyx).expect("compiles");
        assert!(c.contains("#include \"rx_core.h\""));
        for hosted in ["reox_runtime.h", "reox_nxrender_bridge.h", "<stdio.h>", "<stdlib.h>", "<string.h>", "<setjmp.h>"] {
            assert!(!c.contains(hosted), "neolyx C must not include {}", hosted);
        }
        let host = compile_source("fn main() -> int { return 0; }").expect("compiles");
        assert!(host.contains("#include \"reox_runtime.h\""));
    }
}
