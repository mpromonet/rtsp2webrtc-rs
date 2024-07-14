/* ---------------------------------------------------------------------------
** This software is in the public domain, furnished "as is", without technical
** support, and with no warranty, express or implied, as to its usefulness for
** any purpose.
**
** SPDX-License-Identifier: Unlicense
**
** -------------------------------------------------------------------------*/

use actix_files::Files;
use actix_web::{get, post, web, App, HttpServer, HttpResponse};
use clap::Parser;
use log::{debug, info};
use std::sync::Arc;
use serde_json::json;
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

#[derive(Parser)]
struct Opts {
    #[arg(long)]
    url: url::Url,

    #[clap(short)]
    transport: Option<String>,    
}



#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let opts = Opts::parse();

    let stream = streamdef::StreamsDef::new("video".to_owned());

    // start the RTSP clients
    tokio::spawn(rtspclient::run(opts.url.clone(), opts.transport.clone(), stream.tx.clone()));

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

    //create track
    stream.start();
    let mut streams = std::collections::HashMap::new();
    streams.insert(stream.name.clone(), Arc::new(stream.clone()));


    info!("start actix web server");
    HttpServer::new( move || {
        App::new().app_data(web::Data::new(AppContext::new(api.clone(), streams.clone())))
            .service(version)
            .service(whep)
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

#[post("/api/whep")]
async fn whep(bytes: web::Bytes, data: web::Data<AppContext>) -> HttpResponse {

    let ctx = data.get_ref();
    let downstream_cfg = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ..Default::default()
    };

    let downstream_conn = Arc::new(ctx.api.new_peer_connection(downstream_cfg).await.unwrap());

    let stream = Arc::clone(&ctx.streams.get("video").unwrap());
    let sender = downstream_conn
        .add_track(stream.track.clone() as Arc<dyn TrackLocal + Send + Sync>)
        .await.unwrap();
    
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

    HttpResponse::Ok()
        .content_type("application/sdp")
        .append_header(("Location", "/api/whep?peerid"))
        .append_header(("Access-Control-Expose-Headers","Location"))
        .body(answer.sdp.clone())
}
