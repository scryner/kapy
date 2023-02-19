mod init;
mod clean;
mod clone;

mod config;
mod processor;
mod drive;
mod progress;
mod login;

use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};
use crate::config::Config;
use crate::drive::auth::ListenPort;

#[derive(Parser)]
#[command(author, version, about = "A copy utility for large images taken by cameras", long_about = None)]
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
    #[command(arg_required_else_help = false)]
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

        /// Import after specific date (YYYY-MM-DD or YYYY-MM or YYYY)
        #[arg(long, value_name = "AFTER")]
        after: Option<String>,
    },
    /// Initialize to make configuration file
    Init {
        /// Force overwritten
        #[arg(long, default_value_t = false)]
        force: bool,
    },

    /// Login to google drive
    Login {
        /// Listen port to exchange token for OAuth2.0
        #[arg(short, long)]
        listen_port: Option<i32>,
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
        Commands::Clone { from, to, ignore_geotag, dry_run,after } => {
            if let Some(from) = from {
                conf.set_import_from(from.clone());
            }

            if let Some(to) = to {
                conf.set_import_to(to.clone());
            }

            return clone::do_clone(conf, cred_path, *ignore_geotag, *dry_run, after.clone());
        }
        Commands::Clean => {
            return clean::do_clean(cred_path);
        }
        Commands::Login { listen_port } => {
            let listen_port = match *listen_port {
                Some(port) => ListenPort::Port(port),
                None => ListenPort::DefaultPort,
            };

            return login::do_login(cred_path, listen_port);
        }
        _ => {
            // never reached
            eprintln!("Unsupported command {:?}", &cli.command);
            process::exit(1);
        }
    }
}
