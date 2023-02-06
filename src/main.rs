use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};
use kapy::config::Config;

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

#[derive(Subcommand)]
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
}

fn main() {
    let cli = Cli::parse();

    let default_config_path = home::home_dir().unwrap_or_else(|| {
        // actually never reached
        eprintln!("Failed get home directory");
        process::exit(1);
    }).join(".kapy.yaml");

    let conf_path = cli.config
        .as_deref()
        .unwrap_or(&default_config_path);

    let conf = Config::build_from_file(conf_path).unwrap_or_else(|err| {
        eprintln!("Failed to build configuration: {:?}", err);
        process::exit(1);
    });

    match &cli.command {
        Commands::Clone { from, to } => {
            println!("in command Clone...");
        }
        _ => ()
    }
}
