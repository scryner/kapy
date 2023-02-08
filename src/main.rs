mod clone;
mod init;
mod config;

use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};
use config::Config;

#[derive(Parser)]
#[command(name = "kapy")]
#[command(author = "scryner <scryner@gmail.com>")]
#[command(about = "A copy utility for large images taken by cameras", long_about = None)]
struct Cli {
    /// Set a custom config file
    #[arg(short, long, value_name = "FILE", global = true)]
    config: Option<PathBuf>,

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
    },
    /// Initialize to make configuration file
    Init {
        /// Force overwritten
        #[arg(long, default_value_t = false)]
        force: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    if let Commands::Init { force } = cli.command {
        return init::do_init(force);
    }

    let default_config_path = match config::default_config_path() {
        Ok((_, p)) => p,
        Err(e) => {
            eprintln!("Failed to get default config path: {}", e.to_string());
            process::exit(1);
        }
    };

    let conf_path = cli.config
        .as_deref()
        .unwrap_or(&default_config_path);

    let conf = Config::build_from_file(conf_path).unwrap_or_else(|err| {
        eprintln!("Failed to build configuration: {:?}", err);
        eprintln!("You should run 'init' first");
        process::exit(1);
    });

    match &cli.command {
        Commands::Clone { from, to } => {
            return clone::do_clone(conf);
        }
        _ => {
            // never reached
            eprintln!("Unsupported command {:?}", &cli.command);
            process::exit(1);
        }
    }
}
