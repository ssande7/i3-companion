use super::{
    keybinding::KeyBinding,
    traits::{Configurable, OnEvent},
};
use async_trait::async_trait;
use std::{
    collections::{vec_deque::VecDeque, HashSet},
    ops::{Add, AddAssign},
    time::{Duration, Instant},
};
use tokio_i3ipc::{
    event as I3Event,
    event::{Event, Subscribe},
    I3,
};

pub struct WSHistory {
    ws_hist: VecDeque<i32>,
    hist_sz: usize,
    hist_ptr: usize,
    ignore_ctr: usize,
    activity_timer: Instant,
    activity_timeout: Duration,
    pub skip_visible: bool,
    pub binding_prev: Option<KeyBinding>,
    pub binding_move_prev: Option<KeyBinding>,
    pub binding_next: Option<KeyBinding>,
    pub binding_move_next: Option<KeyBinding>,
}
pub struct WSHistoryConfig {
    pub hist_sz: usize,
    pub skip_visible: bool,
    pub activity_timeout: Duration,
    pub binding_prev: Option<KeyBinding>,
    pub binding_move_prev: Option<KeyBinding>,
    pub binding_next: Option<KeyBinding>,
    pub binding_move_next: Option<KeyBinding>,
}

impl Configurable for WSHistoryConfig {
    fn default() -> Self {
        Self {
            hist_sz: 20,
            skip_visible: true,
            activity_timeout: Duration::from_secs(10),
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
        }
    }
    fn from_config(_config: &str) -> Self {
        unimplemented!()
    }
    fn from_cli() -> Self {
        unimplemented!()
    }
}

impl From<&WSHistoryConfig> for WSHistory {
    fn from(config: &WSHistoryConfig) -> Self {
        Self {
            ws_hist: VecDeque::with_capacity(config.hist_sz),
            hist_sz: config.hist_sz,
            hist_ptr: 0,
            ignore_ctr: 0,
            skip_visible: config.skip_visible,
            activity_timer: Instant::now(),
            activity_timeout: config.activity_timeout,
            binding_prev: config.binding_prev.clone(),
            binding_move_prev: config.binding_move_prev.clone(),
            binding_next: config.binding_next.clone(),
            binding_move_next: config.binding_move_next.clone(),
        }
    }
}

impl WSHistory {
    /// Get the next or previous workspace from the history
    async fn get_ws(&mut self, dir: WSDirection, i3: &mut I3) -> bool {
        self.check_timeout();
        let check_range = |hist_ptr| match dir {
            WSDirection::PREV => hist_ptr < self.ws_hist.len() - 1,
            WSDirection::NEXT => hist_ptr > 0,
        };
        if check_range(self.hist_ptr) {
            self.hist_ptr += dir;
            if self.skip_visible {
                if let Ok(workspaces) = i3.get_workspaces().await {
                    let mut dest_ws = self.hist_ptr;
                    while check_range(dest_ws) {
                        if matches!(workspaces.iter().find(|&w| w.num == self.ws_hist[dest_ws]), Some(ws) if ws.visible)
                        {
                            dest_ws += dir;
                        } else {
                            self.hist_ptr = dest_ws;
                            break;
                        }
                    }
                }
            }
            true
        } else {
            false
        }
    }

    /// Add `ws_num` to the history, resetting the history pointer
    fn add_ws(&mut self, ws_num: i32) {
        self.reset_ptr();
        // Add `ws_num` to history if it won't create a duplicate
        if !matches!(self.ws_hist.front(), Some(&hist_last) if hist_last == ws_num) {
            // Prevent duplicate sequences of 2
            if self.ws_hist.len() > 2
                && self.ws_hist[0] == self.ws_hist[2]
                && ws_num == self.ws_hist[1]
            {
                self.ws_hist.pop_front();
            } else {
                // Add new ws, forgetting oldest if at max length
                self.ws_hist.truncate(self.hist_sz);
                self.ws_hist.push_front(ws_num);
            }
        }
    }

    /// Reset the history pointer, reversing the order of history before it
    /// NOTE: may change `ws_hist.len()`
    fn reset_ptr(&mut self) {
        if self.hist_ptr > 0 {
            // Reverse order of history that has been cycled back through,
            // preventing double ups
            if self.hist_ptr < self.ws_hist.len() - 1
                && self.ws_hist[self.hist_ptr + 1] == self.ws_hist[0]
            {
                self.ws_hist.pop_front();
            }
            for i in 0..=self.hist_ptr / 2 {
                self.ws_hist.swap(i, self.hist_ptr - i);
            }
            self.hist_ptr = 0;
        }
    }

    /// Reset the activity timeout
    fn reset_timer(&mut self) {
        self.activity_timer = Instant::now() + self.activity_timeout;
    }

    /// Check if workspace hasn't been changed since `activity_timer`,
    /// and reset the pointer if so
    fn check_timeout(&mut self) {
        if self.activity_timeout > Duration::from_secs(0)
            && Instant::now() > self.activity_timer
        {
            self.reset_ptr();
            self.reset_timer();
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
                self.reset_timer();
                if self.ignore_ctr > 0 {
                    self.ignore_ctr -= 1;
                } else if let (Some(old), Some(current)) = (&ws.old, &ws.current) {
                    if let Some(old_num) = old.num {
                        self.add_ws(old_num);
                    }
                    if let Some(cur_num) = current.num {
                        self.add_ws(cur_num);
                    }
                }
                None
            }
            Event::Binding(key) => {
                if self.ws_hist.len() > 0 {
                    if matches!(&self.binding_prev, Some(kb) if kb == key) {
                        if self.get_ws(WSDirection::PREV, i3).await {
                            self.ignore_ctr += 1;
                            Some(format!("workspace number {}", self.ws_hist[self.hist_ptr]))
                        } else {
                            None
                        }
                    } else if matches!(&self.binding_move_prev, Some(kb) if kb == key) {
                        if self.get_ws(WSDirection::PREV, i3).await {
                            self.ignore_ctr += 2;
                            Some(format!(
                                "move container to workspace number {0}; workspace number {0}",
                                self.ws_hist[self.hist_ptr]
                            ))
                        } else {
                            None
                        }
                    } else if matches!(&self.binding_next, Some(kb) if kb == key) {
                        if self.get_ws(WSDirection::NEXT, i3).await {
                            self.ignore_ctr += 1;
                            Some(format!("workspace number {}", self.ws_hist[self.hist_ptr]))
                        } else {
                            None
                        }
                    } else if matches!(&self.binding_move_next, Some(kb) if kb == key) {
                        if self.get_ws(WSDirection::NEXT, i3).await {
                            self.ignore_ctr += 2;
                            Some(format!(
                                "move container to workspace number {0}; workspace number {0}",
                                self.ws_hist[self.hist_ptr]
                            ))
                        } else {
                            None
                        }
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
    NEXT,
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
