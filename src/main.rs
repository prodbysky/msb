mod target;

use clap::Parser;
use error_stack::ResultExt;
use std::path::PathBuf;
use thiserror::Error;

fn main() -> AppResult {
    let config = Config::parse();
    if let Err(e) = config.validate() {
        eprintln!("Error: {}", e);
        eprintln!("Run with -h to print help");
        std::process::exit(1);
    }

    let input_content =
        std::fs::read_to_string(&config.input_name).change_context(AppError::FailedToReadInput)?;

    let targets = target::Makefile::from_str(input_content.as_str())
        .change_context(AppError::FailedToParseBuildFile)
        .attach_printable("failed to parse the .msb file")?;

    if config.print_targets {
        for (i, target) in targets.get_targets().iter().enumerate() {
            println!("{}: {}", i, target.name());
            if target.file_dependencies().is_empty() && target.target_dependencies().is_empty() {
                println!("Does not depend on anything")
            } else {
                println!("Depends on:");
                if !target.file_dependencies().is_empty() {
                    println!("  These files:");
                    for f in target.file_dependencies() {
                        println!("    {}", f);
                    }
                }
                if !target.target_dependencies().is_empty() {
                    println!("  These targets:");
                    for t in target.target_dependencies() {
                        println!("    {}", t);
                    }
                }
            }
        }
        return Ok(());
    }
    targets.build(&config.target);

    Ok(())
}

#[derive(Debug, Error)]
enum AppError {
    #[error("A file system error occured when reading input build config")]
    FailedToReadInput,
    #[error("Failed to parse .msb file")]
    FailedToParseBuildFile,
}

type AppResult = error_stack::Result<(), AppError>;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Config {
    /// Path to the input file (defaults to "build.msb" if not provided)
    #[arg(default_value = "build.msb")]
    input_name: PathBuf,

    /// Target name (defaults to "main" if not provided)
    #[arg(default_value = "main")]
    target: String,

    /// Print the available targets in this .msb file
    #[arg(long)]
    print_targets: bool,
}

impl Config {
    fn validate(&self) -> Result<(), String> {
        if !self.input_name.exists() || !self.input_name.is_file() {
            return Err(format!(
                "Build file does not exist: {}",
                self.input_name.to_string_lossy()
            ));
        }
        Ok(())
    }
}
