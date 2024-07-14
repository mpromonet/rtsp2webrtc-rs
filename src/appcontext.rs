/* ---------------------------------------------------------------------------
** This software is in the public domain, furnished "as is", without technical
** support, and with no warranty, express or implied, as to its usefulness for
** any purpose.
**
** SPDX-License-Identifier: Unlicense
**
** -------------------------------------------------------------------------*/

use std::sync::Arc;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::api::API;

pub struct AppContext {
    pub api: Arc<API>,
    pub track: Arc<TrackLocalStaticSample>,
}

impl AppContext {
    pub fn new(api: Arc<API>, track: Arc<TrackLocalStaticSample>) -> Self {
        Self {
            api: api.clone(),
            track: track.clone(),
        }
    }
}