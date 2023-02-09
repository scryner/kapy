use std::{fs, process};
use std::io::Write;

use console::style;

use crate::config;

pub fn do_init(force: bool) {
    println!("Initializing kapy...");

    // get default config_path
    let (conf_dir, conf_path) = match config::default_config_path() {
        Ok((dir, path, _)) => (dir, path),
        Err(e) => {
            eprintln!("Failed to get default config path: {}", e.to_string());
            process::exit(1);
        }
    };

    // check configuration file is already existed
    if fs::metadata(&conf_path).is_ok() && !force {
        println!("Already initialized, config is on '{}'", conf_path.to_str().unwrap());
        process::exit(0);
    }

    // create kapy home directory
    print!("\tCreating kapy home directory '{}'...", conf_dir.to_str().unwrap());
    match fs::create_dir(conf_dir) {
        Ok(()) => println!("\t{}", style("[  OK  ]").green()),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                println!("\t{}", style("[  OK  ]").green())
            } else {
                println!("\t{}", style("[FAILED]").red());
                eprintln!("Failed to create directory: {}", e);
                process::exit(1);
            }
        }
    }

    // make default configuration to the directory
    print!("\tCreating configurations on '{}'...", conf_path.to_str().unwrap());
    match fs::File::create(&conf_path) {
        Ok(mut file) => {
            match file.write_all(DEFAULT_CONF_YAML.as_bytes()) {
                Ok(_) => println!("\t{}", style("[  OK  ]").green()),
                Err(e) => {
                    println!("\t{}", style("[FAILED]").red());
                    eprintln!("Failed to write configuration to file: {}", e.to_string());
                    process::exit(1);
                }
            }
        }
        Err(e) => {
            println!("\t{}", style("[FAILED]").red());
            eprintln!("Failed to create file: {}", e.to_string());
            process::exit(1);
        }
    }

    println!("\nYou must edit configurations on '{}'",
           style(conf_path.to_str().unwrap()).cyan());
}

const DEFAULT_CONF_YAML: &str = r#"import:
  from: YOUR_ORIGIN_PATH
  to: YOUR_TARGET_PATH
policies:
- rate: [4]
  command:
    format: heic
- rate: [3]
  command:
    format: heic
    resize: 50m
- rate: [0,1,2]
  command:
    format: heic
    resize: 36m
    quality: 92%
"#;