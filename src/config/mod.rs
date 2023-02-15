use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use regex::Regex;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    import: ImportPath,
    policies: Vec<Policy>,

    #[serde(skip_deserializing)]
    commands: BTreeMap<i8, Command>,
}

#[derive(Deserialize, Debug)]
pub struct ImportPath {
    from: PathBuf,
    to: PathBuf,
}

type UnparsedCommand = Option<BTreeMap<String, String>>;

#[derive(Deserialize, Debug)]
struct Policy {
    rate: Vec<i8>,
    command: UnparsedCommand,
}

impl Policy {
    fn command(&self) -> Result<Command, String> {
        let m = match &self.command {
            Some(m) => m,
            None => return Ok(Command::ByPass),
        };

        let mut resize: Option<Resize> = None;
        let mut format: Option<Format> = None;
        let mut quality: Option<Quality> = None;

        // resize: 100% or 50m or preserve
        if let Some(opt) = m.get("resize") {
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
        if let Some(opt) = m.get("format") {
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
        if let Some(opt) = m.get("quality") {
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
            return Ok(Command::Convert {
                resize: resize.unwrap_or(Resize::Preserve),
                format: format.unwrap_or(Format::Preserve),
                quality: quality.unwrap_or(Quality::Preserve),
            });
        }

        Ok(Command::ByPass)    // never reached
    }
}

#[derive(Debug, PartialEq)]
pub enum Command {
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

impl Format {
    pub fn as_str(&self) -> &str {
        match self {
            Format::JPEG => "JPEG",
            Format::HEIC => "HEIC",
            Format::Preserve => "",
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Quality {
    Percentage(u8),
    Preserve,
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
                let mut m = BTreeMap::<i8, Command>::new();

                for policy in conf.policies.iter() {
                    for rate in policy.rate.iter() {
                        match policy.command() {
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

    pub fn set_import_from(&mut self, path: PathBuf) {
        self.import.from = path;
    }

    pub fn set_import_to(&mut self, path: PathBuf) {
        self.import.to = path;
    }

    pub fn import_from(&self) -> &Path {
        &self.import.from
    }

    pub fn import_to(&self) -> &Path {
        &self.import.to
    }

    pub fn command(&self, rate: i8) -> &Command {
        self.commands.get(&rate).unwrap_or(&Command::ByPass)
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

#[derive(Debug)]
pub struct ConfigPath {
    app_home: Rc<Box<Path>>,
    config_path: Rc<Box<Path>>,
    cred_path: Rc<Box<Path>>,
}

impl ConfigPath {
    pub fn app_home(&self) -> Rc<Box<Path>> {
        Rc::clone(&self.app_home)
    }

    pub fn config_path(&self) -> Rc<Box<Path>> {
        Rc::clone(&self.config_path)
    }

    pub fn cred_path(&self) -> Rc<Box<Path>> {
        Rc::clone(&self.cred_path)
    }
}

pub fn default_path() -> ConfigPath {
    let home_dir = home::home_dir().unwrap();
    let app_home = home_dir.join(".kapy");
    let config_path = app_home.join("config.yaml");
    let cred_path = app_home.join(".cred");

    ConfigPath {
        app_home: Rc::new(app_home.into_boxed_path()),
        config_path: Rc::new(config_path.into_boxed_path()),
        cred_path: Rc::new(cred_path.into_boxed_path()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_from_str() {
        let yaml = r#"import:
  from: /Volumes/Untitled/DCIM/108HASBL
  to: ~/images
policies:
- rate: [5]
  command:
    resize: 100%  # default value; ignore it
    format: preserve  # default value
- rate: [4]
- rate: [0,1,2,3]
  command:
    resize: 36m # resize image to 36m pixels
    quality: 92%
    format: heic
"#;

        let conf = Config::build(String::from(yaml))
            .expect("Failed to deserialize from string");

        println!("conf=\n{:#?}", conf);

        // get unspecified rate: it must be default value
        let command = conf.command(6);
        assert_eq!(command, &Command::ByPass);

        // get specific rate
        let command = conf.command(3);
        assert_eq!(command, &Command::Convert {
            resize: Resize::MPixels(36),
            format: Format::HEIC,
            quality: Quality::Percentage(92),
        });
    }

    #[test]
    fn get_policy() {
        let policy = Policy {
            rate: vec![1],
            command: Some(BTreeMap::from([
                ("resize".to_string(), "50%".to_string()),
                ("format".to_string(), "heic".to_string()),
                ("quality".to_string(), "90%".to_string()),
            ])),
        };

        let commands = policy.command().unwrap();
        assert_eq!(commands, Command::Convert {
            resize: Resize::Percentage(50),
            format: Format::HEIC,
            quality: Quality::Percentage(90),
        })
    }
}