#[derive(Debug)]
pub struct Target {
    name: String,
    file_dependencies: Vec<String>,
    target_dependencies: Vec<String>,
    commands: Vec<String>,
}

use nom::{
    bytes::complete::{tag, take_until, take_while1},
    character::complete::{alphanumeric1, multispace0, multispace1},
    combinator::map,
    multi::{many0, separated_list0},
    sequence::{delimited, preceded, separated_pair},
    IResult, Parser,
};

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

impl Target {
    pub fn build(&self, file: &Makefile) -> Option<()> {
        let pre_build = std::time::Instant::now();
        for t_dep in &self.target_dependencies {
            file.get_target(t_dep)?.build(file);
        }

        let mut build_children = vec![];
        for cmd in self
            .commands
            .iter()
            .map(|s| s.split_whitespace().collect::<Vec<_>>())
        {
            let exe = cmd[0];
            let args = &cmd[1..];
            let mut cmd = std::process::Command::new(exe);
            cmd.args(args);
            build_children.push(cmd.spawn().unwrap());
        }

        for mut c in build_children {
            c.wait().unwrap();
        }
        println!(
            "Building target `{}` took: {:.2?}",
            self.name,
            pre_build.elapsed()
        );

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
