use std::collections::HashSet;

use async_trait::async_trait;
use tokio_i3ipc::{
    event::{Event, Subscribe},
    I3,
};

use super::{
    traits::{OnEvent, Configurable},
    pipe_sender::PipeSender,
};

pub struct OutputTracker {
    pub ipc_str: String,
    pub pipe: PipeSender,
}
pub struct OutputTrackerConfig {
    pub ipc_str: String,
    pub bar_pipe_glob: String,
}

impl Configurable for OutputTrackerConfig {
    fn default() -> Self {
        Self {
            ipc_str: "hook:module/date1".into(),
            bar_pipe_glob: "/tmp/polybar_mqueue.*".into(),
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
        Self {
            ipc_str: config.ipc_str.clone(),
            pipe: PipeSender {
                bar_pipe_glob: config.bar_pipe_glob.clone(),
            }
        }
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
