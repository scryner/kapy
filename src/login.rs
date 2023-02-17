use std::path::Path;
use std::process;
use console::style;
use crate::drive::auth::{CredPath, GoogleAuthenticator, ListenPort};

pub fn do_login(cred_path: &Path, listen_port: ListenPort) {
    println!("Login to google drive...");

    // try to login
    print!("\tTrying to login...");
    let auth = GoogleAuthenticator::new(listen_port, CredPath::Path(cred_path));
    match auth.access_token() {
        Ok(_) => println!("\t{}", style("[  OK  ]").green()),
        Err(e) => {
            println!("\t{}", style("[FAILED]").red());
            eprintln!("Failed to login: {}", e);
            process::exit(1);
        }
    }
}
