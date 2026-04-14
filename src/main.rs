#![deny(unsafe_code)]
// "OpenAPI", "YAML", "JSON" are spec/format names, not code identifiers.
#![allow(clippy::doc_markdown)]

use std::io::IsTerminal as _;

use clap::Parser;

use openapi_linter::reporter::{ColorMode, Format};

#[derive(clap::ValueEnum, Clone)]
enum OutputFormat {
    Text,
    Json,
    Sarif,
}

#[derive(Parser)]
#[command(
    name = "openapi-linter",
    about = "Fast OpenAPI linter — Spectral OAS compatible"
)]
struct Cli {
    /// Path to an OpenAPI spec file or directory (YAML or JSON).
    ///
    /// When a directory is given, all .yaml/.yml/.json files are linted
    /// recursively (hidden directories like .git are skipped).
    spec: std::path::PathBuf,

    /// Path to a .spectral.yaml ruleset file.
    #[arg(short = 'r', long)]
    ruleset: Option<std::path::PathBuf>,

    /// Output format.
    #[arg(short = 'f', long, default_value = "text")]
    format: OutputFormat,

    /// Disable ANSI colour in text output.
    #[arg(long)]
    no_color: bool,

    /// Always enable ANSI colour (overrides --no-color).
    #[arg(long)]
    color: bool,

    /// Suppress output; exit 0 if clean, 1 if violations found.
    #[arg(short = 'q', long)]
    quiet: bool,
}

fn main() {
    let cli = Cli::parse();

    let color_mode = if cli.no_color {
        ColorMode::Never
    } else if cli.color || std::io::stdout().is_terminal() {
        ColorMode::Always
    } else {
        ColorMode::Never
    };

    let format = match cli.format {
        OutputFormat::Text => Format::Text,
        OutputFormat::Json => Format::Json,
        OutputFormat::Sarif => Format::Sarif,
    };

    if cli.spec.is_dir() {
        run_dir(&cli, format, color_mode);
    } else {
        run_file(&cli, format, color_mode);
    }
}

fn run_file(cli: &Cli, format: Format, color_mode: ColorMode) {
    match openapi_linter::lint(&cli.spec, cli.ruleset.as_deref()) {
        Ok(violations) => {
            if cli.quiet {
                std::process::exit(i32::from(!violations.is_empty()));
            }

            let files = vec![(cli.spec.clone(), violations)];
            let has_violations = files.iter().any(|(_, vs)| !vs.is_empty());

            let mut out = std::io::stdout().lock();
            if let Err(e) = openapi_linter::reporter::report(&files, format, color_mode, &mut out) {
                eprintln!("error writing output: {e}");
                std::process::exit(2);
            }

            std::process::exit(i32::from(has_violations));
        }
        Err(e) => {
            eprintln!("{e:#}");
            std::process::exit(2);
        }
    }
}

fn run_dir(cli: &Cli, format: Format, color_mode: ColorMode) {
    let scan_results = match openapi_linter::lint_dir(&cli.spec, cli.ruleset.as_deref()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error scanning directory: {e:#}");
            std::process::exit(2);
        }
    };

    if scan_results.is_empty() {
        eprintln!("no OpenAPI spec files found in {}", cli.spec.display());
        std::process::exit(0);
    }

    // Separate successes from per-file errors.
    let mut files: Vec<(std::path::PathBuf, Vec<openapi_linter::model::Violation>)> = Vec::new();
    let mut parse_errors: usize = 0;

    for (path, result) in scan_results {
        match result {
            Ok(violations) => files.push((path, violations)),
            Err(e) => {
                parse_errors += 1;
                eprintln!("[warn] {}: {e:#}", path.display());
            }
        }
    }

    let total_violations: usize = files.iter().map(|(_, vs)| vs.len()).sum();
    let file_count = files.len();

    if !cli.quiet {
        let mut out = std::io::stdout().lock();
        if let Err(e) = openapi_linter::reporter::report(&files, format, color_mode, &mut out) {
            eprintln!("error writing output: {e}");
            std::process::exit(2);
        }

        // Summary line.
        if total_violations == 0 {
            eprintln!("no violations in {file_count} file(s)");
        } else {
            eprintln!("{total_violations} violation(s) in {file_count} file(s)");
        }
        if parse_errors > 0 {
            eprintln!("{parse_errors} file(s) failed to parse");
        }
    }

    let has_violations = total_violations > 0 || parse_errors > 0;
    std::process::exit(i32::from(has_violations));
}
