#![windows_subsystem = "windows"]

use std::env;
use std::process::{self, Command};

fn main() {
    let mut arguments = env::args_os().skip(1);
    let Some(client) = arguments.next() else {
        process::exit(2);
    };

    let mut command = Command::new(client);
    command.args(arguments);
    // Keep the world's original offline UUID even if the launcher has a different global name.
    if let Some(name) = env::var_os("DRAGONFYRE_OFFLINE_PLAYER") {
        command.arg("--name").arg(name);
    }

    match command.status() {
        Ok(status) => process::exit(status.code().unwrap_or(1)),
        Err(error) => {
            eprintln!("Could not start the Dragonfyre client: {error}");
            process::exit(1);
        }
    }
}
