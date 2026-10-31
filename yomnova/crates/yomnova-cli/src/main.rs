//! YOMNOVA Official Command Line Interface (CLI)
//! ONE LANGUAGE. EVERY SYSTEM.

use clap::{Parser, Subcommand};
use colored::Colorize;
use std::fs;
use std::path::PathBuf;
use std::process;
use yomnova_compiler::Compiler;

#[derive(Parser)]
#[command(name = "yomnova")]
#[command(author = "Mouhamed Salim Bousmina")]
#[command(version = "0.1.0-dev")]
#[command(about = "YOMNOVA — Original Hybrid Systems & Networking Programming Language", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile and run a YOMNOVA source file (.yom)
    Run {
        /// Source file path (.yom)
        file: PathBuf,
        /// Show disassembled bytecode before execution
        #[arg(long, short)]
        disassemble: bool,
    },
    /// Verify syntax, types, and ownership without execution
    Check {
        /// Source file path (.yom)
        file: PathBuf,
    },
    /// Compile source file to bytecode artifact (.yomc)
    Build {
        /// Source file path (.yom)
        file: PathBuf,
        /// Output artifact destination
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
    /// Format YOMNOVA source code
    Fmt {
        /// Source file path (.yom)
        file: PathBuf,
        /// Check formatting without overwriting file
        #[arg(long)]
        check: bool,
    },
    /// Display YOMNOVA toolchain version and build identity
    Version,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { file, disassemble } => {
            let filename = file.to_string_lossy().to_string();
            let source = match fs::read_to_string(&file) {
                Ok(content) => content,
                Err(e) => {
                    eprintln!("{}: Cannot read file '{}': {}", "error".red().bold(), filename, e);
                    process::exit(1);
                }
            };

            match Compiler::compile_and_run(&source) {
                Ok(output) => {
                    if disassemble {
                        println!("{}", output.chunk.disassemble(&filename).cyan());
                    }
                    // Program output has already been emitted during execution
                    if let Some(res) = output.execution_result {
                        if res != yomnova_bytecode::Value::Unit {
                            println!("=> {}", res);
                        }
                    }
                }
                Err(diagnostics) => {
                    for diag in diagnostics {
                        eprint!("{}", diag.render(Some(&source), Some(&filename)));
                    }
                    process::exit(1);
                }
            }
        }
        Commands::Check { file } => {
            let filename = file.to_string_lossy().to_string();
            let source = match fs::read_to_string(&file) {
                Ok(content) => content,
                Err(e) => {
                    eprintln!("{}: Cannot read file '{}': {}", "error".red().bold(), filename, e);
                    process::exit(1);
                }
            };

            match Compiler::check(&source) {
                Ok(_) => {
                    println!("{} Verification succeeded: 0 errors in '{}'", "success:".green().bold(), filename);
                }
                Err(diagnostics) => {
                    for diag in diagnostics {
                        eprint!("{}", diag.render(Some(&source), Some(&filename)));
                    }
                    process::exit(1);
                }
            }
        }
        Commands::Build { file, output } => {
            let filename = file.to_string_lossy().to_string();
            let source = match fs::read_to_string(&file) {
                Ok(content) => content,
                Err(e) => {
                    eprintln!("{}: Cannot read file '{}': {}", "error".red().bold(), filename, e);
                    process::exit(1);
                }
            };

            match Compiler::compile_and_run(&source) {
                Ok(out) => {
                    let out_path = output.unwrap_or_else(|| file.with_extension("yomc"));
                    let dis = out.chunk.disassemble(&filename);
                    if let Err(e) = fs::write(&out_path, dis) {
                        eprintln!("{}: Cannot write output '{}': {}", "error".red().bold(), out_path.display(), e);
                        process::exit(1);
                    }
                    println!("{} Built '{}' -> '{}'", "success:".green().bold(), filename, out_path.display());
                }
                Err(diagnostics) => {
                    for diag in diagnostics {
                        eprint!("{}", diag.render(Some(&source), Some(&filename)));
                    }
                    process::exit(1);
                }
            }
        }
        Commands::Fmt { file, check } => {
            let filename = file.to_string_lossy().to_string();
            let source = match fs::read_to_string(&file) {
                Ok(content) => content,
                Err(e) => {
                    eprintln!("{}: Cannot read file '{}': {}", "error".red().bold(), filename, e);
                    process::exit(1);
                }
            };

            let formatted = format_source(&source);
            if check {
                if formatted != source {
                    eprintln!("{}: '{}' requires reformatting", "warning:".yellow().bold(), filename);
                    process::exit(1);
                } else {
                    println!("{} '{}' is formatted correctly", "ok:".green().bold(), filename);
                }
            } else {
                if let Err(e) = fs::write(&file, formatted) {
                    eprintln!("{}: Failed to format file '{}': {}", "error".red().bold(), filename, e);
                    process::exit(1);
                }
                println!("{} Formatted '{}'", "success:".green().bold(), filename);
            }
        }
        Commands::Version => {
            println!("yomnova 0.1.0-dev (compiler: rustc 1.99.1, target: linux-x86_64)");
            println!("Architecture: Hybrid General-Purpose & Systems Language");
            println!("Creator: Mouhamed Salim Bousmina");
            println!("Identity: ONE LANGUAGE. EVERY SYSTEM.");
        }
    }
}

fn format_source(src: &str) -> String {
    let mut out = String::new();
    let mut indent: usize = 0;

    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }

        if trimmed.starts_with('}') {
            indent = indent.saturating_sub(1);
        }

        out.push_str(&"    ".repeat(indent));
        out.push_str(trimmed);
        out.push('\n');

        if trimmed.ends_with('{') {
            indent += 1;
        }
    }

    out
}
