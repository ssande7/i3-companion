use super::{
    pipe_sender::PipeSender,
    traits::{Configurable, OnEvent},
};
use async_trait::async_trait;
use regex::Regex;
use std::collections::HashSet; //, fs::OpenOptions, io::Write, time::Duration};
use tokio_i3ipc::{
    event::{Event, Subscribe},
    reply::Node,
    I3,
};

/// Layout indicator
pub struct LayoutTracker {
    pub fmt_regex: Regex,
    pub pipe_echo_fmt: String,
    pub pipe: PipeSender,
}
pub struct LayoutTrackerConfig {
    pub pipe_echo_fmt: String,
    pub bar_pipe_glob: String,
}

impl Configurable for LayoutTrackerConfig {
    fn default() -> Self {
        Self {
            pipe_echo_fmt: "hook:module/i3_layout{}".into(),
            bar_pipe_glob: "/tmp/polybar_mqueue.*".into(),
        }
    }
    fn from_config(_config: &str) -> Self {
        unimplemented!()
    }
    fn from_cli() -> Self {
        unimplemented!()
    }
}

impl From<&LayoutTrackerConfig> for LayoutTracker {
    fn from(config: &LayoutTrackerConfig) -> Self {
        Self {
            fmt_regex: Regex::new("\\{\\}").unwrap(),
            pipe_echo_fmt: config.pipe_echo_fmt.clone(),
            pipe: PipeSender {
                bar_pipe_glob: config.bar_pipe_glob.clone(),
            },
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
                            parent.layout
                        } else {
                            focused.focused.layout
                        };
                        self.pipe.send(
                            self.fmt_regex
                                .replace_all(
                                    &self.pipe_echo_fmt[..],
                                    ((layout as i32) + 1).to_string(),
                                )
                                .as_ref(),
                        );
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
