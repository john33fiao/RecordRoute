fn main() {
    if let Err(error) = recordroute_rust::main_server() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
