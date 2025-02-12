use nom::{
    bytes::complete::{tag, take_until, take_while1},
    character::complete::{alphanumeric1, multispace0, multispace1},
    combinator::map,
    multi::{many0, separated_list0},
    sequence::{delimited, preceded, separated_pair},
    IResult, Parser,
};

use error_stack::ResultExt;
use thiserror::Error;

#[derive(Debug)]
pub struct Target {
    name: String,
    file_dependencies: Vec<String>,
    target_dependencies: Vec<String>,
    commands: Vec<String>,
}

fn identifier(input: &str) -> IResult<&str, &str> {
    alphanumeric1(input)
}

fn file_identifier(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| !c.is_whitespace() && c != ')')(input)
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
    let (input, _) = multispace1(input)?;
    let (input, (files, target_deps)) = parse_dependencies(input)?;
    let (input, _) = multispace0(input)?;
    let (input, commands) = parse_commands(input)?;

    Ok((
        input,
        Target::new(name.to_string(), files, target_deps, commands),
    ))
}

pub fn parse_makefile(input: &str) -> Option<Makefile> {
    let (_, targets) = many0(parse_target).parse(input).ok()?;
    Some(Makefile { targets })
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

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn file_dependencies(&self) -> &[String] {
        &self.file_dependencies
    }
    pub fn target_dependencies(&self) -> &[String] {
        &self.target_dependencies
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

impl Target {
    pub fn build(&self, file: &Makefile) -> BuildResult<()> {
        let pre_build = std::time::Instant::now();
        for t_dep in &self.target_dependencies {
            match file.get_target(t_dep) {
                None => {
                    return Err(BuildError::FailedToFindTargetForDependency {
                        target_name: self.name.clone(),
                        dependency_name: t_dep.to_string(),
                    }
                    .into());
                }
                Some(t) => t.build(file)?,
            }
        }

        let mut build_children = vec![];
        for cmd in &self.commands {
            let split = cmd.split_whitespace().collect::<Vec<_>>();
            let exe = split[0];
            let args = &split[1..];
            let mut cmd_proc = std::process::Command::new(exe);
            cmd_proc.args(args);
            build_children.push(cmd_proc.spawn().change_context(
                BuildError::FailedToSpawnProcess {
                    cmd: cmd.to_string(),
                },
            )?);
        }

        for mut c in build_children {
            match c
                .wait()
                .change_context(BuildError::BuildProcessFailedToStart)?
                .code()
                .ok_or(BuildError::FailedToGetChildExitCode)?
            {
                x if x != 0 => Err(BuildError::BuildProcessQuitWithNonZero),
                _ => Ok(()),
            }?;
        }
        println!(
            "Building target `{}` took: {:.2?}",
            self.name,
            pre_build.elapsed()
        );

        Ok(())
    }
}

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
