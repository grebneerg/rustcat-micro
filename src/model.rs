use chrono::{DateTime, TimeZone, Utc};
use serde::de;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ChannelMessage {
    channel_id: isize,
    message: String,
}

const FORMAT: &str = "/Date(%s";

pub fn dow_deserialize<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s = isize::deserialize(deserializer)?;
    match s {
        127 => Ok("Every Day".to_string()),
        65 => Ok("Weekeends".to_string()),
        62 => Ok("Weekdays".to_string()),
        2 => Ok("Monday".to_string()),
        4 => Ok("Tuesday".to_string()),
        8 => Ok("Wednesday".to_string()),
        16 => Ok("Thursday".to_string()),
        32 => Ok("Friday".to_string()),
        64 => Ok("Saturday".to_string()),
        1 => Ok("Sunday".to_string()),
        _ => Err(de::Error::custom("Invalid day number")),
    }
}

pub fn datetime_deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut s = String::deserialize(deserializer)?;
    let i = s.find('-').unwrap();
    s.replace_range(i - 3..s.len(), "");
    Utc.datetime_from_str(&s, FORMAT)
        .map_err(serde::de::Error::custom)
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "PascalCase"))]
pub struct Alert {
    channel_messages: Vec<ChannelMessage>,
    #[serde(deserialize_with = "dow_deserialize")]
    days_of_week: String,
    #[serde(deserialize_with = "datetime_deserialize")]
    from_date: DateTime<Utc>,
    #[serde(deserialize_with = "datetime_deserialize")]
    from_time: DateTime<Utc>,
    #[serde(rename(deserialize = "MessageId"))]
    id: isize,
    message: String,
    priority: isize,
    routes: Vec<isize>,
    signs: Vec<isize>,
    #[serde(deserialize_with = "datetime_deserialize")]
    to_date: DateTime<Utc>,
    #[serde(deserialize_with = "datetime_deserialize")]
    to_time: DateTime<Utc>,
}

#[derive(Deserialize, Serialize)]
pub struct RouteInfo {
    agency_id: String,
    route_id: String,
    route_long_name: String,
    route_short_name: String,
    route_type: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtfEntity {
    pub route_id: String,
    pub stop_updates: HashMap<String, i32>,
    pub vehicle_id: Option<String>,
}

const BUS_STOP_TYPE: &str = "busStop";

fn bus_stop_type() -> &'static str {
    BUS_STOP_TYPE
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "PascalCase"))]
pub struct BusStop {
    name: String,
    #[serde(rename(deserialize = "Latitude"))]
    lat: f64,
    #[serde(rename(deserialize = "Longitude"))]
    long: f64,
    #[serde(default = "bus_stop_type", rename = "type", skip_deserializing)]
    stop_type: &'static str,
}
