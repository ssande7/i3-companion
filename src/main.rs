use std::{io, time::Duration, collections::HashSet};
use tokio_stream::StreamExt;
use tokio_i3ipc::{I3, event as I3Event, event::{Event, Subscribe}, msg::Msg};

#[tokio::main(flavor="current_thread")]
async fn main() -> io::Result<()> {
    let mut config = Config::new();
    config.ws_back_and_forth = Some(WSBackAndForthConfig::default()); // TODO: parse config
    listener(config).await
}

/// Continuously try to connect to i3 for the duration `time_limit`.
/// `interval` is the time to wait after a failed connection before retrying
/// Returns `Err(..)` if no successful connection after `time_limit`.
async fn try_i3_connection(time_limit: Duration, interval: Duration) ->  Result<I3,tokio::time::error::Elapsed> {
    tokio::time::timeout(time_limit, async {
        loop {
            match I3::connect().await {
                Ok(i3) => {return i3;},
                Err(_) => {std::thread::sleep(interval);}
            }
        }
    }).await
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
        let mut i3 = try_i3_connection(
            config.connection_timeout,
            config.connection_interval
            ).await?;
        let resp = i3.subscribe(&subs).await?;
        println!("Response: {:#?}", resp);

        let mut i3_tx = I3::connect().await?;

        let mut listener = i3.listen();
        let mut restart = false;
        while let Some(event) = listener.next().await {
            let event = event?;
            if let Event::Shutdown(sd) = &event {
                if sd.change == I3Event::ShutdownChange::Restart {
                    restart=true;
                    println!("Restart detected");
                }
            }
            // TODO: parallelize
            for handler in handlers.iter_mut() {
                if let Some(msg) = handler.handle_event(&event) {
                    i3_tx.send_msg_body(Msg::RunCommand, msg).await?;
                }
            }
        }
        if !restart {break;}
    }
    Ok(())
}

struct Config {
    connection_timeout: Duration,    // secs
    connection_interval: Duration,   // millis
    ws_back_and_forth: Option<WSBackAndForthConfig>,
}

impl Config {
    fn new() -> Self {
        // TODO: read from command line args or .config/i3-companion/config
        Self {
            connection_timeout: Duration::from_secs(3),
            connection_interval: Duration::from_millis(10),
            ws_back_and_forth: None,
        }
    }

    // Send trait not required right now, but keeping for future parallization
    fn get_handlers(&self) -> Vec<Box<dyn OnEvent + Send>> {
        let mut handlers = Vec::<Box<dyn OnEvent + Send>>::new();
        if let Some(config) = &self.ws_back_and_forth {
            handlers.push(Box::new(WSBackAndForth::from(config)))
        }
        handlers
    }
}

trait OnEvent {
    // Need to use u32 since Subscribe doesn't impl Eq
    fn add_subscriptions(&self, subs: &mut HashSet<u32>);
    fn handle_event(&mut self, e: &Event) -> Option<String>;
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
        self.symbol == key.symbol &&
            self.input_type == key.input_type && 
            self.event_state_mask.len() == key.event_state_mask.len() &&
            {
                for m in key.event_state_mask.iter() {
                    if !self.event_state_mask.contains(m) {return false;}
                }
                true
            }
    }
}

struct WSBackAndForth {
    ws_hist: Vec<i32>,
    binding_switch: Option<KeyBinding>,
    binding_move: Option<KeyBinding>,
}
struct WSBackAndForthConfig {
    hist_sz: usize,
    binding_switch: Option<KeyBinding>,
    binding_move: Option<KeyBinding>,
}

impl Configurable for WSBackAndForthConfig {
    fn default() -> Self {
        Self {
            hist_sz: 20,
            binding_switch: Some(KeyBinding{
                event_state_mask: vec!["Mod4".to_string()].into_iter().collect(),
                symbol: Some("o".into()),
                input_type: I3Event::BindType::Keyboard,
            }),
            binding_move: Some(KeyBinding{
                event_state_mask: vec!["Mod4".into(), "shift".into()].into_iter().collect(),
                symbol: Some("o".into()),
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


impl OnEvent for WSBackAndForth {
    fn add_subscriptions(&self, subs: &mut HashSet<u32>) {
        subs.insert(Subscribe::Workspace as u32);
        subs.insert(Subscribe::Binding as u32);
    }

    fn handle_event(&mut self, e: &Event) -> Option<String> {
        match e {
            Event::Workspace(ws) => {
                if let (Some(old), Some(current)) = (&ws.old, &ws.current) {
                    if old.output == current.output {
                        if let (Some(old_num), Some(current_num)) = (old.num, current.num) {
                            if let Some(idx) = self.ws_hist.iter().position(|&i| i == old_num) {
                                self.ws_hist.remove(idx);
                            }
                            if let Some(idx) = self.ws_hist.iter().position(|&i| i == current_num) {
                                self.ws_hist.remove(idx);
                            }
                            if old.nodes.len() > 0 {
                                self.ws_hist.push(old_num);
                            }
                        }
                    }
                }
                None
            },
            Event::Binding(key) => {
                if matches!(&self.binding_switch, Some(kb) if kb == key) {
                    if let Some(&prev) = self.ws_hist.last() {
                        Some(format!("workspace number {}", prev))
                    } else {None}
                } else if matches!(&self.binding_move, Some(kb) if kb == key) {
                    if let Some(&prev) = self.ws_hist.last() {
                        Some(format!("move container to workspace number {0}; workspace number {0}", prev))
                    } else {None}
                } else {
                    None
                }
            },
            _ => None
        }
    }
}

impl From<&WSBackAndForthConfig> for WSBackAndForth {
    fn from(config: &WSBackAndForthConfig) -> Self {
        Self {
            ws_hist: Vec::with_capacity(config.hist_sz),
            binding_switch: config.binding_switch.clone(),
            binding_move: config.binding_move.clone(),
        }
    }
}
