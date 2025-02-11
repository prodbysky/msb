use error_stack::ResultExt;
use thiserror::Error;

#[derive(Debug)]
pub struct Target {
    name: String,
    file_dependencies: Vec<String>,
    target_dependencies: Vec<String>,
    commands: Vec<String>,
}

impl Target {
    pub fn new(
        name: String,
        file_dependencies: Vec<String>,
        target_dependencies: Vec<String>,
        commands: Vec<String>,
    ) -> Self {
        Self {
            name,
            file_dependencies,
            target_dependencies,
            commands,
        }
    }
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Invalid target format found")]
    InvalidTarget,
    #[error("Invalid target header format found")]
    InvalidTargetHeader,
    #[error("Invalid dependencies format found")]
    InvalidDependencies,
    #[error("Missmatched braces found")]
    MissmatchedBracesInFile,
}

pub type ParseResult<T> = error_stack::Result<T, ParseError>;

impl Target {
    fn from_str(s: &str) -> ParseResult<Target> {
        let parts: Vec<&str> = s.trim().split('{').collect();
        if parts.len() != 2 {
            return Err(ParseError::InvalidTarget.into());
        }

        let header = parts[0].trim();
        let header_parts: Vec<&str> = header.split('[').collect();
        if header_parts.len() != 2 {
            return Err(ParseError::InvalidTargetHeader.into());
        }

        let name = header_parts[0]
            .trim_start_matches("target")
            .trim()
            .to_string();

        let deps_str = header_parts[1].trim_end_matches(']');
        let (file_deps, target_deps) = parse_dependencies(deps_str)?;

        let commands_str = parts[1].trim_end_matches('}').trim();
        let commands = commands_str
            .split(';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(Target {
            name,
            file_dependencies: file_deps,
            target_dependencies: target_deps,
            commands,
        })
    }
}

fn parse_dependencies(deps_str: &str) -> ParseResult<(Vec<String>, Vec<String>)> {
    let parts: Vec<&str> = deps_str.split("targets(").collect();
    if parts.len() != 2 {
        return Err(ParseError::InvalidDependencies.into());
    }

    let files_str = parts[0]
        .trim_start_matches("files(")
        .trim()
        .trim_end_matches(')');

    let file_deps: Vec<String> = if files_str.is_empty() {
        Vec::new()
    } else {
        files_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    let targets_str = parts[1].trim_end_matches(')').trim();
    let target_deps: Vec<String> = if targets_str.is_empty() {
        Vec::new()
    } else {
        targets_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    Ok((file_deps, target_deps))
}

#[derive(Debug)]
pub struct Makefile {
    targets: Vec<Target>,
}

impl Makefile {
    pub fn from_str(s: &str) -> ParseResult<Makefile> {
        let mut targets = Vec::new();
        let mut current_target = String::new();
        let mut brace_count = 0;

        for line in s.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            current_target.push_str(line);
            current_target.push(' ');

            brace_count += line.chars().filter(|&c| c == '{').count();
            brace_count -= line.chars().filter(|&c| c == '}').count();

            if brace_count == 0 && !current_target.trim().is_empty() {
                if current_target.contains("target") {
                    targets.push(
                        Target::from_str(&current_target)
                            .attach_printable("Failed to parse some target")?,
                    );
                }
                current_target.clear();
            }
        }

        if brace_count != 0 {
            return Err(ParseError::MissmatchedBracesInFile.into());
        }

        Ok(Makefile { targets })
    }
}
impl Target {
    pub fn build(&self, file: &Makefile) -> Option<()> {
        for t_dep in &self.target_dependencies {
            file.get_target(t_dep)?.build(file);
        }

        for cmd in self
            .commands
            .iter()
            .map(|s| s.split_whitespace().collect::<Vec<_>>())
        {
            let exe = cmd[0];
            let args = &cmd[1..];
            let mut cmd = std::process::Command::new(exe);
            cmd.args(args);
            cmd.spawn().unwrap().wait().unwrap();
        }

        Some(())
    }
}

impl Makefile {
    pub fn get_targets(&self) -> &Vec<Target> {
        &self.targets
    }

    pub fn get_target(&self, name: &str) -> Option<&Target> {
        self.targets.iter().find(|t| t.name == name)
    }

    pub fn build(self, target: &str) -> Option<()> {
        self.get_target(target)?.build(&self);
        Some(())
    }
}
