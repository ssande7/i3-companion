use serde::Deserialize;

pub mod keybinding;
pub mod layout_tracker;
pub mod output_tracker;
pub mod parsable_duration;
pub mod pipe_sender;
pub mod shell_caller;
pub mod traits;
pub mod ws_history;
pub mod config;

#[derive(Deserialize)]
pub enum SenderType {
    SHELL,
    PIPE,
}
pub trait MsgSender {
    fn send(&self, msg: &str);
}
