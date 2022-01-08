use clap::{App, AppSettings, Arg};
use homesync::path::ResPathBuf;
use std::{error::Error, io, path::PathBuf};
use {
    simplelog,
    simplelog::{error, paris},
};

#[cfg(debug_assertions)]
fn log_level() -> simplelog::LevelFilter {
    simplelog::LevelFilter::Trace
}

#[cfg(not(debug_assertions))]
fn log_level() {
    simplelog::LevelFilter::Info
}

fn main() {
    // Only one logger should ever be initialized and it should be done at the
    // beginning of the program. Otherwise logs are ignored.
    simplelog::TermLogger::init(
        log_level(),
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
        simplelog::ColorChoice::Auto,
    )
    .expect("Could not initialize logger library.");

    let matches = App::new("homesync")
        .about("Cross desktop sync tool.")
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
        .subcommand(
            App::new("apply")
                .about("Copy files from local repository to rest of desktop")
                .arg(
                    Arg::new("package")
                        .value_name("PACKAGE")
                        .conflicts_with("all")
                        .required_unless_present("all")
                        .help("The package we want to configure from the local repository")
                        .takes_value(true),
                )
                .arg(
                    Arg::new("all")
                        .long("all")
                        .conflicts_with("package")
                        .help("Indicates we want to copy all configurations from the local repository")
                        .takes_value(false),
                ),
        )
        .subcommand(
            App::new("daemon")
                .about("Start up a new homesync daemon")
                .arg(
                    Arg::new("frequency")
                        .short('f')
                        .long("frequency")
                        .value_name("FREQUENCY")
                        .help("How often (in seconds) we poll/debounce file system changes")
                        .long_help(
                            "There exists a balance between how responsive changes are \
                    made and how expensive it is to look for changes. Empirically we found the \
                    default value to offer a nice compromise but this can be tweaked based on \
                    preference.",
                        )
                        .takes_value(true)
                        .default_value("5"),
                ),
        )
        .subcommand(App::new("list").about("See which packages homesync manages"))
        .subcommand(App::new("pull").about("Pull changes from remote to local"))
        .subcommand(App::new("push").about("Push changes from local to remote"))
        .subcommand(
            App::new("stage").about("Find all changes and stage them onto the local repository"),
        )
        .get_matches();

    if let Err(e) = dispatch(matches) {
        error!("{}", e);
    }
}

fn dispatch(matches: clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    let candidates = find_candidates(&matches)?;
    let config = homesync::config::load(&candidates)?;
    match matches.subcommand() {
        Some(("apply", matches)) => Ok(homesync::run_apply(config, matches.value_of("package"))?),
        Some(("daemon", matches)) => {
            let freq_secs: u64 = match matches.value_of("frequency") {
                Some(f) => f.parse().unwrap_or(0),
                None => 5,
            };
            if freq_secs > 0 {
                homesync::run_daemon(config, freq_secs)?;
            } else {
                error!("Invalid frequency. Expected a positive integer.");
            }
            Ok(())
        }
        Some(("list", _)) => Ok(homesync::run_list(config)?),
        Some(("pull", _)) => Ok(homesync::run_pull(config)?),
        Some(("push", _)) => Ok(homesync::run_push(config)?),
        Some(("stage", _)) => Ok(homesync::run_stage(config)?),
        _ => unreachable!(),
    }
}

fn find_candidates(matches: &clap::ArgMatches) -> Result<Vec<ResPathBuf>, io::Error> {
    let candidates = match matches.value_of("config") {
        Some(config_match) => vec![PathBuf::from(config_match)],
        None => homesync::config::default_paths(),
    };
    let mut resolved = vec![];
    for candidate in candidates {
        if let Ok(Some(r)) = homesync::path::soft_resolve(&candidate) {
            resolved.push(r);
        }
    }
    if resolved.is_empty() {
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
        Ok(resolved)
    }
}
