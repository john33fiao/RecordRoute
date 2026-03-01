fn main() {
    #[cfg(not(windows))]
    let status = std::process::Command::new("bash")
        .args(["../scripts/install_unix.sh", "--check"])
        .status();

    #[cfg(windows)]
    let status = std::process::Command::new("cmd")
        .args(["/C", "..\\scripts\\install_windows.bat", "--check"])
        .status();

    if let Ok(s) = status {
        if !s.success() {
            panic!("Installation gate failed. Please run the install script first.");
        }
    } else {
        println!("cargo:warning=Failed to execute installation gate script.");
    }
    tauri_build::build();
}
