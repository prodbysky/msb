mod target;

use error_stack::ResultExt;
use thiserror::Error;

fn main() -> AppResult {
    let config = Config::parse().change_context(AppError::FailedToParseConfig)?;

    let input_content =
        std::fs::read_to_string(&config.input_name).change_context(AppError::FailedToReadInput)?;

    let targets = target::Makefile::from_str(input_content.as_str())
        .change_context(AppError::FailedToParseBuildFile)
        .attach_printable("failed to parse the .msb file")?;
    targets.build(&config.target);

    Ok(())
}

#[derive(Debug, Error)]
enum AppError {
    #[error("Failed to parse command line arguments")]
    FailedToParseConfig,
    #[error("A file system error occured when reading input build config")]
    FailedToReadInput,
    #[error("Failed to parse .msb file")]
    FailedToParseBuildFile,
}

type AppResult = error_stack::Result<(), AppError>;

#[derive(Debug, Error)]
enum ConfigError {
    #[error("Input file does not exist: {0}")]
    BuildFileDoesNotExist(String),
}

type ConfigResult = error_stack::Result<Config, ConfigError>;

struct Config {
    input_name: std::path::PathBuf,
    target: String,
}

impl Config {
    fn parse() -> ConfigResult {
        let mut args = std::env::args();
        let _program_name = args.next().unwrap();
        let input_name = match args.next() {
            None => {
                let mut input = std::path::PathBuf::new();
                input.push("build");
                input.set_extension("msb");
                input
            }
            Some(path) => {
                let mut input = std::path::PathBuf::new();
                input.push(path);
                input.set_extension("msb");
                input
            }
        };
        let as_str = input_name.clone().into_os_string().into_string().unwrap();
        if !input_name.exists() || !input_name.is_file() {
            return Err(ConfigError::BuildFileDoesNotExist(as_str).into());
        }
        let target_name = args.next().map_or("main".to_string(), |o| o);

        Ok(Self {
            input_name,
            target: target_name,
        })
    }
}
