use clap::Parser;
use std::process;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
struct Args {
    #[clap(short, long)]
    config: Option<String>,
}

fn main() {
    let args = Args::parse();
    homesync::run(args.config).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {}", err);
        process::exit(1);
    });
}
