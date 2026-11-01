//! YOMNOVA command line interface.
//! ONE LANGUAGE. EVERY SYSTEM.
//!
//! Argument parsing is hand-written so the CLI has no third-party dependencies.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use yomnova_bytecode::Value;
use yomnova_compiler::{format::format_source, Compiler};
use yomnova_diagnostics::Diagnostic;

const USAGE: &str = "\
yomnova - experimental hybrid programming language toolchain

USAGE:
    yomnova <COMMAND> [OPTIONS] <FILE>

COMMANDS:
    run [-d|--disassemble] <FILE>   Compile and run a program
    check <FILE>                    Lex, parse, type-check and ownership-check only
    build [-o <OUT>] <FILE>         Compile to a textual bytecode listing (does not run the program)
    disasm <FILE>                   Print the bytecode listing to stdout (does not run the program)
    tokens <FILE>                   Print the token stream
    ast <FILE>                      Print the syntax tree
    fmt [--check] <FILE>            Re-indent a source file (in place unless --check)
    version                         Print version information
    help                            Print this message
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match dispatch(&args) {
        Ok(code) => code,
        Err(CliError::Usage(msg)) => {
            eprintln!("error: {msg}\n\n{USAGE}");
            ExitCode::from(2)
        }
        Err(CliError::Failure(msg)) => {
            eprintln!("error: {msg}");
            ExitCode::from(1)
        }
        Err(CliError::Diagnostics {
            diags,
            source,
            file,
        }) => {
            for d in diags {
                eprint!("{}", d.render(Some(&source), Some(&file)));
            }
            ExitCode::from(1)
        }
    }
}

enum CliError {
    Usage(String),
    Failure(String),
    Diagnostics {
        diags: Vec<Diagnostic>,
        source: String,
        file: String,
    },
}

/// Options shared by all subcommands, parsed from the arguments after the command name.
#[derive(Default)]
struct Opts {
    file: Option<PathBuf>,
    disassemble: bool,
    check: bool,
    output: Option<PathBuf>,
}

fn parse_opts(command: &str, rest: &[String]) -> Result<Opts, CliError> {
    let mut opts = Opts::default();
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        match (command, arg.as_str()) {
            ("run", "-d" | "--disassemble") => opts.disassemble = true,
            ("fmt", "--check") => opts.check = true,
            ("build", "-o" | "--output") => {
                let value = it
                    .next()
                    .ok_or_else(|| CliError::Usage(format!("{arg} requires a value")))?;
                opts.output = Some(PathBuf::from(value));
            }
            (_, a) if a.starts_with('-') && a.len() > 1 => {
                return Err(CliError::Usage(format!(
                    "unknown option '{a}' for '{command}'"
                )));
            }
            _ => {
                if opts.file.is_some() {
                    return Err(CliError::Usage(format!(
                        "unexpected extra argument '{arg}'"
                    )));
                }
                opts.file = Some(PathBuf::from(arg));
            }
        }
    }
    Ok(opts)
}

fn read_source(path: &Path) -> Result<(String, String), CliError> {
    let name = path.to_string_lossy().to_string();
    match fs::read_to_string(path) {
        Ok(s) => Ok((s, name)),
        Err(e) => Err(CliError::Failure(format!("Cannot read file '{name}': {e}"))),
    }
}

fn dispatch(args: &[String]) -> Result<ExitCode, CliError> {
    let Some(command) = args.first().map(String::as_str) else {
        print!("{USAGE}");
        return Ok(ExitCode::from(2));
    };
    let rest = &args[1..];

    match command {
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        "version" | "--version" | "-V" => {
            println!(
                "yomnova {} ({}-{})",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH
            );
            println!("ONE LANGUAGE. EVERY SYSTEM.");
            Ok(ExitCode::SUCCESS)
        }
        "run" | "check" | "build" | "disasm" | "tokens" | "ast" | "fmt" => {
            let opts = parse_opts(command, rest)?;
            let path = opts
                .file
                .clone()
                .ok_or_else(|| CliError::Usage(format!("'{command}' requires a source file")))?;
            let (source, file) = read_source(&path)?;
            let diag = |diags: Vec<Diagnostic>| CliError::Diagnostics {
                diags,
                source: source.clone(),
                file: file.clone(),
            };

            match command {
                "run" => {
                    let out = Compiler::compile_and_run(&source).map_err(diag)?;
                    if opts.disassemble {
                        println!("{}", out.disassemble_all());
                    }
                    if let Some(res) = out.execution_result {
                        if res != Value::Unit {
                            println!("=> {res}");
                        }
                    }
                }
                "check" => {
                    Compiler::check(&source).map_err(diag)?;
                    println!("ok: no errors in '{file}'");
                }
                "build" => {
                    let unit = Compiler::compile_unit(&source).map_err(diag)?;
                    let out_path = opts.output.unwrap_or_else(|| path.with_extension("yomc"));
                    fs::write(&out_path, unit.disassemble_all()).map_err(|e| {
                        CliError::Failure(format!(
                            "Cannot write output '{}': {e}",
                            out_path.display()
                        ))
                    })?;
                    println!(
                        "ok: wrote textual bytecode listing '{}' (not an executable artifact)",
                        out_path.display()
                    );
                }
                "disasm" => {
                    let unit = Compiler::compile_unit(&source).map_err(diag)?;
                    print!("{}", unit.disassemble_all());
                }
                "tokens" => {
                    let tokens = Compiler::tokens(&source).map_err(diag)?;
                    print!("{}", Compiler::render_tokens(&tokens));
                }
                "ast" => {
                    println!("{}", Compiler::ast_dump(&source).map_err(diag)?);
                }
                "fmt" => {
                    let formatted = format_source(&source);
                    if opts.check {
                        if formatted != source {
                            eprintln!("'{file}' requires reformatting");
                            return Ok(ExitCode::from(1));
                        }
                        println!("ok: '{file}' is formatted");
                    } else {
                        fs::write(&path, formatted).map_err(|e| {
                            CliError::Failure(format!("Failed to write '{file}': {e}"))
                        })?;
                        println!("ok: formatted '{file}'");
                    }
                }
                _ => unreachable!(),
            }
            Ok(ExitCode::SUCCESS)
        }
        other => Err(CliError::Usage(format!("unknown command '{other}'"))),
    }
}
