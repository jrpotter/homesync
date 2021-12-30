use clap::{App, AppSettings, Arg};
use homesync::path::{NormalPathBuf, Normalize};
use std::error::Error;
use std::io;
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
        .subcommand(App::new("daemon").about("Start up a new homesync daemon"))
        .subcommand(App::new("init").about("Initialize the homesync local repository"))
        .subcommand(App::new("list").about("See which packages homesync manages"))
        .subcommand(App::new("pull").about("Pull remote repository into local repository"))
        .subcommand(App::new("push").about("Push local repository to remote repository"))
        .get_matches();

    if let Err(e) = dispatch(matches) {
        eprintln!("{}", e);
    }
}

fn dispatch(matches: clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    let candidates = find_candidates(&matches)?;
    match matches.subcommand() {
        Some(("init", _)) => Ok(homesync::run_init(candidates)?),
        // All subcommands beside `init` require a config. If we invoke any of
        // these, immediately attempt to load our config. Note once a config is
        // loaded, this same config is used throughout the lifetime of the
        // process. We avoid introducing the ability to "change" which config is
        // used, even if one of higher priority is eventually defined.
        subcommand => {
            let config = homesync::config::load(&candidates)?;
            match subcommand {
                Some(("add", _)) => Ok(homesync::run_add(config)?),
                Some(("daemon", _)) => Ok(homesync::run_daemon(config)?),
                Some(("list", _)) => Ok(homesync::run_list(config)?),
                Some(("pull", _)) => Ok(homesync::run_pull(config)?),
                Some(("push", _)) => Ok(homesync::run_push(config)?),
                _ => unreachable!(),
            }
        }
    }
}

fn find_candidates(matches: &clap::ArgMatches) -> Result<Vec<NormalPathBuf>, Box<dyn Error>> {
    let candidates = match matches.value_of("config") {
        Some(config_match) => vec![PathBuf::from(config_match)],
        None => homesync::config::default_paths(),
    };
    let mut normals = vec![];
    for candidate in candidates {
        if let Ok(Normalize::Done(n)) = homesync::path::normalize(&candidate) {
            normals.push(n);
        }
    }
    if normals.is_empty() {
        if let Some(config_match) = matches.value_of("config") {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} is not a valid config path.", config_match),
            ))?
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Could not find a suitable configuration path. Is \
                $XDG_CONFIG_PATH or $HOME defined?",
            ))?
        }
    } else {
        Ok(normals)
    }
}
