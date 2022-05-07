use super::{pipe_sender::PipeSender, traits::OnEvent, MsgSender};
use async_trait::async_trait;
use regex::Regex;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    thread,
};
use tokio_i3ipc::{
    event::{Event, Subscribe},
    reply::Node,
    I3,
};

/// Layout indicator
pub struct LayoutTracker {
    fmt_regex: Regex,
    cur_layout: i32,
    pub pipe_echo_fmt: String,
    pub pipe: Arc<dyn MsgSender + Send + Sync>,
}
#[derive(Deserialize)]
pub struct LayoutTrackerConfig {
    pub pipe_echo_fmt: String,
    pub pipe_name: String,
}

impl Default for LayoutTracker {
    fn default() -> Self {
        Self {
            fmt_regex: Regex::new("\\{\\}").unwrap(),
            cur_layout: -1,
            pipe_echo_fmt: "hook:module/i3_layout{}".into(),
            pipe: Arc::new(PipeSender::new("/tmp/polybar_mqueue.*".into())),
        }
    }
}

impl
    From<(
        LayoutTrackerConfig,
        &HashMap<String, Arc<dyn MsgSender + Send + Sync>>,
    )> for LayoutTracker
{
    fn from(
        (config, pipes): (
            LayoutTrackerConfig,
            &HashMap<String, Arc<dyn MsgSender + Send + Sync>>,
        ),
    ) -> Self {
        Self {
            fmt_regex: Regex::new("\\{\\}").unwrap(),
            cur_layout: -1,
            pipe_echo_fmt: config.pipe_echo_fmt,
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
        }
    }
}

#[async_trait]
impl OnEvent for LayoutTracker {
    fn add_subscriptions(&self, subs: &mut HashSet<u32>) {
        subs.insert(Subscribe::Tick.into());
        subs.insert(Subscribe::Workspace.into());
        subs.insert(Subscribe::Window.into());
    }

    async fn handle_event(&mut self, e: &Event, i3: &mut I3) -> Option<String> {
        match e {
            Event::Window(_) | Event::Workspace(_) | Event::Tick(_) => {
                if let Ok(tree) = &i3.get_tree().await {
                    let layout = if let Some(focused) = get_focused_node(tree.into()) {
                        if let Some(parent) = focused.parent {
                            parent.layout as i32
                        } else {
                            focused.focused.layout as i32
                        }
                    } else {
                        6 // floating
                    };
                    if self.cur_layout != layout {
                        self.cur_layout = layout;
                        let pipe = self.pipe.clone();
                        let msg = self
                            .fmt_regex
                            .replace_all(&self.pipe_echo_fmt[..], self.cur_layout.to_string())
                            .to_string();
                        thread::spawn(move || {
                            pipe.send(msg.as_str());
                        });
                    }
                }
            }

            _ => (),
        }
        None
    }
}

#[derive(Debug, Clone, Copy)]
struct FocusedNode<'a> {
    focused: &'a Node,
    parent: Option<&'a Node>, // Need to track parent since that contains the correct layout information
}
impl<'a> From<(&'a Node, &'a Node)> for FocusedNode<'a> {
    fn from((focused, parent): (&'a Node, &'a Node)) -> Self {
        Self {
            focused,
            parent: Some(parent),
        }
    }
}
impl<'a> From<&'a Node> for FocusedNode<'a> {
    fn from(focused: &'a Node) -> Self {
        Self {
            focused,
            parent: None,
        }
    }
}
fn get_focused_node<'a>(node: FocusedNode<'a>) -> Option<FocusedNode<'a>> {
    if node.focused.focus.len() == 0 || node.focused.focused {
        if node.focused.focused {
            Some(node)
        } else {
            None // Should never happen unless there's a problem with i3
        }
    } else if let Some(focused) = node
        .focused
        .nodes
        .iter()
        .find(|&n| n.id == node.focused.focus[0])
    {
        get_focused_node((focused, node.focused).into())
    } else {
        None // Floating window causes this
    }
}
