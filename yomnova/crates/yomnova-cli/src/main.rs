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
    run [-d] [--no-opt] <FILE>      Compile and run a program (--no-opt skips the IR optimiser)
    check <FILE>                    Lex, parse, type-check and ownership-check only
    build [-o <OUT>] <FILE>         Compile to a bytecode file (.yomc) that `exec` can run later
    exec <FILE.yomc>                Run a bytecode file produced by `build`
    disasm <FILE>                   Print the bytecode listing of a source file or a .yomc file
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
    no_opt: bool,
}

fn parse_opts(command: &str, rest: &[String]) -> Result<Opts, CliError> {
    let mut opts = Opts::default();
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        match (command, arg.as_str()) {
            ("run", "-d" | "--disassemble") => opts.disassemble = true,
            ("fmt", "--check") => opts.check = true,
            ("run" | "build" | "disasm", "--no-opt") => opts.no_opt = true,
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

/// Largest `.yomc` file `exec` and `disasm` will read.
const MAX_MODULE_BYTES: u64 = 16 * 1024 * 1024;

fn load_module(path: &Path) -> Result<yomnova_bytecode::module::Module, CliError> {
    let name = path.to_string_lossy().to_string();
    let meta = fs::metadata(path)
        .map_err(|e| CliError::Failure(format!("Cannot read file '{name}': {e}")))?;
    if meta.len() > MAX_MODULE_BYTES {
        return Err(CliError::Failure(format!(
            "'{name}' is larger than the {MAX_MODULE_BYTES} byte limit"
        )));
    }
    let bytes =
        fs::read(path).map_err(|e| CliError::Failure(format!("Cannot read file '{name}': {e}")))?;
    yomnova_bytecode::module::decode(&bytes)
        .map_err(|e| CliError::Failure(format!("'{name}': {e}")))
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
        "exec" => {
            let opts = parse_opts(command, rest)?;
            let path = opts
                .file
                .clone()
                .ok_or_else(|| CliError::Usage("'exec' requires a .yomc file".to_string()))?;
            let module = load_module(&path)?;
            let file = path.to_string_lossy().to_string();
            match Compiler::run_module(&module) {
                Ok((res, _)) => {
                    if res != Value::Unit {
                        println!("=> {res}");
                    }
                    Ok(ExitCode::SUCCESS)
                }
                Err(diags) => Err(CliError::Diagnostics {
                    diags,
                    source: String::new(),
                    file,
                }),
            }
        }
        "disasm"
            if rest.iter().any(|a| {
                fs::read(a).is_ok_and(|b| yomnova_bytecode::module::looks_like_module(&b))
            }) =>
        {
            let opts = parse_opts(command, rest)?;
            let path = opts
                .file
                .clone()
                .ok_or_else(|| CliError::Usage("'disasm' requires a file".to_string()))?;
            let module = load_module(&path)?;
            let mut out = module.main.disassemble("<main>");
            for (name, chunk, arity) in &module.functions {
                out.push('\n');
                out.push_str(&chunk.disassemble(&format!("fn {name}/{arity}")));
            }
            print!("{out}");
            Ok(ExitCode::SUCCESS)
        }
        "run" | "check" | "build" | "disasm" | "tokens" | "ast" | "fmt" => {
            let opts = parse_opts(command, rest)?;
            let path = opts
                .file
                .clone()
                .ok_or_else(|| CliError::Usage(format!("'{command}' requires a source file")))?;
            let (mut source, file) = read_source(&path)?;
            if matches!(command, "run" | "check" | "build" | "disasm") {
                let base = path.parent().unwrap_or(Path::new("."));
                source = yomnova_compiler::files::resolve(&source, base).map_err(|diags| {
                    CliError::Diagnostics {
                        diags,
                        source: source.clone(),
                        file: file.clone(),
                    }
                })?;
            }
            let diag = |diags: Vec<Diagnostic>| CliError::Diagnostics {
                diags,
                source: source.clone(),
                file: file.clone(),
            };

            // Warnings go to stderr before anything else; they never change the exit code.
            if matches!(command, "run" | "check" | "build") {
                if let Ok((_, warnings)) = Compiler::check_with_warnings(&source) {
                    for w in &warnings {
                        eprint!("{}", w.render(Some(&source), Some(&file)));
                    }
                }
            }

            match command {
                "run" => {
                    let out = Compiler::compile_and_run_opt(&source, !opts.no_opt).map_err(diag)?;
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
                    let module =
                        Compiler::compile_module_opt(&source, !opts.no_opt).map_err(diag)?;
                    let bytes = yomnova_bytecode::module::encode(&module)
                        .map_err(|e| CliError::Failure(format!("Cannot encode program: {e}")))?;
                    let out_path = opts.output.unwrap_or_else(|| path.with_extension("yomc"));
                    fs::write(&out_path, &bytes).map_err(|e| {
                        CliError::Failure(format!(
                            "Cannot write output '{}': {e}",
                            out_path.display()
                        ))
                    })?;
                    println!(
                        "ok: wrote '{}' ({} bytes); run it with `yomnova exec`",
                        out_path.display(),
                        bytes.len()
                    );
                }
                "disasm" => {
                    let unit = Compiler::compile_unit_opt(&source, !opts.no_opt).map_err(diag)?;
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
