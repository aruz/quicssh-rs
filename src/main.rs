mod client;
mod server;

use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;

use clap::{Parser, Subcommand};
use log::{error, LevelFilter};
use std::{path::PathBuf, str};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Location of log, Default if
    #[clap(value_parser, long = "log")]
    log_file: Option<PathBuf>,
    /// Log level, Default Error
    #[clap(long)]
    log_level: Option<LevelFilter>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Server
    Server(server::Opt),
    /// Client
    Client(client::Opt),
}

/// Creates a log4rs configuration based on log file and level settings
///
/// # Arguments
/// * `log_file` - Optional path to log file. If None, logs to stderr.
/// * `log_level` - Optional log level. Defaults to Error if None.
///
/// # Returns
/// A log4rs Config object ready for initialization
fn create_log_config(log_file: Option<PathBuf>, log_level: Option<LevelFilter>) -> Config {
    let level = log_level.unwrap_or(LevelFilter::Error);

    match log_file {
        Some(log_file) => {
            let logfile = FileAppender::builder()
                .encoder(Box::<PatternEncoder>::default())
                .build(log_file)
                .unwrap();

            Config::builder()
                .appender(Appender::builder().build("logfile", Box::new(logfile)))
                .build(Root::builder().appender("logfile").build(level))
                .unwrap()
        }
        None => {
            let stderr = ConsoleAppender::builder()
                .encoder(Box::<PatternEncoder>::default())
                .target(Target::Stderr)
                .build();
            Config::builder()
                .appender(Appender::builder().build("stderr", Box::new(stderr)))
                .build(Root::builder().appender("stderr").build(level))
                .unwrap()
        }
    }
}

fn main() {
    let args = Cli::parse();

    let config = create_log_config(args.log_file, args.log_level);
    log4rs::init_config(config).unwrap();

    match args.command {
        Commands::Server(server) => {
            let err = server::run(server);
            match err {
                Ok(_) => {}
                Err(e) => {
                    error!("Error: {:#?}", e);
                }
            }
        }
        Commands::Client(client) => {
            let err = client::run(client);
            match err {
                Ok(_) => {}
                Err(e) => {
                    error!("Error: {:#?}", e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_log_config_with_stderr_default_level() {
        let config = create_log_config(None, None);
        // Verify config is created without panicking
        // The default level should be Error
        assert_eq!(config.root().level(), LevelFilter::Error);
    }

    #[test]
    fn test_create_log_config_with_stderr_custom_level() {
        let config = create_log_config(None, Some(LevelFilter::Debug));
        assert_eq!(config.root().level(), LevelFilter::Debug);
    }

    #[test]
    fn test_create_log_config_with_file() {
        use std::env;
        let temp_dir = env::temp_dir();
        let log_file = temp_dir.join("test_quicssh.log");

        let config = create_log_config(Some(log_file.clone()), Some(LevelFilter::Info));
        assert_eq!(config.root().level(), LevelFilter::Info);

        // Clean up
        let _ = std::fs::remove_file(log_file);
    }

    #[test]
    fn test_create_log_config_appenders() {
        // Test that stderr config creates "stderr" appender
        let config = create_log_config(None, None);
        assert_eq!(config.root().appenders().len(), 1);
        assert!(config.root().appenders().contains(&"stderr".to_string()));

        // Test that file config creates "logfile" appender
        use std::env;
        let temp_dir = env::temp_dir();
        let log_file = temp_dir.join("test_appender.log");
        let config = create_log_config(Some(log_file.clone()), None);
        assert_eq!(config.root().appenders().len(), 1);
        assert!(config.root().appenders().contains(&"logfile".to_string()));

        // Clean up
        let _ = std::fs::remove_file(log_file);
    }
}
