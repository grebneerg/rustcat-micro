use crate::model::{Alert, BusStop, RouteInfo, RtfEntity};
use crate::proto::gtfs_realtime;
use crate::proto::gtfs_realtime::TripUpdate_StopTimeUpdate_ScheduleRelationship;
use actix_web::http::header::{AUTHORIZATION, CACHE_CONTROL};
use chrono::{DateTime, FixedOffset, Utc};
use protobuf::{CodedInputStream, Message};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::ops::Add;
use std::sync::{Arc, Mutex};
use lazy_static::lazy_static;

lazy_static! {
    static ref CLIENT: reqwest::Client = reqwest::Client::new();
}

const ALERTS_URL: &str =
    "https://gateway.api.cloud.wso2.com:443/t/mystop/tcat/v1/rest/PublicMessages/GetAllMessages";

const RTF_URL: &str =
    "https://realtimetcatbus.availtec.com/InfoPoint/GTFS-Realtime.ashx?&Type=TripUpdate";

const STOPS_URL: &str =
    "https://gateway.api.cloud.wso2.com:443/t/mystop/tcat/v1/rest/Stops/GetAllStops";

const TOKEN_URL: &str = "https://gateway.api.cloud.wso2.com:443/token";

pub async fn alerts(auth: Arc<Mutex<AuthInfo>>) -> Result<Vec<Alert>, Box<dyn Error>> {
    let bearer = update_auth(auth).await?;

    Ok(CLIENT.get(ALERTS_URL)
        .header(CACHE_CONTROL, "no-cache")
        .bearer_auth(bearer)
        .send()
        .await?
        .json()
        .await?)
}

pub async fn gtfs() -> Result<Vec<RouteInfo>, ()> {
    actix_web::web::block(|| {
        let mut rdr = csv::Reader::from_path("tcat-ny-us/routes.txt").map_err(|_| ())?;
        Ok(rdr
            .deserialize()
            .filter_map(|r: Result<RouteInfo, csv::Error>| r.ok())
            .collect::<Vec<_>>())
    })
    .await.map_err(|_| ())?
}

pub async fn rtf() -> Result<HashMap<String, RtfEntity>, Box<dyn Error>> {
    let data = CLIENT.get(RTF_URL).send().await?.bytes().await?;
    let mut readable_data = data.as_ref();
    let mut cis = CodedInputStream::new(&mut readable_data);
    let mut msg = gtfs_realtime::FeedMessage::parse_from(&mut cis)?;
    let mut map = HashMap::new();
    for mut entity in msg.take_entity() {
        if let Some(mut tu) = entity.trip_update.take() {
            map.insert(
                entity.take_id(),
                RtfEntity {
                    route_id: tu.trip.unwrap().take_route_id(),
                    stop_updates: tu
                        .stop_time_update
                        .into_iter()
                        .filter(|stu| {
                            stu.get_schedule_relationship()
                                != TripUpdate_StopTimeUpdate_ScheduleRelationship::NO_DATA
                        })
                        .map(|mut stu| (stu.take_stop_id(), stu.arrival.unwrap().get_delay()))
                        .collect(),
                    vehicle_id: tu.vehicle.take().map(|mut v| v.take_id()),
                },
            );
        }
    }

    Ok(map)
}

pub async fn stops(auth: Arc<Mutex<AuthInfo>>) -> Result<Vec<BusStop>, Box<dyn Error>> {
    let bearer = update_auth(auth).await?;
    // todo filter
    Ok(serde_json::from_slice(
        &CLIENT
            .get(STOPS_URL)
            .bearer_auth(bearer)
            .send()
            .await?
            .bytes()
            .await?[..],
    )?)
}

#[derive(Clone)]
pub struct AuthInfo {
    header: String,
    expires: DateTime<Utc>,
    updated: DateTime<Utc>,
}

impl AuthInfo {
    pub fn expired(&self) -> bool {
        self.expires < Utc::now()
    }

    pub async fn fetch() -> Result<Self, Box<dyn Error>> {
        let info = CLIENT
            .post(TOKEN_URL)
            .header(CACHE_CONTROL, "no-cache")
            .header(
                AUTHORIZATION,
                format!(
                    "Basic {}",
                    std::env::var("TOKEN").expect("No TOKEN env var")
                ),
            )
            .form(&AUTH_REQUEST)
            .send()
            .await?
            .json::<AuthResponse>()
            .await?;
        let now = Utc::now();

        Ok(Self {
            header: info.access_token,
            expires: now.add(FixedOffset::east(info.expires_in)),
            updated: now,
        })
    }
}

async fn update_auth(auth: Arc<Mutex<AuthInfo>>) -> Result<String, Box<dyn Error>> {
    let auth_info = {
        let lock = auth.lock().unwrap();
        lock.clone()
    };

    Ok(if auth_info.expired() {
        let new_info = AuthInfo::fetch().await?;

        let mut ci = auth.lock().unwrap();
        if new_info.updated > ci.updated {
            ci.updated = new_info.updated;
            ci.expires = new_info.expires;
            ci.header = new_info.header;
        }

        ci.header.clone()
    } else {
        auth_info.header
    })
}

#[derive(Deserialize)]
struct AuthResponse {
    access_token: String,
    expires_in: i32,
}

#[derive(Serialize)]
struct AuthRequest {
    grant_type: &'static str,
}

const AUTH_REQUEST: AuthRequest = AuthRequest {
    grant_type: "client_credentials",
};
