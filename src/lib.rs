mod init;
mod clean;
mod clone;

mod config;
mod processor;
mod drive;
mod progress;

use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};
use crate::config::Config;

#[derive(Parser)]
#[command(name = "kapy")]
#[command(author = "scryner <scryner@gmail.com>")]
#[command(about = "A copy utility for large images taken by cameras", long_about = None)]
struct Cli {
    /// Set a custom config file
    #[arg(short, long, value_name = "CONF_PATH", global = true)]
    config: Option<PathBuf>,

    /// Set google API credentials path
    #[arg(long, value_name = "CRED_PATH", global = true)]
    cred: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, PartialEq, Debug)]
enum Commands {
    /// Clone images according to policies defined at config file
    #[command(arg_required_else_help = true)]
    Clone {
        /// Set origin path to import
        #[arg(long, value_name = "FROM_PATH")]
        from: Option<PathBuf>,

        /// Set destination path to export
        #[arg(long, value_name = "TO_PATH")]
        to: Option<PathBuf>,

        /// Set ignore geotag
        #[arg(long, default_value_t = false)]
        ignore_geotag: bool,

        /// Show what would do without copying/writing to destination
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Initialize to make configuration file
    Init {
        /// Force overwritten
        #[arg(long, default_value_t = false)]
        force: bool,
    },

    /// Clean credentials
    Clean,
}

pub fn run() {
    let cli = Cli::parse();

    // do initialization if 'init' command
    if let Commands::Init { force } = cli.command {
        return init::do_init(force);
    }

    let default_path = config::default_path();
    let default_config_path = default_path.config_path();
    let default_cred_path = default_path.cred_path();

    // read config
    let conf_path = cli.config
        .as_deref()
        .unwrap_or(default_config_path.as_ref());

    let mut conf = Config::build_from_file(conf_path).unwrap_or_else(|err| {
        eprintln!("Failed to build configuration: {:?}", err);
        eprintln!("You should run 'init' first");
        process::exit(1);
    });

    let cred_path = cli.cred.as_deref().unwrap_or(default_cred_path.as_ref());

    match &cli.command {
        Commands::Clone { from, to, ignore_geotag , dry_run} => {
            if let Some(from) = from {
                conf.set_import_from(from.clone());
            }

            if let Some(to) = to {
                conf.set_import_to(to.clone());
            }

            return clone::do_clone(conf, cred_path, *ignore_geotag, *dry_run);
        }
        Commands::Clean => {
            return clean::do_clean(cred_path);
        }
        _ => {
            // never reached
            eprintln!("Unsupported command {:?}", &cli.command);
            process::exit(1);
        }
    }
}
