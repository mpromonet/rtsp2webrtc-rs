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

pub struct StreamsDef {
    pub name: String,
    pub tx: broadcast::Sender<Vec<u8>>,
    pub rx: broadcast::Receiver<Vec<u8>>,
    pub track: Arc<TrackLocalStaticSample>,
}

impl Clone for StreamsDef {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            tx: self.tx.clone(),
            rx: self.rx.resubscribe(),
            track: self.track.clone(),
        }
    }
}

impl StreamsDef {
    pub fn new(name: String) -> Self {
        let (tx, rx) = broadcast::channel::<Vec<u8>>(1024*1024);

        let track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_H264.to_owned(),
                ..Default::default()
            },
            name.clone(),
            name.clone(),
        ));

        Self { name, tx,  rx, track }
    }

    pub fn start(&self) {
        let self_clone = self.clone();
        tokio::spawn(async move {
            let mut rx = self_clone.rx.resubscribe();
            while let Ok(data) = rx.recv().await {
                debug!("Received data:{}\n", data.len());
                let sample = Sample {
                    data: data.into(),
                    ..Default::default()};
                self_clone.track.write_sample(&sample).await.unwrap();
            }    
        });
    }
}