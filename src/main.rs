fn main() {
    if let Err(err) = gsw::app::run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
