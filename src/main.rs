use anki_md_sync::AnkiSync;
use clap::{Arg, ArgMatches, command};
use std::path::PathBuf;
use env_logger::Builder;
use log::{error, info, LevelFilter};

const FILES_ARG: &str = "files";
const DEBUG_ARG: &str = "debug";

#[tokio::main]
async fn main() {
    let args = parse_args();
    
    let mut logging_builder = Builder::new();
    let log_level = if *args.get_one::<bool>(&DEBUG_ARG).unwrap() {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    logging_builder.filter_level(log_level).init();

    let mut anki_sync = AnkiSync::new();
    
    let files = args.get_many::<PathBuf>(&FILES_ARG)
        .unwrap();
    for f in files {
        info!("Syncing file {:?}...", f);
        match anki_sync.sync_file(f).await {
            Ok(_) => info!("Done syncing file {:?}!", f),
            Err(e) => error!("Error while syncing: {}", e),
        }
    }
}

fn parse_args() -> ArgMatches {
    command!()
        .arg(Arg::new(&FILES_ARG)
            .short('f')
            .value_parser(clap::value_parser!(PathBuf))
            .num_args(0..)
            .required(true)
            .help("Markdown files to parse and sync into Anki")
        )
        .arg(Arg::new(&DEBUG_ARG)
            .short('d')
            .action(clap::ArgAction::SetTrue)
            .help("Enables debug logging")
        )
        .get_matches()
}
