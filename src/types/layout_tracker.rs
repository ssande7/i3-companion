use super::{
    pipe_sender::PipeSender,
    traits::OnEvent,
};
use async_trait::async_trait;
use regex::Regex;
use serde::Deserialize;
use std::{collections::{HashSet, HashMap}, sync::Arc, thread}; //, fs::OpenOptions, io::Write, time::Duration};
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
    pub pipe: Arc<PipeSender>,
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

impl From<(LayoutTrackerConfig, &HashMap<String, Arc<PipeSender>>)> for LayoutTracker {
    fn from(config: (LayoutTrackerConfig, &HashMap<String, Arc<PipeSender>>)) -> Self {
        Self {
            fmt_regex: Regex::new("\\{\\}").unwrap(),
            cur_layout: -1,
            pipe_echo_fmt: config.0.pipe_echo_fmt,
            pipe: config.1.get(&config.0.pipe_name).unwrap_or_else(|| {
                eprintln!("ERROR: pipe '{}' not found in config file", config.0.pipe_name);
                std::process::exit(6);
            }).clone(),
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
                    if let Some(focused) = get_focused_node(tree.into()) {
                        let layout = if let Some(parent) = focused.parent {
                            parent.layout as i32 + 1
                        } else {
                            focused.focused.layout as i32 + 1
                        };
                        if self.cur_layout != layout {
                            self.cur_layout = layout;
                            let pipe = self.pipe.clone();
                            let msg = self.fmt_regex
                                    .replace_all(
                                        &self.pipe_echo_fmt[..],
                                        self.cur_layout.to_string(),
                                    ).to_string();
                            thread::spawn(move || {
                                pipe.send(msg.as_str());
                            });
                        }
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
        None // Should never happen unless there's a problem with i3
    }
}
