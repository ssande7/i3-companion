use serde::Deserialize;
use std::collections::HashSet;
use tokio_i3ipc::event as I3Event;

#[derive(Clone, Deserialize)]
pub struct KeyBinding {
    pub event_state_mask: HashSet<String>,
    pub symbol: Option<String>,
    pub input_type: I3Event::BindType,
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
