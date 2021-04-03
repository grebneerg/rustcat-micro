use crate::fetch::AuthInfo;
use actix_web::{get, middleware, web, App, HttpResponse, HttpServer, Responder};
use std::sync::{Arc, Mutex, RwLock};
use actix_web::rt::{Arbiter, time::interval};
use std::time::Duration;
use futures::{select, FutureExt, pin_mut};
use actix_web::middleware::TrailingSlash;

mod fetch;
mod model;
mod proto;

struct State {
    alerts: RwLock<String>,
    gtfs: RwLock<String>,
    rtf: RwLock<String>,
    stops: RwLock<String>,
}

#[get("/alerts")]
async fn get_alerts(state: web::Data<State>) -> impl Responder {
    HttpResponse::Ok()
        .content_type("application/json")
        .body(&*state.alerts.read().unwrap())
}

#[get("/gtfs")]
async fn get_gtfs(state: web::Data<State>) -> impl Responder {
    HttpResponse::Ok()
        .content_type("application/json")
        .body(&*state.gtfs.read().unwrap())
}

#[get("/rtf")]
async fn get_rtf(state: web::Data<State>) -> impl Responder {
    HttpResponse::Ok()
        .content_type("application/json")
        .body(&*state.rtf.read().unwrap())
}

#[get("/stops")]
async fn get_stops(state: web::Data<State>) -> impl Responder {
    HttpResponse::Ok()
        .content_type("application/json")
        .body(&*state.stops.read().unwrap())
}

async fn update(data: web::Data<State>, auth: Arc<Mutex<AuthInfo>>) {

    let alerts_fut = fetch::alerts(auth.clone()).fuse();
    let gtfs_fut = fetch::gtfs().fuse();
    let rtf_fut = fetch::rtf().fuse();
    let stops_fut = fetch::stops(auth.clone()).fuse();

    pin_mut!(alerts_fut, gtfs_fut, rtf_fut, stops_fut);

    loop {
        select! {
            alerts = alerts_fut => *data.alerts.write().unwrap() = serde_json::to_string(&alerts.unwrap()).unwrap(),
            gtfs = gtfs_fut => *data.gtfs.write().unwrap() = serde_json::to_string(&gtfs.unwrap()).unwrap(),
            rtf = rtf_fut => *data.rtf.write().unwrap() = serde_json::to_string(&rtf.unwrap()).unwrap(),
            stops = stops_fut => *data.stops.write().unwrap() = serde_json::to_string(&stops.unwrap()).unwrap(),
            complete => break
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let auth = Arc::new(Mutex::new(AuthInfo::fetch().await.unwrap()));

    let data = web::Data::new(State {
        alerts: RwLock::new(
            serde_json::to_string(&fetch::alerts(auth.clone()).await.unwrap()).unwrap(),
        ),
        gtfs: RwLock::new(serde_json::to_string(&fetch::gtfs().await.unwrap()).unwrap()),
        rtf: RwLock::new(serde_json::to_string(&fetch::rtf().await.unwrap()).unwrap()),
        stops: RwLock::new(
            serde_json::to_string(&fetch::stops(auth.clone()).await.unwrap()).unwrap(),
        ),
    });

    let data2 = data.clone();

    let updater = Arbiter::new();
    updater.spawn(Box::pin(async move {

        let mut delay = interval(Duration::new(60, 0));

        loop {
            delay.tick().await;
            update(data2.clone(), auth.clone()).await;
        }
    }));

    HttpServer::new(move || {
        App::new()
            .app_data(data.clone())
            .wrap(middleware::NormalizePath::new(TrailingSlash::Trim))
            .service(get_alerts)
            .service(get_gtfs)
            .service(get_rtf)
            .service(get_stops)
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
