mod cli;
mod content_lists;
mod generator;
mod kits;
mod model;
mod plugin;
mod records;
mod ui;

fn main() {
    // If any arguments are present, run in CLI mode.
    // With no arguments and the UI feature enabled, launch the GUI.
    if std::env::args().len() > 1 {
        cli::run();
    } else {
        #[cfg(feature = "ui")]
        ui::run();

        #[cfg(not(feature = "ui"))]
        {
            eprintln!("No arguments provided. Built without UI — pass --help for usage.");
            std::process::exit(1);
        }
    }
}
