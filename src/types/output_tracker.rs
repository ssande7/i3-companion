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
};

pub struct OutputTracker {
    pub ipc_str: String,
    pub pipe: Arc<PipeSender>,
}
#[derive(Deserialize)]
pub struct OutputTrackerConfig {
    pub ipc_str: String,
    pub pipe_name: String,
    pub update_interval: ParsableDuration,
}

impl From<(OutputTrackerConfig, &HashMap<String, Arc<PipeSender>>)> for OutputTracker {
    fn from(config: (OutputTrackerConfig, &HashMap<String, Arc<PipeSender>>)) -> Self {
        let out = Self {
            ipc_str: config.0.ipc_str,
            pipe: config
                .1
                .get(&config.0.pipe_name)
                .unwrap_or_else(|| {
                    eprintln!(
                        "ERROR: pipe '{}' not found in config file",
                        config.0.pipe_name
                    );
                    std::process::exit(6);
                })
                .clone(),
        };
        out.spawn_timer(config.0.update_interval.into());
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
