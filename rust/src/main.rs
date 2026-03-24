fn main() {
    if let Err(error) = recordroute_rust::main_cli() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
