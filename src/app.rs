use crate::agent;
use crate::cli::{self, Command};
use crate::presentation::summary;
use crate::storage::sqlite::Store;
use std::env;

pub fn run() -> Result<(), String> {
    match cli::parse(env::args().skip(1).collect())? {
        Command::Help => {
            cli::print_help();
            Ok(())
        }
        Command::Watch(args) => agent::watch(args),
        Command::Agent(args) => agent::watch(args),
        Command::Summary(args) => {
            let store = Store::open(&args.db_path)?;
            let rows = store.hourly_summary(args.service.as_deref())?;
            summary::print_hourly(&rows)
        }
    }
}
