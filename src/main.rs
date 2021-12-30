use clap::{App, AppSettings, Arg};
use std::error::Error;
use std::path::PathBuf;

fn dispatch(paths: Vec<PathBuf>, matches: clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    match matches.subcommand() {
        Some(("add", _)) => homesync::run_add(paths)?,
        Some(("daemon", _)) => homesync::run_daemon(paths)?,
        Some(("init", _)) => homesync::run_init(paths)?,
        Some(("list", _)) => homesync::run_list(paths)?,
        Some(("pull", _)) => homesync::run_pull()?,
        Some(("push", _)) => homesync::run_push()?,
        _ => unreachable!(),
    };
    Ok(())
}

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
        .subcommand(App::new("daemon").about("Start up a new homesync daemon"))
        .subcommand(App::new("init").about("Initialize the homesync local repository"))
        .subcommand(App::new("list").about("See which packages homesync manages"))
        .subcommand(App::new("pull").about("Pull remote repository into local repository"))
        .subcommand(App::new("push").about("Push local repository to remote repository"))
        .get_matches();

    let candidates = match matches.value_of("config") {
        Some(path) => vec![PathBuf::from(path)],
        None => homesync::config::default_paths(),
    };

    if let Err(e) = dispatch(candidates, matches) {
        eprintln!("{}", e);
    }
}
