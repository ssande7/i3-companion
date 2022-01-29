use super::traits::{Configurable, OnEvent};
use async_trait::async_trait;
use std::collections::HashSet;
use tokio_i3ipc::{
    event::{Event, Subscribe},
    I3,
};

/// Layout indicator
pub struct LayoutTracker {}
pub struct LayoutTrackerConfig {}

impl Configurable for LayoutTrackerConfig {
    fn default() -> Self {
        Self {}
    }
    fn from_config(_config: &str) -> Self {
        unimplemented!()
    }
    fn from_cli() -> Self {
        unimplemented!()
    }
}

impl From<&LayoutTrackerConfig> for LayoutTracker {
    fn from(_: &LayoutTrackerConfig) -> Self {
        LayoutTracker {}
    }
}

#[async_trait]
impl OnEvent for LayoutTracker {
    fn add_subscriptions(&self, subs: &mut HashSet<u32>) {
        subs.insert(Subscribe::Window.into());
    }

    async fn handle_event(&mut self, e: &Event, _i3: &mut I3) -> Option<String> {
        match e {
            Event::Window(w) => {
                println!("Window event: {:#?}", w);
                None
            }
            _ => None,
        }
    }
}
