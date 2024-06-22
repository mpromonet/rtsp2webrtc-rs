/* ---------------------------------------------------------------------------
** This software is in the public domain, furnished "as is", without technical
** support, and with no warranty, express or implied, as to its usefulness for
** any purpose.
**
** SPDX-License-Identifier: Unlicense
**
** -------------------------------------------------------------------------*/

use retina::client::{SessionGroup, SetupOptions, Transport};
use retina::codec::{CodecItem, VideoFrame};
use anyhow::{anyhow, Error};
use log::{debug, error, info};
use std::sync::Arc;
use tokio::sync::broadcast;
use futures::StreamExt;

pub async fn run(url: url::Url, transport: Option<String>, tx: broadcast::Sender<Vec<u8>>) -> Result<(), Error> {
    let session_group = Arc::new(SessionGroup::default());
    let r = run_inner(url, transport, session_group.clone(), tx).await;
    if let Err(e) = session_group.await_teardown().await {
        error!("TEARDOWN failed: {}", e);
    }
    r
}

fn process_video_frame(m: VideoFrame, tx: broadcast::Sender<Vec<u8>>) {
    debug!(
        "{}: size:{} is_random_access_point:{} has_new_parameters:{}",
        m.timestamp().timestamp(),
        m.data().len(),
        m.is_random_access_point(),
        m.has_new_parameters(),
    );

    if let Err(e) = tx.send(m.data().to_vec()) {
        error!("Error broadcasting message: {}", e);
    }                        
}

async fn run_inner(url: url::Url, transport: Option<String>, session_group: Arc<SessionGroup>, tx: broadcast::Sender<Vec<u8>>) -> Result<(), Error> {
    let stop = tokio::signal::ctrl_c();

    let mut session = retina::client::Session::describe(
        url.clone(),
        retina::client::SessionOptions::default()
            .session_group(session_group),
    )
    .await?;
    debug!("{:?}", session.streams());

    let video_stream = session
        .streams()
        .iter()
        .position(|s| s.media() == "video" && s.encoding_name() == "h264")
        .ok_or_else(|| anyhow!("couldn't find video stream"))?;

    let transport_value = match transport {
        Some(t) => t.parse::<Transport>().unwrap(),
        None => Transport::default(), 
    };    
    let options = SetupOptions::transport(SetupOptions::default(), transport_value);
    session
        .setup(video_stream, options)
        .await?;

    let video_params = match session.streams()[video_stream].parameters() {
        Some(retina::codec::ParametersRef::Video(v)) => v.clone(),
        Some(_) => unreachable!(),
        None => unreachable!(),
    };
    info!("video_params:{:?}", video_params);

    let mut videosession = session
        .play(retina::client::PlayOptions::default())
        .await?
        .demuxed()?;

    
    tokio::pin!(stop);
    loop {
        tokio::select! {
            item = videosession.next() => {
                match item.ok_or_else(|| anyhow!("EOF"))?? {
                    CodecItem::VideoFrame(m) => process_video_frame(m, tx.clone()),
                    _ => continue,
                };
            },
            _ = &mut stop => {
                break;
            },
        }
    }
    Ok(())
}
