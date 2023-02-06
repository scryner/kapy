use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    default_path: DefaultPath,
    policies: Vec<Policy>,

    #[serde(skip_deserializing)]
    commands: BTreeMap<i32, Commands>,
}

#[derive(Deserialize, Debug)]
pub struct DefaultPath {
    from: PathBuf,
    to: PathBuf,
}

#[derive(Deserialize, Debug)]
struct Policy {
    rate: Vec<i32>,
    commands: BTreeMap<String, String>,
}

impl Policy {
    fn commands(&self) -> Result<Commands, String> {
        let mut resize: Option<Resize> = None;
        let mut format: Option<Format> = None;
        let mut quality: Option<Quality> = None;

        // resize: 100% or 50m or preserve
        if let Some(opt) = self.commands.get("resize") {
            let opt = opt.clone().to_lowercase();

            let re = Regex::new(r"(?P<val>[0-9]+)(?P<postfix>[%m]{1})$").unwrap();
            if let Some(captures) = re.captures(&opt) {
                let vals = captures.name("val").unwrap().as_str();
                let val = vals.parse::<u8>().unwrap_or(100);

                let postfix = captures.name("postfix").unwrap().as_str();

                match postfix {
                    "%" => {
                        if val <= 100 {
                            resize = Some(Resize::Percentage(val));
                        }
                    }
                    "m" => {
                        resize = Some(Resize::MPixels(val));
                    }
                    _ => {
                        resize = Some(Resize::Preserve);
                    }
                }
            } else {
                return Err(format!("Invalid resize option from '{}'", opt));
            }
        }

        // format
        if let Some(opt) = self.commands.get("format") {
            let opt = opt.clone().to_lowercase();

            match opt.as_str() {
                "heic" => {
                    format = Some(Format::HEIC);
                }
                "jpg" | "jpeg" => {
                    format = Some(Format::JPEG);
                }
                "preserve" => {
                    format = Some(Format::Preserve);
                }
                _ => {
                    return Err(format!("Invalid format option from '{}'", opt));
                }
            }
        }

        // quality
        if let Some(opt) = self.commands.get("quality") {
            let re = Regex::new(r"(?P<val>[0-9]+)%$").unwrap();

            if let Some(captures) = re.captures(&opt) {
                let vals = captures.name("val").unwrap().as_str();
                let val = vals.parse::<u8>().unwrap_or(95);

                quality = Some(Quality::Percentage(val));
            } else {
                return Err(format!("Invalid quality option from '{}'", opt));
            }
        }

        if !resize.is_none() || !format.is_none() || !quality.is_none() {
            return Ok(Commands::Convert {
                resize: resize.unwrap_or(Resize::Preserve),
                format: format.unwrap_or(Format::Preserve),
                quality: quality.unwrap_or(Quality::Percentage(95)),
            });
        }

        Ok(Commands::ByPass)    // never reached
    }
}

#[derive(Debug, PartialEq)]
pub enum Commands {
    ByPass,
    Convert {
        resize: Resize,
        format: Format,
        quality: Quality,
    },
}

#[derive(Debug, PartialEq)]
pub enum Resize {
    Percentage(u8),
    MPixels(u8),
    Preserve,
}

#[derive(Debug, PartialEq)]
pub enum Format {
    JPEG,
    HEIC,
    Preserve,
}

#[derive(Debug, PartialEq)]
pub enum Quality {
    Percentage(u8),
}

impl Config {
    pub fn build_from_file(path: &Path) -> Result<Config, Error> {
        match fs::read_to_string(&path) {
            Ok(contents) => {
                Config::build(contents)
            }
            Err(err) => {
                Err(Error::IO(err.to_string()))
            }
        }
    }

    pub fn build(contents: String) -> Result<Config, Error> {
        // deserialize from yaml
        match deserialize(contents) {
            Ok(mut conf) => {
                // build for commands
                let mut m = BTreeMap::<i32, Commands>::new();

                for policy in conf.policies.iter() {
                    for rate in policy.rate.iter() {
                        match policy.commands() {
                            Ok(commands) => {
                                m.insert(*rate, commands);
                            }
                            Err(e) => {
                                return Err(Error::Parse(e));
                            }
                        }
                    }
                }

                conf.commands = m;
                Ok(conf)
            }
            Err(e) => {
                return Err(Error::Parse(format!("Failed to deserialize: {:?}", e)));
            }
        }
    }

    pub fn commands(&self, rate: i32) -> &Commands {
        self.commands.get(&rate).unwrap_or(&Commands::ByPass)
    }
}

fn deserialize(s: String) -> Result<Config, Error> {
    match serde_yaml::from_str(&s) {
        Ok(conf) => {
            Ok(conf)
        }
        Err(e) => {
            Err(Error::Parse(e.to_string()))
        }
    }
}

#[derive(Debug)]
pub enum Error {
    IO(String),
    Parse(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_from_str() {
        let yaml = r#"default_path:
  from: /Volumes/Untitled/DCIM/108HASBL
  to: ~/images
policies:
- rate: [5]
  commands:
    resize: 100%  # default value; ignore it
    format: preserve  # default value
- rate: [0,1,2,3,4]
  commands:
    resize: 36m # resize image to 36m pixels
    quality: 92%
    format: heic"#;

        let conf = Config::build(String::from(yaml))
            .expect("Failed to deserialize from string");

        println!("conf=\n{:?}", conf);

        // get unspecified rate: it must be default value
        let commands = conf.commands(6);
        assert_eq!(commands, &Commands::ByPass);

        // get specific rate
        let commands = conf.commands(3);
        assert_eq!(commands, &Commands::Convert {
            resize: Resize::MPixels(36),
            format: Format::HEIC,
            quality: Quality::Percentage(92),
        });
    }

    #[test]
    fn get_policy() {
        let policy = Policy {
            rate: vec![1],
            commands: BTreeMap::from([
                ("resize".to_string(), "50%".to_string()),
                ("format".to_string(), "heic".to_string()),
                ("quality".to_string(), "90%".to_string()),
            ]),
        };

        let commands = policy.commands().unwrap();
        assert_eq!(commands, Commands::Convert {
            resize: Resize::Percentage(50),
            format: Format::HEIC,
            quality: Quality::Percentage(90),
        })
    }
}