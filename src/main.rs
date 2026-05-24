mod app;
mod cli;
mod metrics;
mod process;
mod procfs;
mod sqlite_store;
mod summary;
mod terminal;
mod time;

fn main() {
    if let Err(err) = app::run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
