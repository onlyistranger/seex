fn main() {
    let mut args = std::env::args().skip(1);
    if let Some(flag) = args.next() {
        match flag.as_str() {
            "-h" | "--help" => {
                println!("happyjlc-tui");
                println!();
                println!("Interactive ratatui frontend for HappyJLC.");
                println!();
                println!("Usage:");
                println!("  happyjlc-tui");
                println!("  happyjlc-tui --help");
                println!("  happyjlc-tui --version");
                return;
            }
            "-V" | "--version" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return;
            }
            _ => {}
        }
    }

    if let Err(err) = happyjlc_desktop::tui::run() {
        eprintln!("happyjlc-tui failed: {err}");
        std::process::exit(1);
    }
}
