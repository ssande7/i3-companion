use super::{keybinding::KeyBinding, parsable_duration::ParsableDuration, traits::OnEvent};
use async_trait::async_trait;
use serde::Deserialize;
use std::{
    collections::{vec_deque::VecDeque, HashMap, HashSet},
    ops::{Add, AddAssign, Index},
    time::{Duration, Instant},
};
use tokio_i3ipc::{
    event as I3Event,
    event::{Event, Subscribe, WorkspaceChange},
    I3,
};

/// Config setting for history stack type
#[derive(Clone, Copy, Deserialize)]
pub enum HistTypeConfig {
    Single,
    PerOutput,
}

/// History stack type (single or per-output)
enum HistType {
    Single(History),
    PerOutput(HashMap<String, History>),
}
impl From<(HistTypeConfig, usize)> for HistType {
    fn from(config: (HistTypeConfig, usize)) -> Self {
        match config.0 {
            HistTypeConfig::Single => Self::Single(History::with_capacity(config.1)),
            HistTypeConfig::PerOutput => Self::PerOutput(HashMap::new()),
        }
    }
}

/// History stack
struct History {
    hist: VecDeque<i32>,
    hist_ptr: usize,
}
impl History {
    fn with_capacity(hist_sz: usize) -> Self {
        Self {
            hist: VecDeque::with_capacity(hist_sz),
            hist_ptr: 0,
        }
    }
    fn len(&self) -> usize {
        self.hist.len()
    }
    /// Reset the history pointer, reversing the order of history before it
    /// NOTE: may change `ws_hist.len()`
    fn reset_ptr(&mut self) {
        if self.hist_ptr > 0 {
            // Reverse order of history that has been cycled back through,
            // preventing double ups
            if self.hist_ptr < self.hist.len() - 1 && self.hist[self.hist_ptr + 1] == self.hist[0] {
                self.hist.pop_front();
            }
            for i in 0..=self.hist_ptr / 2 {
                self.hist.swap(i, self.hist_ptr - i);
            }
            self.hist_ptr = 0;
        }
    }
}
impl Index<usize> for History {
    type Output = i32;
    fn index(&self, index: usize) -> &Self::Output {
        &self.hist[index]
    }
}

/// Internal manager for workspace history stack
struct HistoryManager {
    hist: HistType,
    hist_sz: usize,
}
impl From<(HistTypeConfig, usize)> for HistoryManager {
    fn from(config: (HistTypeConfig, usize)) -> Self {
        Self {
            hist_sz: config.1,
            hist: config.into(),
        }
    }
}
impl HistoryManager {
    fn get(&self, output: &String) -> Option<&History> {
        match &self.hist {
            HistType::Single(hist) => Some(hist),
            HistType::PerOutput(hist) => hist.get(output),
        }
    }
    fn get_mut(&mut self, output: &String) -> Option<&mut History> {
        match &mut self.hist {
            HistType::Single(hist) => Some(hist),
            HistType::PerOutput(hist) => hist.get_mut(output),
        }
    }
    fn get_or_add_mut(&mut self, output: &String) -> &mut History {
        match &mut self.hist {
            HistType::Single(hist) => hist,
            HistType::PerOutput(hist) => {
                if !hist.contains_key(output) {
                    hist.insert(output.clone(), History::with_capacity(self.hist_sz));
                }
                hist.get_mut(output).unwrap() // Should never panic
            }
        }
    }
    fn display(&self, output: &String) -> Result<String, ()> {
        let hist = self.get(output).ok_or(())?;
        let mut out = String::with_capacity(6 * self.hist_sz);
        for (id, ws) in hist.hist.iter().enumerate() {
            if id == hist.hist_ptr {
                out.push_str(format!("{}\t<-\n", ws).as_str());
            } else {
                out.push_str(format!("{}\n", ws).as_str());
            }
        }
        Ok(out)
    }
}

/// Interface struct for workspace history stack
pub struct WSHistory {
    hist: HistoryManager,
    ignore_ctr: usize,
    activity_timer: Instant,
    activity_timeout: Option<Duration>,
    cur_output: String,
    pub skip_visible: bool,
    pub binding_prev: Option<KeyBinding>,
    pub binding_move_prev: Option<KeyBinding>,
    pub binding_next: Option<KeyBinding>,
    pub binding_move_next: Option<KeyBinding>,
    pub binding_swap_prev: Option<KeyBinding>,
    pub binding_swap_next: Option<KeyBinding>,
    pub binding_reset: Option<KeyBinding>,
    pub binding_to_head: Option<KeyBinding>,
    pub binding_move_to_head: Option<KeyBinding>,
    pub binding_rem_and_prev: Option<KeyBinding>,
    pub binding_rem_and_next: Option<KeyBinding>,
    pub binding_show_stack: Option<KeyBinding>,
}

// serde default values
fn default_hist_sz() -> usize {
    20
}
fn default_skip_visible() -> bool {
    true
}
fn default_hist_type() -> HistTypeConfig {
    HistTypeConfig::PerOutput
}

/// Config input for `WSHistory`
#[derive(Deserialize)]
pub struct WSHistoryConfig {
    #[serde(default = "default_hist_sz")]
    pub hist_sz: usize,
    #[serde(default = "default_hist_type")]
    pub hist_type: HistTypeConfig,
    #[serde(default = "default_skip_visible")]
    pub skip_visible: bool,
    pub activity_timeout: Option<ParsableDuration>,
    pub binding_prev: Option<KeyBinding>,
    pub binding_move_prev: Option<KeyBinding>,
    pub binding_next: Option<KeyBinding>,
    pub binding_move_next: Option<KeyBinding>,
    pub binding_swap_prev: Option<KeyBinding>,
    pub binding_swap_next: Option<KeyBinding>,
    pub binding_reset: Option<KeyBinding>,
    pub binding_to_head: Option<KeyBinding>,
    pub binding_move_to_head: Option<KeyBinding>,
    pub binding_rem_and_prev: Option<KeyBinding>,
    pub binding_rem_and_next: Option<KeyBinding>,
    pub binding_show_stack: Option<KeyBinding>,
}

impl Default for WSHistory {
    fn default() -> Self {
        Self {
            hist: (default_hist_type(), default_hist_sz()).into(),
            skip_visible: default_skip_visible(),
            ignore_ctr: 0,
            cur_output: "".to_string(),
            activity_timer: Instant::now(),
            activity_timeout: Some(Duration::from_secs(10).into()),
            binding_prev: Some(KeyBinding {
                event_state_mask: vec!["Mod4".to_string()].into_iter().collect(),
                symbol: Some("o".into()),
                input_type: I3Event::BindType::Keyboard,
            }),
            binding_move_prev: Some(KeyBinding {
                event_state_mask: vec!["Mod4".into(), "shift".into()].into_iter().collect(),
                symbol: Some("o".into()),
                input_type: I3Event::BindType::Keyboard,
            }),
            binding_next: Some(KeyBinding {
                event_state_mask: vec!["Mod4".to_string()].into_iter().collect(),
                symbol: Some("i".into()),
                input_type: I3Event::BindType::Keyboard,
            }),
            binding_move_next: Some(KeyBinding {
                event_state_mask: vec!["Mod4".into(), "shift".into()].into_iter().collect(),
                symbol: Some("i".into()),
                input_type: I3Event::BindType::Keyboard,
            }),
            binding_swap_prev: Some(KeyBinding {
                event_state_mask: vec!["Mod4".into(), "ctrl".into()].into_iter().collect(),
                symbol: Some("o".into()),
                input_type: I3Event::BindType::Keyboard,
            }),
            binding_swap_next: Some(KeyBinding {
                event_state_mask: vec!["Mod4".into(), "ctrl".into()].into_iter().collect(),
                symbol: Some("i".into()),
                input_type: I3Event::BindType::Keyboard,
            }),
            binding_reset: Some(KeyBinding {
                event_state_mask: vec!["Mod4".into(), "ctrl".into(), "shift".into()]
                    .into_iter()
                    .collect(),
                symbol: Some("o".into()),
                input_type: I3Event::BindType::Keyboard,
            }),
            binding_to_head: Some(KeyBinding {
                event_state_mask: vec!["Mod4".into(), "ctrl".into(), "shift".into()]
                    .into_iter()
                    .collect(),
                symbol: Some("i".into()),
                input_type: I3Event::BindType::Keyboard,
            }),
            binding_move_to_head: Some(KeyBinding {
                event_state_mask: vec!["Mod4".into(), "Mod1".into(), "shift".into()]
                    .into_iter()
                    .collect(),
                symbol: Some("i".into()),
                input_type: I3Event::BindType::Keyboard,
            }),
            binding_rem_and_prev: Some(KeyBinding {
                event_state_mask: vec!["Mod4".into(), "Mod1".into()].into_iter().collect(),
                symbol: Some("o".into()),
                input_type: I3Event::BindType::Keyboard,
            }),
            binding_rem_and_next: Some(KeyBinding {
                event_state_mask: vec!["Mod4".into(), "Mod1".into()].into_iter().collect(),
                symbol: Some("i".into()),
                input_type: I3Event::BindType::Keyboard,
            }),
            binding_show_stack: Some(KeyBinding {
                event_state_mask: vec!["Mod4".into(), "ctrl".into()].into_iter().collect(),
                symbol: Some("s".into()),
                input_type: I3Event::BindType::Keyboard,
            }),
        }
    }
}

impl From<WSHistoryConfig> for WSHistory {
    fn from(config: WSHistoryConfig) -> Self {
        Self {
            hist: (config.hist_type, config.hist_sz).into(),
            ignore_ctr: 0,
            skip_visible: config.skip_visible,
            activity_timer: Instant::now(),
            activity_timeout: config.activity_timeout.map(|d| d.into()),
            cur_output: "".to_string(),
            binding_prev: config.binding_prev,
            binding_move_prev: config.binding_move_prev,
            binding_next: config.binding_next,
            binding_move_next: config.binding_move_next,
            binding_swap_prev: config.binding_swap_prev,
            binding_swap_next: config.binding_swap_next,
            binding_reset: config.binding_reset,
            binding_to_head: config.binding_to_head,
            binding_move_to_head: config.binding_move_to_head,
            binding_rem_and_prev: config.binding_rem_and_prev,
            binding_rem_and_next: config.binding_rem_and_next,
            binding_show_stack: config.binding_show_stack,
        }
    }
}

impl WSHistory {
    /// Get the next or previous workspace from the history stack, relative to `cur_ws`
    /// Returns the index in the stack of that workspace if there is one available in that
    /// direction. Otherwise returns `None`.
    async fn get_ws(&self, cur_ws: usize, dir: WSDirection, i3: &mut I3) -> Option<usize> {
        let per_output = match self.hist.hist {
            HistType::PerOutput(_) => true,
            _ => false,
        };
        let hist = self.hist.get(&self.cur_output)?;
        let limit = hist.len() - 1;
        let check_range = |hist_ptr| match dir {
            WSDirection::PREV => hist_ptr < limit,
            WSDirection::NEXT => hist_ptr > 0,
        };
        if check_range(cur_ws) {
            if self.skip_visible || per_output {
                if let Ok(workspaces) = i3.get_workspaces().await {
                    let mut dest_ws = cur_ws + dir;
                    loop {
                        if matches!(workspaces.iter().find(|&w| w.num == hist[dest_ws]), Some(ws)
                            if (self.skip_visible && ws.visible) || (per_output && ws.output != self.cur_output))
                        {
                            dest_ws += dir;
                        } else {
                            return Some(dest_ws);
                        }
                        if !check_range(dest_ws) {
                            break;
                        }
                    }
                    None
                } else {
                    Some(cur_ws + dir)
                }
            } else {
                Some(cur_ws + dir)
            }
        } else {
            None
        }
    }

    /// Go to the next or previous workspace in the stack.
    /// Returns `None` if workspace didn't change, or `Some(new_ws)` if it did
    async fn goto_ws(&mut self, dir: WSDirection, i3: &mut I3) -> Option<i32> {
        self.check_timeout();
        let hist = self.hist.get(&self.cur_output)?;
        let new_ws = self.get_ws(hist.hist_ptr, dir, i3).await?;
        let hist = self.hist.get_mut(&self.cur_output)?;
        hist.hist_ptr = new_ws;
        Some(hist[hist.hist_ptr])
    }

    /// Jump to the top workspace in the stack.
    /// Returns `Some(new_ws)` if stack pointer changed, `None` otherwise.
    async fn goto_head(&mut self, i3: &mut I3) -> Option<i32> {
        self.check_timeout();
        let per_output = match self.hist.hist {
            HistType::PerOutput(_) => true,
            _ => false,
        };
        let hist = self.hist.get_mut(&self.cur_output)?;
        if hist.hist_ptr == 0 {
            return None;
        }
        let old_ptr = hist.hist_ptr;
        hist.hist_ptr = 0;
        if self.skip_visible || per_output {
            if let Ok(workspaces) = i3.get_workspaces().await {
                let limit = hist.len() - 1;
                let mut dest_ws = hist.hist_ptr;
                while dest_ws < limit {
                    if matches!(workspaces.iter().find(|&w| w.num == hist[dest_ws]), Some(ws)
                        if (self.skip_visible && ws.visible) || (per_output && ws.output != self.cur_output))
                    {
                        dest_ws += 1;
                    } else {
                        hist.hist_ptr = dest_ws;
                        break;
                    }
                }
            }
        }
        if hist.hist_ptr != old_ptr {
            Some(hist[hist.hist_ptr])
        } else {
            None
        }
    }

    /// Add `ws_num` to the history, resetting the history pointer
    fn add_ws(&mut self, ws_num: i32, output: &String) {
        let hist_sz = self.hist.hist_sz;
        let hist = self.hist.get_or_add_mut(output);
        // Add `ws_num` to history if it won't create a duplicate
        if hist.len() == 0 || hist[hist.hist_ptr] != ws_num {
            hist.reset_ptr();
            // Prevent duplicate sequences of 2
            if hist.len() > 2 && hist[0] == hist[2] && ws_num == hist[1] {
                hist.hist.pop_front();
            } else {
                // Add new ws, forgetting oldest if at max length
                hist.hist.truncate(hist_sz);
                hist.hist.push_front(ws_num);
            }
        }
    }

    /// Go to the next/previous workspace and remove the current one from the stack
    /// Returns the workspace number of the new workspace if it changed
    async fn rem_ws(&mut self, dir: WSDirection, i3: &mut I3) -> Option<i32> {
        self.check_timeout();
        let cur_ptr = {
            let hist = self.hist.get(&self.cur_output)?;
            hist.hist_ptr
        };
        if let Some(new_ws) = self.get_ws(cur_ptr, dir, i3).await {
            let hist = self.hist.get_mut(&self.cur_output)?;
            hist.hist.remove(cur_ptr);
            hist.hist_ptr = new_ws;
            if cur_ptr < hist.hist_ptr {
                hist.hist_ptr -= 1;
            }
            Some(hist[hist.hist_ptr])
        } else {
            None
        }
    }

    /// Check if workspace hasn't been changed since `activity_timer`,
    /// and reset the pointer if so
    /// Also resets the timer (all checks are triggered by user activity)
    /// Returns true if pointer was reset
    fn check_timeout(&mut self) -> bool {
        if let Some(timeout) = &self.activity_timeout {
            let triggered = Instant::now() > self.activity_timer;
            self.activity_timer = Instant::now() + *timeout;
            if triggered {
                match &mut self.hist.hist {
                    HistType::Single(hist) => hist.reset_ptr(),
                    HistType::PerOutput(hist) => {
                        for (_, h) in hist.iter_mut() {
                            h.reset_ptr();
                        }
                    }
                }
            }
            triggered
        } else {
            false
        }
    }

    /// Swap the position of the next/previous two workspaces in the stack.
    /// Aware of PerOutput and skip_visible settings
    async fn swap_ws(&mut self, dir: WSDirection, i3: &mut I3) {
        self.check_timeout();
        let hist_ptr = match self.hist.get(&self.cur_output) {
            Some(hist) => hist.hist_ptr,
            None => return,
        };
        if let Some(next_ws) = self.get_ws(hist_ptr, dir, i3).await {
            if let Some(next_ws2) = self.get_ws(next_ws, dir, i3).await {
                self.hist
                    .get_mut(&self.cur_output)
                    .unwrap()
                    .hist
                    .swap(next_ws, next_ws2);
            }
        }
    }
}

#[async_trait]
impl OnEvent for WSHistory {
    fn add_subscriptions(&self, subs: &mut HashSet<u32>) {
        subs.insert(Subscribe::Workspace as u32);
        subs.insert(Subscribe::Binding as u32);
    }

    async fn handle_event(&mut self, e: &Event, i3: &mut I3) -> Option<String> {
        match e {
            Event::Workspace(ws) => {
                self.check_timeout();
                if let Some(current) = &ws.current {
                    if let Some(output) = &current.output {
                        self.cur_output = output.clone();
                    }
                }
                if ws.change != WorkspaceChange::Init {
                    if self.ignore_ctr > 0 {
                        self.ignore_ctr -= 1;
                    } else if let (Some(old), Some(current)) = (&ws.old, &ws.current) {
                        if old.num != current.num {
                            if let (Some(old_num), Some(output)) = (old.num, &old.output) {
                                self.add_ws(old_num, output);
                            }
                            if let (Some(cur_num), Some(output)) = (current.num, &current.output) {
                                self.add_ws(cur_num, output);
                            }
                        }
                    }
                }
                None
            }
            Event::Binding(key) => {
                if self.hist.get(&self.cur_output).is_some()
                    && self.hist.get(&self.cur_output).unwrap().len() > 0
                {
                    if matches!(&self.binding_prev, Some(kb) if kb == key) {
                        self.goto_ws(WSDirection::PREV, i3)
                            .await
                            .and_then(|new_ws| {
                                self.ignore_ctr += 1;
                                Some(format!("workspace number {}", new_ws))
                            })
                    } else if matches!(&self.binding_move_prev, Some(kb) if kb == key) {
                        self.goto_ws(WSDirection::PREV, i3)
                            .await
                            .and_then(|new_ws| {
                                self.ignore_ctr += 2;
                                Some(format!(
                                    "move container to workspace number {0}; workspace number {0}",
                                    new_ws
                                ))
                            })
                    } else if matches!(&self.binding_next, Some(kb) if kb == key) {
                        self.goto_ws(WSDirection::NEXT, i3)
                            .await
                            .and_then(|new_ws| {
                                self.ignore_ctr += 1;
                                Some(format!("workspace number {}", new_ws))
                            })
                    } else if matches!(&self.binding_move_next, Some(kb) if kb == key) {
                        self.goto_ws(WSDirection::NEXT, i3)
                            .await
                            .and_then(|new_ws| {
                                self.ignore_ctr += 2;
                                Some(format!(
                                    "move container to workspace number {0}; workspace number {0}",
                                    new_ws
                                ))
                            })
                    } else if matches!(&self.binding_swap_prev, Some(kb) if kb == key) {
                        self.swap_ws(WSDirection::PREV, i3).await;
                        None
                    } else if matches!(&self.binding_swap_next, Some(kb) if kb == key) {
                        self.swap_ws(WSDirection::NEXT, i3).await;
                        None
                    } else if matches!(&self.binding_reset, Some(kb) if kb == key) {
                        // check timeout resets all history anyway, so no need to re-do if it's
                        // just been done
                        if !self.check_timeout() {
                            if let Some(hist) = self.hist.get_mut(&self.cur_output) {
                                hist.reset_ptr();
                            }
                        }
                        None
                    } else if matches!(&self.binding_to_head, Some(kb) if kb == key) {
                        self.goto_head(i3).await.and_then(|new_ws| {
                            self.ignore_ctr += 1;
                            Some(format!("workspace number {}", new_ws))
                        })
                    } else if matches!(&self.binding_move_to_head, Some(kb) if kb == key) {
                        self.goto_head(i3).await.and_then(|new_ws| {
                            self.ignore_ctr += 2;
                            Some(format!(
                                "move container to workspace number {0}; workspace number {0}",
                                new_ws
                            ))
                        })
                    } else if matches!(&self.binding_rem_and_prev, Some(kb) if kb == key) {
                        self.rem_ws(WSDirection::PREV, i3).await.and_then(|new_ws| {
                            self.ignore_ctr += 1;
                            Some(format!("workspace number {}", new_ws))
                        })
                    } else if matches!(&self.binding_rem_and_next, Some(kb) if kb == key) {
                        self.rem_ws(WSDirection::NEXT, i3).await.and_then(|new_ws| {
                            self.ignore_ctr += 1;
                            Some(format!("workspace number {}", new_ws))
                        })
                    } else if matches!(&self.binding_show_stack, Some(kb) if kb == key) {
                        self.check_timeout();
                        if let Ok(hist_msg) = self.hist.display(&self.cur_output) {
                            let header = match self.hist.hist {
                                HistType::PerOutput(_) => format!("i3 Workspace History ({})", self.cur_output),
                                HistType::Single(_) => "i3 Workspace History".into(),
                            };
                            match notify_rust::Notification::new()
                                .summary(header.as_str())
                                .body(hist_msg.as_str())
                                .appname("i3-companion")
                                .show()
                            {
                                Err(err) => {
                                    eprintln!("Error showing history stack: {}", err);
                                }
                                Ok(_) => (),
                            }
                        }
                        None
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum WSDirection {
    /// Newer workspaces (towards the top of the stack, `hist_ptr -= x`)
    NEXT,
    /// Older workspaces (towards the bottom of the stack, `hist_ptr += x`)
    PREV,
}
impl From<i32> for WSDirection {
    fn from(i: i32) -> Self {
        if i >= 0 {
            Self::PREV
        } else {
            Self::NEXT
        }
    }
}
impl From<WSDirection> for i32 {
    fn from(d: WSDirection) -> Self {
        match d {
            WSDirection::NEXT => -1,
            WSDirection::PREV => 1,
        }
    }
}
impl Add<WSDirection> for usize {
    type Output = usize;
    fn add(self, rhs: WSDirection) -> Self::Output {
        match rhs {
            WSDirection::NEXT => self - 1,
            WSDirection::PREV => self + 1,
        }
    }
}
impl AddAssign<WSDirection> for usize {
    fn add_assign(&mut self, rhs: WSDirection) {
        *self = *self + rhs;
    }
}
