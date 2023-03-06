/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct IndexedDbStoreFrame {
    pub pages: Vec<Vec<u8>>,
    #[serde(rename = "pageIds")]
    pub page_ids: Vec<u32>,
}

impl Default for IndexedDbStoreFrame {
    fn default() -> Self {
        IndexedDbStoreFrame {
            pages: Vec::new(),
            page_ids: Vec::new(),
        }
    }
}
