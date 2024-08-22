use anki_md_sync::{
    config::{load_config, AnkiSyncConfigError},
    AnkiSync,
};
use clap::{command, Arg, ArgMatches};
use env_logger::Builder;
use log::{error, info, LevelFilter};
use std::{env, error::Error, path::PathBuf};

const FILES_ARG: &str = "files";
const DEBUG_ARG: &str = "debug";
const CONFIG_ARG: &str = "config";

const HOME_VAR: &str = "HOME";
const CONFIG_PATH: &str = ".config/anki-md-sync";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = parse_args();

    let mut logging_builder = Builder::new();
    let log_level = if *args.get_one::<bool>(DEBUG_ARG).unwrap() {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    logging_builder.filter_level(log_level).init();

    let mut anki_sync = AnkiSync::default();

    let mut files: Vec<PathBuf> = Vec::new();

    if let Some(file_paths) = args.remove_many::<PathBuf>(FILES_ARG) {
        files.extend(file_paths);
    }

    if let Some(true) = args.get_one::<bool>(CONFIG_ARG) {
        let mut config_path = PathBuf::new();
        let home =
            env::var_os(HOME_VAR).ok_or(AnkiSyncConfigError::EnvVarMissing(HOME_VAR.to_owned()))?;
        config_path.push(home);
        config_path.push(CONFIG_PATH);

        info!("Reading config file at {:?}", config_path);

        let found_paths = load_config(&config_path)?;
        for path in found_paths {
            info!("Found file from config: {:?}", path);
        }

        files.extend(load_config(&config_path)?);
    }

    if files.is_empty() {
        info!("No files specified to sync!");
    }

    for f in files {
        info!("Syncing file {:?}...", f);
        match anki_sync.sync_file(&f).await {
            Ok(_) => info!("Done syncing file {:?}!", f),
            Err(e) => error!("Error while syncing: {}", e),
        }
    }

    Ok(())
}

fn parse_args() -> ArgMatches {
    command!()
        .arg(
            Arg::new(FILES_ARG)
                .short('f')
                .value_parser(clap::value_parser!(PathBuf))
                .num_args(0..)
                .help("Markdown files to parse and sync into Anki"),
        )
        .arg(
            Arg::new(DEBUG_ARG)
                .short('d')
                .action(clap::ArgAction::SetTrue)
                .help("Enables debug logging"),
        )
        .arg(
            Arg::new(CONFIG_ARG)
                .short('c')
                .action(clap::ArgAction::SetTrue)
                .help("Use markdown files specified in config file"),
        )
        .get_matches()
}
