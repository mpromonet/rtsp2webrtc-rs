/* ---------------------------------------------------------------------------
** This software is in the public domain, furnished "as is", without technical
** support, and with no warranty, express or implied, as to its usefulness for
** any purpose.
**
** SPDX-License-Identifier: Unlicense
**
** -------------------------------------------------------------------------*/

use std::collections::HashMap;
use std::sync::Arc;
use webrtc::api::API;
use crate::streamdef::StreamsDef;

pub struct AppContext {
    pub api: Arc<API>,
    pub streams: HashMap<String,Arc<StreamsDef>>,
    pub stunurl: String,
}

impl AppContext {
    pub fn new(api: Arc<API>, streams: HashMap<String,Arc<StreamsDef>>, stunurl: String) -> Self {
        Self {
            api: api.clone(),
            streams: streams.clone(),
            stunurl: stunurl.clone(),
        }
    }
}