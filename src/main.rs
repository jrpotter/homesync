use clap::{App, AppSettings, Arg};
use homesync::path::ResPathBuf;
use simplelog;
use simplelog::{error, paris};
use std::{error::Error, io, path::PathBuf};

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
            App::new("apply").about("Find all changes and apply them to the local repository"),
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
        .subcommand(App::new("init").about("Initialize the homesync local repository"))
        .subcommand(App::new("list").about("See which packages homesync manages"))
        .get_matches();

    if let Err(e) = dispatch(matches) {
        error!("{}", e);
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
                Some(("apply", _)) => Ok(homesync::run_apply(config)?),
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
                _ => unreachable!(),
            }
        }
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
