use recordroute_core::config::AppConfig;

fn main() {
    let config = AppConfig::from_env();
    println!(
        "recordroute-cli placeholder: db_root={}",
        config.db_root.display()
    );
}
