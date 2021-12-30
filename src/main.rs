use clap::{App, AppSettings, Arg};
use std::path::PathBuf;

fn main() {
    let matches = App::new("homesync")
        .about("Cross desktop configuration sync tool.")
        .version("0.1.0")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .author("Joshua Potter <jrpotter.github.io>")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Specify a configuration file to use in place of defaults")
                .takes_value(true),
        )
        .subcommand(App::new("add").about("Add new configuration to local repository"))
        .subcommand(App::new("init").about("Initialize the homesync local repository"))
        .subcommand(App::new("list").about("See which packages homesync manages"))
        .subcommand(App::new("pull").about("Pull remote repository into local repository"))
        .subcommand(App::new("push").about("Push local repository to remote repository"))
        .get_matches();

    let paths = match matches.value_of("config") {
        Some(path) => vec![PathBuf::from(path)],
        None => homesync::config::default_paths(),
    };

    match matches.subcommand() {
        Some(("add", _)) => {
            if let Err(e) = homesync::run_add(paths) {
                eprintln!("{}", e);
            }
        }
        Some(("init", _)) => {
            if let Err(e) = homesync::run_init(paths) {
                eprintln!("{}", e);
            }
        }
        Some(("list", _)) => {
            if let Err(e) = homesync::run_list(paths) {
                eprintln!("{}", e);
            }
        }
        Some(("pull", ms)) => {
            homesync::run_pull(ms);
        }
        Some(("push", ms)) => {
            homesync::run_push(ms);
        }
        _ => unreachable!(),
    }
}
