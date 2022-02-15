use serde::{
    de::Visitor,
    Deserialize,
};
use std::collections::HashSet;
use tokio_i3ipc::event as I3Event;

#[derive(Clone)]
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

struct KeyBindingVisitor;
impl<'de> Visitor<'de> for KeyBindingVisitor {
    type Value = KeyBinding;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a keybinding in the i3-style format (eg. Mod4+o)")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let mut symbol = None;
        let mut event_state_mask = HashSet::<String>::new();
        for key in v.split("+") {
            match key {
                "Mod1" | "Mod2" | "Mod3" | "Mod4" | "ctrl" | "shift" => {
                    event_state_mask.insert(key.into());
                }
                "Ctrl" | "Shift" => {
                    event_state_mask.insert(key.to_lowercase());
                }
                _ => {
                    if symbol.is_none() {
                        symbol = Some(key.to_lowercase());
                    } else {
                        return Err(E::custom(format!(
                            "Keybinding {} has unexpected extra symbol: {}",
                            v, key
                        )));
                    }
                }
            }
        }
        Ok(KeyBinding {
            event_state_mask,
            symbol,
            input_type: I3Event::BindType::Keyboard,
        })
    }
}

impl<'de> Deserialize<'de> for KeyBinding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(KeyBindingVisitor)
    }
}
