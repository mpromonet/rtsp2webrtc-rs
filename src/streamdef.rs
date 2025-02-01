/* ---------------------------------------------------------------------------
** This software is in the public domain, furnished "as is", without technical
** support, and with no warranty, express or implied, as to its usefulness for
** any purpose.
**
** SPDX-License-Identifier: Unlicense
**
** -------------------------------------------------------------------------*/

use tokio::sync::broadcast;
use std::sync::Arc;
use log::debug;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::api::media_engine::MIME_TYPE_H264;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::media::Sample;

#[derive(Clone)]
pub struct DataFrame {
    pub metadata: serde_json::Value,
    pub data: Vec<u8>,
}

pub struct StreamsDef {
    pub name: String,
    pub url: url::Url,
    pub tx: broadcast::Sender<DataFrame>,
    pub rx: broadcast::Receiver<DataFrame>,
    pub track: Arc<TrackLocalStaticSample>,
}

impl Clone for StreamsDef {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            url: self.url.clone(),
            tx: self.tx.clone(),
            rx: self.rx.resubscribe(),
            track: self.track.clone(),
        }
    }
}

impl StreamsDef {
    pub fn new(name: String, url: url::Url) -> Self {
        let (tx, rx) = broadcast::channel::<DataFrame>(1024*1024);

        let track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_H264.to_owned(),
                ..Default::default()
            },
            name.clone(),
            name.clone(),
        ));

        let instance = Self { name, url, tx,  rx, track };
        instance.start();
        instance
    }

    pub fn start(&self) {
        let self_clone = self.clone();
        tokio::spawn(async move {
            let mut rx = self_clone.rx.resubscribe();
            while let Ok(frame) = rx.recv().await {
                debug!("Received metadata:{:?} data:{}\n", frame.metadata, frame.data.len());
                let sample = Sample {
                    data: frame.data.into(),
                    ..Default::default()};
                self_clone.track.write_sample(&sample).await.unwrap();
            }    
        });
    }
}