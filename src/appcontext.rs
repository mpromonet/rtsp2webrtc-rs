/* ---------------------------------------------------------------------------
** This software is in the public domain, furnished "as is", without technical
** support, and with no warranty, express or implied, as to its usefulness for
** any purpose.
**
** SPDX-License-Identifier: Unlicense
**
** -------------------------------------------------------------------------*/

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use webrtc::api::API;
use webrtc::peer_connection::RTCPeerConnection;
use crate::streamdef::StreamsDef;

pub struct AppContext {
    pub api: Arc<API>,
    pub streams: HashMap<String,Arc<Mutex<StreamsDef>>>,
    pub stunurl: String,
    pub connections: Arc<Mutex<HashMap<String, Arc<RTCPeerConnection>>>>,
}

impl AppContext {
    pub fn new(api: Arc<API>, streams: HashMap<String,Arc<Mutex<StreamsDef>>>, stunurl: String) -> Self {
        Self {
            api: api.clone(),
            streams: streams.clone(),
            stunurl: stunurl.clone(),
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Clone for AppContext {
    fn clone(&self) -> Self {
        Self {
            api: self.api.clone(),
            streams: self.streams.clone(),
            stunurl: self.stunurl.clone(),
            connections: self.connections.clone(),
        }
    }
}