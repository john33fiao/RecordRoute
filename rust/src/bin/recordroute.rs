fn main() {
    if let Err(error) = recordroute_rust::main_launcher() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
