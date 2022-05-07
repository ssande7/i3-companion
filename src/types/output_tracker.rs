use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    thread,
    time::Duration,
};

use async_trait::async_trait;
use serde::Deserialize;
use tokio_i3ipc::{
    event::{Event, Subscribe},
    I3,
};

use super::{
    parsable_duration::ParsableDuration,
    pipe_sender::PipeSender,
    traits::{OnEvent, OnTimer},
    MsgSender,
};

pub struct OutputTracker {
    pub ipc_str: String,
    pub pipe: Arc<dyn MsgSender + Send + Sync>,
}
#[derive(Deserialize)]
pub struct OutputTrackerConfig {
    pub ipc_str: String,
    pub pipe_name: String,
    pub update_interval: Option<ParsableDuration>,
}

impl From<(OutputTrackerConfig, &HashMap<String, Arc<dyn MsgSender + Send + Sync>>)> for OutputTracker {
    fn from((config, pipes): (OutputTrackerConfig, &HashMap<String, Arc<dyn MsgSender + Send + Sync>>)) -> Self {
        let out = Self {
            ipc_str: config.ipc_str,
            pipe: pipes
                .get(&config.pipe_name)
                .unwrap_or_else(|| {
                    eprintln!(
                        "ERROR: pipe '{}' not found in config file",
                        config.pipe_name
                    );
                    std::process::exit(6);
                })
                .clone(),
        };
        if let Some(interval) = config.update_interval {
            out.spawn_timer(interval.into());
        }
        out
    }
}

impl Default for OutputTracker {
    fn default() -> Self {
        let out = Self {
            ipc_str: "hook:module/date1".into(),
            pipe: Arc::new(PipeSender::new("/tmp/polybar_mqueue.*".into())),
        };
        out.spawn_timer(Duration::from_secs(5));
        out
    }
}

impl OnTimer for OutputTracker {
    fn spawn_timer(&self, interval: Duration) {
        let pipe = self.pipe.clone();
        let text = self.ipc_str.clone();
        thread::spawn(move || {
            let msg = text;
            loop {
                pipe.send(msg.as_str());
                thread::sleep(interval);
            }
        });
    }
}

#[async_trait]
impl OnEvent for OutputTracker {
    fn add_subscriptions(&self, subs: &mut HashSet<u32>) {
        subs.insert(Subscribe::Workspace.into());
    }
    async fn handle_event(&mut self, e: &Event, _i3: &mut I3) -> Option<String> {
        if let Event::Workspace(_) = e {
            let pipe = self.pipe.clone();
            let msg = self.ipc_str.clone();
            thread::spawn(move || {
                pipe.send(msg.as_str());
            });
        }
        None
    }
}
