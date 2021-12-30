use clap::{App, AppSettings, Arg};
use std::error::Error;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
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
        .subcommand(App::new("configure").about("Initialize the homesync local repository"))
        .subcommand(App::new("push").about("Push local repository to remote repository"))
        .subcommand(App::new("pull").about("Pull remote repository into local repository"))
        .subcommand(App::new("add").about("Add new configuration to local repository"))
        .get_matches();

    let configs = match matches.value_of("config") {
        Some(path) => vec![PathBuf::from(path)],
        None => homesync::config::default_configs(),
    };

    match matches.subcommand() {
        Some(("configure", ms)) => homesync::run_configure(configs, ms),
        Some(("push", ms)) => homesync::run_push(ms),
        Some(("pull", ms)) => homesync::run_pull(ms),
        Some(("add", ms)) => homesync::run_add(ms),
        _ => unreachable!(),
    }
}
