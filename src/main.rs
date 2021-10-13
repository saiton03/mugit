use clap::{App, load_yaml};

mod init;
mod add;
mod common;
mod config;
mod object;
mod hash;
mod index;
mod commit;
mod head;
mod log;

//#[cfg(feature = "yaml")]
fn main() -> Result<(), String> {
    let yaml = load_yaml!("commands.yml");
    let matches = App::from(yaml).get_matches();

    let res = match matches.subcommand_name() {
        Some("init") => init::run(matches.subcommand_matches("init").unwrap()),
        Some("add") => add::run(matches.subcommand_matches("add").unwrap()),
        Some("commit") => commit::run(matches.subcommand_matches("commit").unwrap()),
        Some("log") => log::run(matches.subcommand_matches("log").unwrap()),

        Some("head") => head::run(matches.subcommand_matches("head").unwrap()),
        None => Ok(()),
        _ => Err("invalid command".to_string()),
    };

    res
}
