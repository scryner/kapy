use std::{fs, process};
use std::path::Path;
use console::style;

pub fn do_clean(cred_path: &Path) {
    println!("Cleaning kapy...");

    // try to remove credentials
    print!("\tRemoving credentials...");
    match fs::remove_file(&cred_path) {
        Ok(_) => println!("\t{}", style("[  OK  ]").green()),
        Err(e) => {
            println!("\t{}", style("[FAILED]").red());
            eprintln!("Failed to remove credentials: {}", e);
            process::exit(1);
        }
    }
}