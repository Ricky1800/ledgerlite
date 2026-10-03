use clap::Parser;
use ledgerlite::cli::{Cli, Command};
use std::error::Error as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();

    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("error: could not determine the current directory: {e}");
            return ExitCode::FAILURE;
        }
    };

    let result = match cli.command {
        Command::Init { force } => ledgerlite::cmd_init(&cwd, force),
        Command::Import {
            file,
            account,
            profile,
        } => ledgerlite::cmd_import(&cwd, &file, &account, profile.as_deref()),
        Command::Categorize { dry_run } => ledgerlite::cmd_categorize(&cwd, dry_run),
        Command::Report {
            month,
            year,
            from,
            to,
            format,
            schedule_c,
            out,
        } => ledgerlite::cmd_report(
            &cwd,
            month.as_deref(),
            year,
            from.as_deref(),
            to.as_deref(),
            &format,
            schedule_c,
            out.as_deref(),
        ),
        Command::Export {
            format,
            output,
            from,
            to,
        } => ledgerlite::cmd_export(
            &cwd,
            &format,
            output.as_deref(),
            from.as_deref(),
            to.as_deref(),
        ),
        Command::Completions { shell } => ledgerlite::cmd_completions(shell),
    };

    match result {
        Ok(message) => {
            print!("{message}");
            if !message.ends_with('\n') {
                println!();
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err}");
            let mut cause = err.source();
            while let Some(source) = cause {
                eprintln!("  caused by: {source}");
                cause = source.source();
            }
            ExitCode::FAILURE
        }
    }
}
