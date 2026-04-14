#![deny(unsafe_code)]

use std::io::IsTerminal as _;

use clap::Parser;

#[derive(clap::ValueEnum, Clone)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Parser)]
#[command(
    name = "openapi-linter",
    about = "Fast OpenAPI linter — Spectral OAS compatible"
)]
struct Cli {
    /// Path to the `OpenAPI` spec file (YAML or JSON).
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

    /// Suppress output; exit 0 if clean, 1 if violations found.
    #[arg(short = 'q', long)]
    quiet: bool,
}

fn main() {
    let cli = Cli::parse();

    match openapi_linter::lint(&cli.spec, cli.ruleset.as_deref()) {
        Ok(violations) => {
            if cli.quiet {
                std::process::exit(i32::from(!violations.is_empty()));
            }

            let stdout = std::io::stdout();
            let use_color = !cli.no_color && stdout.is_terminal();
            let spec_path = cli.spec.display().to_string();
            let mut out = stdout.lock();

            let write_result = match cli.format {
                OutputFormat::Text => openapi_linter::reporter::write_text(
                    &violations,
                    &spec_path,
                    use_color,
                    &mut out,
                ),
                OutputFormat::Json => {
                    openapi_linter::reporter::write_json(&violations, &spec_path, &mut out)
                }
            };

            if let Err(e) = write_result {
                eprintln!("error writing output: {e}");
                std::process::exit(2);
            }

            std::process::exit(i32::from(!violations.is_empty()));
        }
        Err(e) => {
            eprintln!("{e:#}");
            std::process::exit(2);
        }
    }
}
