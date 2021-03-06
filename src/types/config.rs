use super::{
    layout_tracker::{LayoutTracker, LayoutTrackerConfig},
    output_tracker::{OutputTracker, OutputTrackerConfig},
    parsable_duration::ParsableDuration,
    pipe_sender::PipeSender,
    shell_caller::ShellCaller,
    traits::OnEvent,
    ws_history::{WSHistory, WSHistoryConfig},
    MsgSender, SenderType,
};
use dirs::config_dir;
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf, process::exit, sync::Arc, time::Duration};
use toml;

#[derive(Deserialize)]
pub struct I3Timeout(ParsableDuration);
impl From<Duration> for I3Timeout {
    fn from(d: Duration) -> Self {
        Self { 0: d.into() }
    }
}
impl Default for I3Timeout {
    fn default() -> Self {
        Duration::from_secs(3).into()
    }
}

#[derive(Deserialize)]
pub struct I3Interval(ParsableDuration);
impl From<Duration> for I3Interval {
    fn from(d: Duration) -> Self {
        Self { 0: d.into() }
    }
}
impl Default for I3Interval {
    fn default() -> Self {
        Duration::from_millis(3).into()
    }
}

#[derive(Deserialize, Default)]
pub struct TomlConfig {
    #[serde(default)]
    pub connection_timeout: I3Timeout, // secs
    #[serde(default)]
    pub reconnect_interval: I3Interval, // millis
    pub ws_history: Option<WSHistoryConfig>,
    pub layout_tracker: Option<LayoutTrackerConfig>,
    pub output_tracker: Option<OutputTrackerConfig>,
    pub pipes: Option<HashMap<String, (SenderType, String)>>,
}

pub struct Config {
    pub connection_timeout: Duration, // secs
    pub reconnect_interval: Duration, // millis
    pub ws_history: Option<WSHistory>,
    pub layout_tracker: Option<LayoutTracker>,
    pub output_tracker: Option<OutputTracker>,
    pub pipes: Option<HashMap<String, Arc<dyn MsgSender + Send + Sync>>>,
}
impl From<TomlConfig> for Config {
    fn from(config: TomlConfig) -> Self {
        let pipes: Option<HashMap<String, Arc<dyn MsgSender + Send + Sync>>> =
            config.pipes.and_then(|h| {
                Some(
                    h.into_iter()
                        .map(|p| {
                            (
                                p.0,
                                match p.1 .0 {
                                    SenderType::SHELL => Arc::new(ShellCaller::new(p.1 .1))
                                        as Arc<dyn MsgSender + Send + Sync>,
                                    SenderType::PIPE => Arc::new(PipeSender::new(p.1 .1))
                                        as Arc<dyn MsgSender + Send + Sync>,
                                },
                            )
                        })
                        .collect(),
                )
            });
        Self {
            connection_timeout: config.connection_timeout.0.into(),
            reconnect_interval: config.reconnect_interval.0.into(),
            ws_history: config.ws_history.and_then(|c| Some(c.into())),
            layout_tracker: config.layout_tracker.and_then(|c| {
                Some(
                    (
                        c,
                        pipes.as_ref().unwrap_or_else(|| {
                            eprintln!("ERROR: Layout tracker requires a pipe");
                            exit(7);
                        }),
                    )
                        .into(),
                )
            }),
            output_tracker: config.output_tracker.and_then(|c| {
                Some(
                    (
                        c,
                        pipes.as_ref().unwrap_or_else(|| {
                            eprintln!("ERROR: Layout tracker requires a pipe");
                            exit(7);
                        }),
                    )
                        .into(),
                )
            }),
            pipes,
        }
    }
}

fn parse_cli() -> Option<PathBuf> {
    let mut args = std::env::args_os();
    let appname = args.next();
    let mut out: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        if arg == "-c" || arg == "--config" {
            let mut buf = PathBuf::new();
            buf.push(args.next().unwrap_or_else(|| {
                eprintln!("ERROR: missing argument after '-c/--config'");
                exit(1);
            }));
            if !buf.is_file() {
                eprintln!("ERROR: file does not exist\n{}", buf.to_str().unwrap_or(""));
                exit(3);
            }
            out = Some(buf);
        } else if arg == "-h" || arg == "--help" {
            println!(
                "USAGE: {} [-c/--config CONFIG_FILE] [-h/--help]",
                appname
                    .and_then(|n| n.to_str().and_then(|s| Some(s.to_string())))
                    .unwrap_or("i3_companion".into())
            );
            exit(0);
        }
    }
    out
}

impl TomlConfig {
    pub fn new() -> std::io::Result<Self> {
        // TODO: read from command line args or .config/i3-companion/config
        let config_cli = parse_cli();
        let config_content = if let Some(config) = config_cli {
            std::fs::read_to_string(config).ok()
        } else {
            config_dir().and_then(|mut path| {
                path.push("i3-companion/config.toml");
                std::fs::read_to_string(path).ok()
            })
        }
        .ok_or_else(|| {
            eprintln!("Error reading config file");
            exit(3);
        })
        .unwrap();

        toml::from_str(config_content.as_str()).or_else(|e| {
            eprintln!("Error parsing config file:\n{}", e);
            exit(5);
        })
    }
}
impl Config {
    // Send trait not required right now, but keeping for future parallization
    pub fn get_handlers(&mut self) -> Vec<Box<dyn OnEvent + Send>> {
        let mut handlers = Vec::<Box<dyn OnEvent + Send>>::new();
        if let Some(config) = self.ws_history.take() {
            handlers.push(Box::new(config));
        }
        if let Some(config) = self.layout_tracker.take() {
            handlers.push(Box::new(config));
        }
        if let Some(config) = self.output_tracker.take() {
            handlers.push(Box::new(config));
        }
        handlers
    }
}
