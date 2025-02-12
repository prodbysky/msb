use nom::{
    bytes::complete::{tag, take_until, take_while1},
    character::complete::{alphanumeric1, multispace0, multispace1},
    combinator::{map, opt},
    multi::{many0, separated_list0},
    sequence::{delimited, preceded, separated_pair},
    IResult, Parser,
};

use error_stack::ResultExt;
use std::{
    fs,
    path::Path,
    time::{Instant, SystemTime},
};
use thiserror::Error;

#[derive(Debug)]
pub struct Target {
    name: String,
    outputs: Vec<String>,
    file_dependencies: Vec<String>,
    target_dependencies: Vec<String>,
    commands: Vec<String>,
}

impl Target {
    pub fn new(
        name: String,
        outputs: Vec<String>,
        file_dependencies: Vec<String>,
        target_dependencies: Vec<String>,
        commands: Vec<String>,
    ) -> Self {
        Self {
            name,
            outputs,
            file_dependencies,
            target_dependencies,
            commands,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn outputs(&self) -> &[String] {
        &self.outputs
    }
    pub fn file_dependencies(&self) -> &[String] {
        &self.file_dependencies
    }
    pub fn target_dependencies(&self) -> &[String] {
        &self.target_dependencies
    }

    /// Helper function for getting oldest output last-modified time
    fn get_min_output_time(&self) -> Option<SystemTime> {
        let mut min_time = None;
        for output in &self.outputs {
            let path = Path::new(output);
            let metadata = fs::metadata(path).ok()?;
            let modified = metadata.modified().ok()?;
            min_time = Some(match min_time {
                None => modified,
                Some(current) => {
                    if modified < current {
                        modified
                    } else {
                        current
                    }
                }
            });
        }
        min_time
    }

    fn is_up_to_date(&self, makefile: &Makefile) -> bool {
        // If getting the last modified time failed assume that the file does not exist
        // therefore we need to build
        let Some(target_mod_time) = self.get_min_output_time() else {
            return false;
        };

        for dep in &self.file_dependencies {
            let dep_path = Path::new(dep);
            let Ok(dep_mod_time) = fs::metadata(dep_path).and_then(|meta| meta.modified()) else {
                return false;
            };
            if dep_mod_time > target_mod_time {
                return false;
            }
        }

        for dep_name in &self.target_dependencies {
            if let Some(dep_target) = makefile.get_target(dep_name) {
                let Some(dep_mod_time) = dep_target.get_min_output_time() else {
                    return false;
                };
                if dep_mod_time > target_mod_time {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }

    pub fn build(&self, makefile: &Makefile) -> BuildResult<()> {
        let pre_build = Instant::now();

        for dep in &self.target_dependencies {
            match makefile.get_target(dep) {
                None => {
                    return Err(BuildError::FailedToFindTargetForDependency {
                        target_name: self.name.clone(),
                        dependency_name: dep.to_string(),
                    }
                    .into());
                }
                Some(target_dep) => target_dep.build(makefile)?,
            }
        }

        if self.is_up_to_date(makefile) {
            println!("Target `{}` is up-to-date, skipping build.", self.name);
            return Ok(());
        }

        // TODO: Proper command line parsing
        let mut children = vec![];
        for cmd in &self.commands {
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }
            let exe = parts[0];
            let args = &parts[1..];
            let mut command = std::process::Command::new(exe);
            command.args(args);
            children.push(
                command
                    .spawn()
                    .change_context(BuildError::FailedToSpawnProcess {
                        cmd: cmd.to_string(),
                    })?,
            );
        }

        for mut child in children {
            let exit_code = child
                .wait()
                .change_context(BuildError::BuildProcessFailedToStart)?
                .code()
                .ok_or(BuildError::FailedToGetChildExitCode)?;
            if exit_code != 0 {
                return Err(BuildError::BuildProcessQuitWithNonZero.into());
            }
        }

        println!(
            "Building target `{}` took: {:.2?}",
            self.name,
            pre_build.elapsed()
        );
        Ok(())
    }
}

#[derive(Debug)]
pub struct Makefile {
    targets: Vec<Target>,
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("Failed to spawn build process: {cmd}")]
    FailedToSpawnProcess { cmd: String },
    #[error("Failed to find target to build `{dependency_name}` for target `{target_name}`")]
    FailedToFindTargetForDependency {
        target_name: String,
        dependency_name: String,
    },
    #[error("Failed to find target `{target_name}` to build")]
    FailedToFindTargetToBuild { target_name: String },
    #[error("Some build process failed to start")]
    BuildProcessFailedToStart,
    #[error("Failed to get build process exit code")]
    FailedToGetChildExitCode,
    #[error("Build process quit with non-zero exit code")]
    BuildProcessQuitWithNonZero,
}

pub type BuildResult<T> = error_stack::Result<T, BuildError>;

impl Makefile {
    pub fn get_targets(&self) -> &Vec<Target> {
        &self.targets
    }

    pub fn get_target(&self, name: &str) -> Option<&Target> {
        self.targets.iter().find(|t| t.name == name)
    }

    pub fn build(self, target: &str) -> BuildResult<()> {
        self.get_target(target)
            .ok_or(BuildError::FailedToFindTargetToBuild {
                target_name: target.to_string(),
            })?
            .build(&self)?;
        Ok(())
    }
}

fn identifier(input: &str) -> IResult<&str, &str> {
    alphanumeric1(input)
}

fn file_identifier(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| !c.is_whitespace() && c != ')')(input)
}

fn parse_outputs(input: &str) -> IResult<&str, Vec<String>> {
    delimited(
        tag("outputs("),
        separated_list0(multispace1, map(file_identifier, |s: &str| s.to_string())),
        tag(")"),
    )
    .parse(input)
}

fn parse_files(input: &str) -> IResult<&str, Vec<String>> {
    delimited(
        tag("files("),
        separated_list0(multispace1, map(file_identifier, |s: &str| s.to_string())),
        tag(")"),
    )
    .parse(input)
}

fn target_identifier(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| !c.is_whitespace() && c != ',' && c != ')')(input)
}

fn parse_target_deps(input: &str) -> IResult<&str, Vec<String>> {
    delimited(
        tag("targets("),
        separated_list0(
            delimited(multispace0, tag(","), multispace0),
            map(target_identifier, |s: &str| s.to_string()),
        ),
        tag(")"),
    )
    .parse(input)
}

fn parse_dependencies(input: &str) -> IResult<&str, (Vec<String>, Vec<String>)> {
    delimited(
        tag("["),
        separated_pair(parse_files, multispace1, parse_target_deps),
        tag("]"),
    )
    .parse(input)
}

fn parse_commands(input: &str) -> IResult<&str, Vec<String>> {
    let (input, content) = delimited(
        delimited(multispace0, tag("{"), multispace0),
        take_until("}"),
        preceded(multispace0, tag("}")),
    )
    .parse(input)?;

    let commands: Vec<String> = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect();
    Ok((input, commands))
}

fn parse_target(input: &str) -> IResult<&str, Target> {
    let (input, _) = multispace0(input)?;
    let (input, _) = tag("target")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, name) = identifier(input)?;
    let (input, outputs) = opt(preceded(multispace1, parse_outputs)).parse(input)?;
    let outputs = outputs.unwrap_or_else(|| vec![name.to_string()]);
    let (input, _) = multispace1(input)?;
    let (input, (files, target_deps)) = parse_dependencies(input)?;
    let (input, _) = multispace0(input)?;
    let (input, commands) = parse_commands(input)?;

    Ok((
        input,
        Target::new(name.to_string(), outputs, files, target_deps, commands),
    ))
}

pub fn parse_makefile(input: &str) -> Option<Makefile> {
    let (_, targets) = many0(parse_target).parse(input).ok()?;
    Some(Makefile { targets })
}
