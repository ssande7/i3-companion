use std::{collections::HashSet, thread, time::Duration};

use async_trait::async_trait;
use tokio_i3ipc::{
    event::{Event, Subscribe},
    I3,
};

use super::{
    pipe_sender::PipeSender,
    traits::{Configurable, OnEvent, OnTimer},
};

pub struct OutputTracker {
    pub ipc_str: String,
    pub pipe: PipeSender,
}
pub struct OutputTrackerConfig {
    pub ipc_str: String,
    pub bar_pipe_glob: String,
    pub update_interval: Duration,
}

impl Configurable for OutputTrackerConfig {
    fn default() -> Self {
        Self {
            ipc_str: "hook:module/date1".into(),
            bar_pipe_glob: "/tmp/polybar_mqueue.*".into(),
            update_interval: Duration::from_secs(5),
        }
    }
    fn from_cli() -> Self {
        unimplemented!()
    }
    fn from_config(_config: &str) -> Self {
        unimplemented!()
    }
}

impl From<&OutputTrackerConfig> for OutputTracker {
    fn from(config: &OutputTrackerConfig) -> Self {
        let out = Self {
            ipc_str: config.ipc_str.clone(),
            pipe: PipeSender {
                bar_pipe_glob: config.bar_pipe_glob.clone(),
            },
        };
        out.spawn_timer(config.update_interval);
        out
    }
}

impl OnTimer for OutputTracker {
    fn spawn_timer(&self, interval: Duration) {
        let pipe = self.pipe.clone();
        let text = self.ipc_str.clone();
        thread::spawn(move || loop {
            pipe.send(&text[..]);
            thread::sleep(interval);
        });
    }
}

#[async_trait]
impl OnEvent for OutputTracker {
    fn add_subscriptions(&self, subs: &mut HashSet<u32>) {
        subs.insert(Subscribe::Output.into());
    }
    async fn handle_event(&mut self, e: &Event, _i3: &mut I3) -> Option<String> {
        if let Event::Output(_) = e {
            self.pipe.send(&self.ipc_str[..]);
        }
        None
    }
}
