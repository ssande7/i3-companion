use regex::Regex;
use serde::{
    de::{Error, Visitor},
    Deserialize,
};
use std::time::Duration;

pub struct ParsableDuration(Duration);
impl From<Duration> for ParsableDuration {
    fn from(d: Duration) -> Self {
        Self { 0: d }
    }
}
impl From<ParsableDuration> for Duration {
    fn from(d: ParsableDuration) -> Self {
        d.0
    }
}
struct ParsableDurationVisitor;
impl<'de> Visitor<'de> for ParsableDurationVisitor {
    type Value = ParsableDuration;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a duration with units. eg. 10s, 1.5ms, 0.2 m")
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let caps = Regex::new("^([0-9]+\\.?[0-9]*)\\s*([mun]?s|[mh])$")
            .unwrap()
            .captures(v)
            .ok_or(Error::custom(format!(
                "String '{}' cannot be parsed to a duration",
                v
            )))?;
        if caps.len() == 3 {
            let dur: f32 =
                caps.get(1)
                    .unwrap()
                    .as_str()
                    .parse()
                    .or(Err(Error::custom(format!(
                        "Duration couldn't be converted to a number from '{}'",
                        v
                    ))))?;
            match caps.get(2).unwrap().as_str() {
                "ns" => Ok(ParsableDuration {
                    0: Duration::from_secs_f32(dur * 1000.0 * 1000.0 * 1000.0),
                }),
                "us" => Ok(ParsableDuration {
                    0: Duration::from_secs_f32(dur * 1000.0 * 1000.0),
                }),
                "ms" => Ok(ParsableDuration {
                    0: Duration::from_secs_f32(dur * 1000.0),
                }),
                "s" => Ok(ParsableDuration {
                    0: Duration::from_secs_f32(dur),
                }),
                "m" => Ok(ParsableDuration {
                    0: Duration::from_secs_f32(dur / 60.0),
                }),
                "h" => Ok(ParsableDuration {
                    0: Duration::from_secs_f32(dur / 3600.0),
                }),
                u => Err(Error::custom(format!("Unrecognised time units: {}", u))),
            }
        } else {
            Err(Error::custom(format!("Invalid duration string: {}", v)))
        }
    }
}
impl<'de> Deserialize<'de> for ParsableDuration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(ParsableDurationVisitor)
    }
}
