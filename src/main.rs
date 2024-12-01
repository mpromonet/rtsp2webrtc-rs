/* ---------------------------------------------------------------------------
** This software is in the public domain, furnished "as is", without technical
** support, and with no warranty, express or implied, as to its usefulness for
** any purpose.
**
** SPDX-License-Identifier: Unlicense
**
** -------------------------------------------------------------------------*/

use actix_files::Files;
use actix_web::{get, post, delete, web, App, HttpServer, HttpResponse};
use anyhow::Error;
use clap::Parser;
use log::{debug, info};
use serde_json::json;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use webrtc::{
    api::{interceptor_registry::register_default_interceptors, APIBuilder},
    ice_transport::{ice_connection_state::RTCIceConnectionState, ice_server::RTCIceServer},
    interceptor::registry::Registry,
    peer_connection::{
        configuration::RTCConfiguration, peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription,
    },
    track::track_local::TrackLocal,
};

mod rtspclient;
mod appcontext;
mod streamdef;

use crate::appcontext::AppContext;
use streamdef::StreamsDef;

#[derive(Parser)]
struct Opts {
    #[clap(short)]
    config: String,    

    #[clap(short)]
    transport: Option<String>,   

    #[arg(long, default_value = "stun:stun.l.google.com:19302")]
    stunurl: String,     
}


fn read_json_file(file_path: &str) -> Result<serde_json::Value, Error> {
    let mut file = File::open(file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let data = serde_json::from_str(&contents)?;
    Ok(data)
}

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let opts = Opts::parse();

    let mut streams_defs = HashMap::new();
    match read_json_file(opts.config.as_str()) {
        Ok(data) => {
            let urls = data["urls"].as_object().unwrap();
            for (key, value) in urls.into_iter() {
                let url = url::Url::parse(value["video"].as_str().unwrap()).unwrap().clone();
                streams_defs.insert(key.to_owned(), Arc::new(Mutex::new(StreamsDef::new(key.to_owned(), url.clone()))));
            }
        },
        Err(err) => println!("Error reading JSON file: {:?}", err),
    }


    // webrtc
    let mut m = webrtc::api::media_engine::MediaEngine::default();
    m.register_default_codecs().unwrap();
    let mut registry = Registry::new();
    registry = register_default_interceptors(registry, &mut m).unwrap();

    // Create the API object with the MediaEngine
    let api = Arc::new(APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .build());

    // start the RTSP clients
    let app_context = appcontext::AppContext::new(api.clone(), streams_defs, opts.stunurl.clone());
    app_context.streams.values().for_each(|streamdef| {
        let stream = streamdef.lock().unwrap();
        tokio::spawn(rtspclient::run(stream.url.clone(), opts.transport.clone(), stream.tx.clone()));
    });


    info!("start actix web server");
    HttpServer::new( move || {
        App::new().app_data(web::Data::new(app_context.clone()))
            .service(version)
            .service(streams)
            .service(logger_level)            
            .service(peers)
            .service(whep_post)
            .service(whep_delete)
            .service(web::redirect("/", "/index.html"))
            .service(Files::new("/", "./www"))
    })
    .bind(("0.0.0.0", 8080)).unwrap()
    .run()
    .await
    .unwrap();


    info!("Done");
}

#[get("/api/version")]
async fn version() -> HttpResponse {
    let version = env!("GIT_VERSION");
    let data = json!(version);

    HttpResponse::Ok().json(data)
}

#[get("/api/streams")]
async fn streams(data: web::Data<appcontext::AppContext>) -> HttpResponse {
    let app_context = data.get_ref();
    let mut data = json!({});
    for (key, streamdef) in &app_context.streams {
        data[key] = json!({
            "name": streamdef.lock().unwrap().name,
            "url": streamdef.lock().unwrap().url.as_str(),
        });
    }

    HttpResponse::Ok().json(data)
}

#[get("/api/peers")]
async fn peers(data: web::Data<appcontext::AppContext>) -> HttpResponse {
    let ctx = data.get_ref();
    let mut data = json!({});
    let connections = ctx.connections.lock().unwrap();
    for (key, conn) in connections.iter() {
        data[key] = json!({
            "state": conn.ice_connection_state().to_string(),
        });
    }

    HttpResponse::Ok().json(data)
}

#[get("/api/log")]
async fn logger_level(query: web::Query<HashMap<String, String>>) -> HttpResponse {
    
    if let Some(level_str) = query.get("level") {
        match level_str.as_str() {
            "Off" => log::set_max_level(log::LevelFilter::Off),
            "Error" => log::set_max_level(log::LevelFilter::Error),
            "Warn" => log::set_max_level(log::LevelFilter::Warn),
            "Info" => log::set_max_level(log::LevelFilter::Info),
            "Debug" => log::set_max_level(log::LevelFilter::Debug),
            "Trace" => log::set_max_level(log::LevelFilter::Trace),
            _ => (),
        }
    }

    let level = log::max_level(); 

    let level_str = match level {
        log::LevelFilter::Off => "Off",
        log::LevelFilter::Error => "Error",
        log::LevelFilter::Warn => "Warn",
        log::LevelFilter::Info => "Info",
        log::LevelFilter::Debug => "Debug",
        log::LevelFilter::Trace => "Trace",
    };

    let data = json!({
        "level": level_str,
    });

    HttpResponse::Ok().json(data)
}

#[derive(Deserialize)]
struct WhepPostQuery {
    stream_name: String,
}

#[post("/api/whep")]
async fn whep_post(query: web::Query<WhepPostQuery>, bytes: web::Bytes, data: web::Data<AppContext>) -> HttpResponse {

    let ctx = data.get_ref();
    let downstream_cfg = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec![ctx.stunurl.clone()],
            ..Default::default()
        }],
        ..Default::default()
    };

    let downstream_conn = Arc::new(ctx.api.new_peer_connection(downstream_cfg).await.unwrap());

    let stream_name = &query.stream_name;
    let stream_wrap =  Arc::clone(&ctx.streams.get(stream_name).unwrap());
    let stream = stream_wrap.lock().unwrap();
    let sender = downstream_conn
        .add_track(stream.track.clone() as Arc<dyn TrackLocal + Send + Sync>)
        .await.unwrap();
    
    let downstream_conn_clone = Arc::clone(&downstream_conn);
    tokio::spawn(async move {
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = sender.read(&mut rtcp_buf).await {}
    });

    let sdp = String::from_utf8(bytes.to_vec()).unwrap();
    let offer = RTCSessionDescription::offer(sdp).unwrap();
    downstream_conn.set_remote_description(offer).await.unwrap();
    let answer = downstream_conn.create_answer(None).await.unwrap();
    downstream_conn.set_local_description(answer.clone()).await.unwrap();
    downstream_conn.gathering_complete_promise().await.recv().await;

    tokio::spawn(async move {
        let (ice_conn_state_tx, ice_conn_state_rx) = tokio::sync::mpsc::unbounded_channel();
        downstream_conn.on_ice_connection_state_change(Box::new(
            move |state: RTCIceConnectionState| {
                ice_conn_state_tx.send(state).unwrap();
                Box::pin(async {})
            },
        ));
        tokio::pin!(ice_conn_state_rx);
        let (peer_conn_state_tx, peer_conn_state_rx) = tokio::sync::mpsc::unbounded_channel();
        downstream_conn.on_peer_connection_state_change(Box::new(
            move |state: RTCPeerConnectionState| {
                peer_conn_state_tx.send(state).unwrap();
                Box::pin(async {})
            },
        ));
        tokio::pin!(peer_conn_state_rx);
            loop {
            tokio::select! {
                state = ice_conn_state_rx.recv() => {
                    debug!("ICE connection state changed: {:?}", state);
                }
                state = peer_conn_state_rx.recv() => {
                    debug!("Peer connection state changed: {:?}", state);
                }
            }
        }
    });

    // Generate a unique peerid
    let peer_id = Uuid::new_v4().to_string();

    // Store the connection
    let mut connections = ctx.connections.lock().unwrap();
    connections.insert(peer_id.clone(), downstream_conn_clone);
    info!("registered peerid: {}", peer_id);

    HttpResponse::Ok()
        .content_type("application/sdp")
        .append_header(("Location", "/api/whep?peerid=".to_owned() + &peer_id))
        .append_header(("Access-Control-Expose-Headers","Location"))
        .body(answer.sdp.clone())
}

#[derive(Deserialize)]
struct WhepDeleteQuery {
    peer_id: String,
}

#[delete("/api/whep")]
async fn whep_delete(query: web::Query<WhepDeleteQuery>, data: web::Data<AppContext>) -> HttpResponse {
    let ctx = data.get_ref();
    let peer_id = &query.peer_id;

    info!("unregistered peerid: {}", peer_id);

    let mut connections = ctx.connections.lock().unwrap();

    if let Some(conn) = connections.remove(peer_id) {
        conn.close().await.unwrap();
        HttpResponse::Ok().json("Connection removed")
    } else {
        HttpResponse::NotFound().json("Connection not found")
    }
}