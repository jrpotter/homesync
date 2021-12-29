use clap::{App, AppSettings};

fn main() {
    let matches = App::new("homesync")
        .about("Cross desktop configuration sync tool.")
        .version("0.1.0")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .author("Joshua Potter <jrpotter.github.io>")
        .subcommand(App::new("configure").about("Initialize the homesync local repository."))
        .subcommand(App::new("push").about("Push local repository to remote repository."))
        .subcommand(App::new("pull").about("Pull remote repository into local repository."))
        .subcommand(App::new("add").about("Add new configuration to local repository."))
        .get_matches();

    match matches.subcommand() {
        Some(("configure", ms)) => homesync::run_configure(ms).unwrap(),
        Some(("push", ms)) => homesync::run_push(ms).unwrap(),
        Some(("pull", ms)) => homesync::run_pull(ms).unwrap(),
        Some(("add", ms)) => homesync::run_add(ms).unwrap(),
        _ => unreachable!(),
    }
}
