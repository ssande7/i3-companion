use async_trait::async_trait;
use std::{collections::HashSet, time::Duration};
use tokio_i3ipc::{event::Event, I3};

#[async_trait]
pub trait OnEvent {
    // Need to use u32 since Subscribe doesn't impl Eq
    fn add_subscriptions(&self, subs: &mut HashSet<u32>);
    async fn handle_event(&mut self, e: &Event, i3: &mut I3) -> Option<String>;
}

pub trait OnTimer {
    fn spawn_timer(&self, interval: Duration);
}
