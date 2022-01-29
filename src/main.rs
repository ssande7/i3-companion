use async_trait::async_trait;
use std::{
    collections::{vec_deque::VecDeque, HashSet},
    io,
    ops::{Add, AddAssign},
    time::Duration,
};
use tokio_i3ipc::{
    event as I3Event,
    event::{Event, Subscribe},
    msg::Msg,
    I3,
};
use tokio_stream::StreamExt;

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let mut config = Config::new();
    config.ws_history = Some(WSHistoryConfig::default()); // TODO: parse config
                                                          // config.layout_tracker = Some(LayoutTrackerConfig::default());
    listener(config).await
}

/// Continuously try to connect to i3 for the duration `time_limit`.
/// `interval` is the time to wait after a failed connection before retrying
/// Returns `Err(..)` if no successful connection after `time_limit`.
async fn try_i3_connection(
    time_limit: Duration,
    interval: Duration,
) -> Result<I3, tokio::time::error::Elapsed> {
    tokio::time::timeout(time_limit, async {
        loop {
            match I3::connect().await {
                Ok(i3) => {
                    return i3;
                }
                Err(_) => {
                    std::thread::sleep(interval);
                }
            }
        }
    })
    .await
}

/// Main listener loop
async fn listener(config: Config) -> io::Result<()> {
    // Set up event handlers
    let mut handlers = config.get_handlers();
    let mut subs = HashSet::new();
    for h in handlers.iter() {
        h.add_subscriptions(&mut subs);
    }
    let subs: Vec<Subscribe> = subs.iter().map(|&s| s.into()).collect();

    loop {
        let mut i3 =
            try_i3_connection(config.connection_timeout, config.connection_interval).await?;
        let resp = i3.subscribe(&subs).await?;
        println!("Response: {:#?}", resp);

        let mut i3_tx = I3::connect().await?;

        let mut listener = i3.listen();
        let mut restart = false;
        while let Some(event) = listener.next().await {
            let event = event?;
            if let Event::Shutdown(sd) = &event {
                if sd.change == I3Event::ShutdownChange::Restart {
                    restart = true;
                    println!("Restart detected");
                }
            }
            // TODO: parallelize
            for handler in handlers.iter_mut() {
                if let Some(msg) = handler.handle_event(&event, &mut i3_tx).await {
                    i3_tx.send_msg_body(Msg::RunCommand, msg).await?;
                }
            }
        }
        if !restart {
            break;
        }
    }
    Ok(())
}

struct Config {
    connection_timeout: Duration,  // secs
    connection_interval: Duration, // millis
    ws_history: Option<WSHistoryConfig>,
    layout_tracker: Option<LayoutTrackerConfig>,
}

impl Config {
    fn new() -> Self {
        // TODO: read from command line args or .config/i3-companion/config
        Self {
            connection_timeout: Duration::from_secs(3),
            connection_interval: Duration::from_millis(10),
            ws_history: None,
            layout_tracker: None,
        }
    }

    // Send trait not required right now, but keeping for future parallization
    fn get_handlers(&self) -> Vec<Box<dyn OnEvent + Send>> {
        let mut handlers = Vec::<Box<dyn OnEvent + Send>>::new();
        if let Some(config) = &self.ws_history {
            let wshist = Box::new(WSHistory::from(config));
            handlers.push(wshist);
        }
        if let Some(config) = &self.layout_tracker {
            handlers.push(Box::new(LayoutTracker::from(config)));
        }
        handlers
    }
}

#[async_trait]
trait OnEvent {
    // Need to use u32 since Subscribe doesn't impl Eq
    fn add_subscriptions(&self, subs: &mut HashSet<u32>);
    async fn handle_event(&mut self, e: &Event, i3: &mut I3) -> Option<String>;
}

trait Configurable {
    fn default() -> Self;
    fn from_config(config: &str) -> Self;
    fn from_cli() -> Self;
}

#[derive(Clone)]
struct KeyBinding {
    event_state_mask: HashSet<String>,
    symbol: Option<String>,
    input_type: I3Event::BindType,
}
impl PartialEq<I3Event::BindingData> for KeyBinding {
    fn eq(&self, other: &I3Event::BindingData) -> bool {
        let key = &other.binding;
        self.symbol == key.symbol
            && self.input_type == key.input_type
            && self.event_state_mask.len() == key.event_state_mask.len()
            && {
                for m in key.event_state_mask.iter() {
                    if !self.event_state_mask.contains(m) {
                        return false;
                    }
                }
                true
            }
    }
}

/// Better back and forth
struct WSHistory {
    ws_hist: VecDeque<i32>,
    hist_sz: usize,
    hist_ptr: usize,
    ignore_ctr: usize,
    skip_visible: bool,
    binding_prev: Option<KeyBinding>,
    binding_move_prev: Option<KeyBinding>,
    binding_next: Option<KeyBinding>,
    binding_move_next: Option<KeyBinding>,
}
struct WSHistoryConfig {
    hist_sz: usize,
    skip_visible: bool,
    binding_prev: Option<KeyBinding>,
    binding_move_prev: Option<KeyBinding>,
    binding_next: Option<KeyBinding>,
    binding_move_next: Option<KeyBinding>,
}

impl Configurable for WSHistoryConfig {
    fn default() -> Self {
        Self {
            hist_sz: 20,
            skip_visible: true,
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
            binding_prev: config.binding_prev.clone(),
            binding_move_prev: config.binding_move_prev.clone(),
            binding_next: config.binding_next.clone(),
            binding_move_next: config.binding_move_next.clone(),
        }
    }
}
#[derive(PartialEq, Eq, Clone, Copy)]
enum WSDirection {
    NEXT,
    PREV,
}
impl From<i32> for WSDirection {
    fn from(i: i32) -> Self {
        if i >= 0 {
            Self::NEXT
        } else {
            Self::PREV
        }
    }
}
impl From<WSDirection> for i32 {
    fn from(d: WSDirection) -> Self {
        match d {
            WSDirection::NEXT => 1,
            WSDirection::PREV => -1,
        }
    }
}
impl Add<WSDirection> for usize {
    type Output = usize;
    fn add(self, rhs: WSDirection) -> Self::Output {
        match rhs {
            WSDirection::NEXT => self + 1,
            WSDirection::PREV => self - 1,
        }
    }
}
impl AddAssign<WSDirection> for usize {
    fn add_assign(&mut self, rhs: WSDirection) {
        *self = *self + rhs;
    }
}
impl WSHistory {
    async fn get_ws(&mut self, dir: WSDirection, i3: &mut I3) -> bool {
        if (dir == WSDirection::NEXT && self.hist_ptr < self.ws_hist.len() - 1)
            || (dir == WSDirection::PREV && self.hist_ptr > 0)
        {
            self.hist_ptr += dir;
            if self.skip_visible {
                if let Ok(workspaces) = i3.get_workspaces().await {
                    let mut dest_ws = self.hist_ptr;
                    while dest_ws < self.ws_hist.len() - 1 {
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
                if self.ignore_ctr > 0 {
                    self.ignore_ctr -= 1;
                } else if let (Some(old), Some(current)) = (&ws.old, &ws.current) {
                    if let Some(old_num) = old.num {
                        if self.hist_ptr > 0 {
                            // let front: Vec<i32> = self.ws_hist.drain(..self.hist_ptr).collect();
                            for i in 0..=self.hist_ptr / 2 {
                                self.ws_hist.swap(i, self.hist_ptr - i);
                            }
                            self.hist_ptr = 0;
                        }
                        if !matches!(self.ws_hist.front(), Some(&hist_last) if hist_last == old_num)
                        {
                            if self.ws_hist.len() > 1 && self.ws_hist[1] == old_num {
                                self.ws_hist.swap(0, 1);
                            } else {
                                if self.ws_hist.len() == self.hist_sz {
                                    self.ws_hist.pop_back();
                                }
                                self.ws_hist.push_front(old_num);
                            }
                        }
                    }
                    if let Some(cur_num) = current.num {
                        if self.hist_ptr > 0 {
                            // self.ws_hist.drain(..self.hist_ptr);
                            for i in 0..=self.hist_ptr / 2 {
                                self.ws_hist.swap(i, self.hist_ptr - i);
                            }
                            self.hist_ptr = 0;
                        }
                        if !matches!(self.ws_hist.front(), Some(&hist_last) if hist_last == cur_num)
                        {
                            if self.ws_hist.len() > 1 && self.ws_hist[1] == cur_num {
                                self.ws_hist.swap(0, 1);
                            } else {
                                if self.ws_hist.len() == self.hist_sz {
                                    self.ws_hist.pop_back();
                                }
                                self.ws_hist.push_front(cur_num);
                            }
                        }
                    }
                }
                None
            }
            Event::Binding(key) => {
                let hist_len = self.ws_hist.len();
                if hist_len > 0 {
                    if matches!(&self.binding_prev, Some(kb) if kb == key) {
                        if self.get_ws(WSDirection::NEXT, i3).await {
                            self.ignore_ctr += 1;
                            Some(format!("workspace number {}", self.ws_hist[self.hist_ptr]))
                        } else {
                            None
                        }
                    } else if matches!(&self.binding_move_prev, Some(kb) if kb == key) {
                        if self.get_ws(WSDirection::NEXT, i3).await {
                            self.ignore_ctr += 2;
                            Some(format!(
                                "move container to workspace number {0}; workspace number {0}",
                                self.ws_hist[self.hist_ptr]
                            ))
                        } else {
                            None
                        }
                    } else if matches!(&self.binding_next, Some(kb) if kb == key) {
                        if self.get_ws(WSDirection::PREV, i3).await {
                            self.ignore_ctr += 1;
                            Some(format!("workspace number {}", self.ws_hist[self.hist_ptr]))
                        } else {
                            None
                        }
                    } else if matches!(&self.binding_move_next, Some(kb) if kb == key) {
                        if self.get_ws(WSDirection::PREV, i3).await {
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

/// Layout indicator
struct LayoutTracker {}
struct LayoutTrackerConfig {}

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
