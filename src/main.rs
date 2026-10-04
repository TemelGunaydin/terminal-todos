use clap::Parser;
use terminal_todos::{
    cli::{self, Cli, Command},
    model::safe,
    store::Store,
    ui,
};

fn main() {
    let cli = Cli::parse();
    let result = (|| {
        let store = Store::from_environment()?;
        match &cli.command {
            None | Some(Command::Tui) => ui::run(store, cli.colors()),
            Some(command) => {
                cli::run(command, &store, cli.colors())?;
                Ok(0)
            }
        }
    })();
    let code = match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("Error: {}", safe(&format!("{error:#}")));
            1
        }
    };
    // All terminal and storage guards have already been dropped.
    std::process::exit(code);
}
