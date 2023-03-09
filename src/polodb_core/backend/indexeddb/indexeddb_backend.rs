/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use std::cell::RefCell;
use std::num::{NonZeroU32, NonZeroU64};
use std::sync::Arc;
use std::io::Write;
use std::rc::Rc;
use bson::oid::ObjectId;
use web_sys::{IdbTransactionMode, IdbCursor};
use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use lz4_flex::frame::FrameEncoder;
use crate::backend::Backend;
use crate::backend::memory::{MemoryBackendInner, Transaction};
use crate::{DbResult, TransactionType};
use crate::page::RawPage;
use crate::IndexedDbContext;
use super::store_data::IndexedDbStoreFrame;

#[allow(dead_code)]
pub(crate) struct IndexedDbBackend {
    inner: RefCell<IndexedDbBackendInner>,
}

unsafe impl Send for IndexedDbBackend {}

impl IndexedDbBackend {

    pub fn open(ctx: IndexedDbContext, page_size: NonZeroU32, init_block_count: NonZeroU64) -> IndexedDbBackend {
        let inner = IndexedDbBackendInner::open(ctx, page_size, init_block_count);
        let result = IndexedDbBackend {
            inner: RefCell::new(inner),
        };

        result.load_data();

        result
    }

    /// Load data from idb
    fn load_data(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.load_data();
    }

}

impl Backend for IndexedDbBackend {
    fn read_page(&self, page_id: u32, session_id: Option<&ObjectId>) -> DbResult<Arc<RawPage>> {
        let mut inner = self.inner.borrow_mut();
        inner.read_page(page_id, session_id)
    }

    fn write_page(&self, page: &RawPage, session_id: Option<&ObjectId>) -> DbResult<()> {
        let mut inner = self.inner.borrow_mut();
        inner.write_page(page, session_id)
    }

    fn commit(&self) -> DbResult<()> {
        let mut inner = self.inner.borrow_mut();
        inner.commit()
    }

    fn db_size(&self) -> u64 {
        let inner = self.inner.borrow_mut();
        inner.db_size()
    }

    fn set_db_size(&self, size: u64) -> DbResult<()> {
        let mut inner = self.inner.borrow_mut();
        inner.set_db_size(size)
    }

    fn transaction_type(&self) -> Option<TransactionType> {
        let inner = self.inner.borrow_mut();
        inner.transaction_type()
    }

    fn upgrade_read_transaction_to_write(&self) -> DbResult<()> {
        let mut inner = self.inner.borrow_mut();
        inner.upgrade_read_transaction_to_write()
    }

    fn rollback(&self) -> DbResult<()> {
        let mut inner = self.inner.borrow_mut();
        inner.rollback()
    }

    fn start_transaction(&self, ty: TransactionType) -> DbResult<()> {
        let mut inner = self.inner.borrow_mut();
        inner.start_transaction(ty)
    }

    fn new_session(&self, id: &ObjectId) -> DbResult<()> {
        let mut inner = self.inner.borrow_mut();
        inner.new_session(id)
    }

    fn remove_session(&self, id: &ObjectId) -> DbResult<()> {
        let mut inner = self.inner.borrow_mut();
        inner.remove_session(id)
    }
}

pub struct IndexedDbBackendInner {
    ctx: IndexedDbContext,
    mem: MemoryBackendInner,
}

impl IndexedDbBackendInner {

    pub fn open(ctx: IndexedDbContext, page_size: NonZeroU32, init_block_count: NonZeroU64) -> IndexedDbBackendInner {
        IndexedDbBackendInner {
            ctx,
            mem: MemoryBackendInner::new(page_size, init_block_count),
        }
    }

    fn load_data(&mut self) {
        let transaction = self.ctx.idb.transaction_with_str("db_logs").unwrap();
        let obj_store = transaction.object_store("db_logs").unwrap();
        let cursor = obj_store.open_cursor().unwrap();

        let frames: Rc<RefCell<Vec<IndexedDbStoreFrame>>> = Rc::new(RefCell::new(Vec::new()));
        let loaded = self.ctx.loaded.clone();
        let onsuccess = Closure::<dyn Fn(JsValue)>::new(move |event: JsValue| {
            let loaded = loaded.clone();
            let frames = frames.clone();
            let target = Reflect::get(event.as_ref(), &"target".into()).unwrap();
            let cursor_js = Reflect::get(target.as_ref(), &"result".into()).unwrap();

            if cursor_js.is_truthy() {
                let cursor = cursor_js.dyn_into::<IdbCursor>().unwrap();
                let value_js = Reflect::get(cursor.as_ref(), &"value".into()).unwrap();
                let frame: IndexedDbStoreFrame = serde_wasm_bindgen::from_value(value_js).unwrap();

                {
                    let mut frames_vec_ref = frames.as_ref().borrow_mut();
                    frames_vec_ref.push(frame);
                }

                cursor.continue_().unwrap();
            } else {
                loaded.as_ref()();
            }
        });

        cursor.set_onsuccess(Some(onsuccess.as_ref().unchecked_ref()));
    }

    fn read_page(&mut self, page_id: u32, session_id: Option<&ObjectId>) -> DbResult<Arc<RawPage>> {
        self.mem.read_page(page_id, session_id)
    }

    fn write_page(&mut self, page: &RawPage, session_id: Option<&ObjectId>) -> DbResult<()> {
        self.mem.write_page(page, session_id)
    }

    fn commit(&mut self) -> DbResult<()> {
        let transaction = self.mem.commit()?;
        self.write_transaction_to_indexeddb(&transaction)?;
        Ok(())
    }

    fn write_transaction_to_indexeddb(&mut self, transaction: &Transaction) -> DbResult<()> {
        let idb_transaction = self.ctx.idb.transaction_with_str_and_mode(
            "db_logs",
            IdbTransactionMode::Readwrite,
        ).unwrap();

        let obj_store = idb_transaction.object_store("db_logs").unwrap();
        let frame = self.transaction_to_store_frame(transaction);

        let frame_js = serde_wasm_bindgen::to_value(&frame).unwrap();
        obj_store.add(&frame_js).unwrap();

        idb_transaction.commit().unwrap();

        Ok(())
    }

    fn transaction_to_store_frame(&self, transaction: &Transaction) -> IndexedDbStoreFrame {
        let cap_len = transaction.dirty_pages.len();
        let mut pages = Vec::<Vec<u8>>::with_capacity(cap_len);
        let mut page_ids = Vec::<u32>::with_capacity(cap_len);

        for (page_id, page) in &transaction.dirty_pages {
            let out_data = IndexedDbBackendInner::fast_compress(page.as_ref());
            pages.push(out_data);
            page_ids.push(*page_id);
        }

        IndexedDbStoreFrame {
            pages,
            page_ids,
            sid: self.ctx.session_id.clone(),
        }
    }

    fn fast_compress(page: &RawPage) -> Vec<u8> {
        let mut out_data = Vec::<u8>::new();
        let mut encoder = FrameEncoder::new(&mut out_data);
        encoder.write_all(&page.data).unwrap();

        out_data
    }

    fn db_size(&self) -> u64 {
        self.mem.db_size()
    }

    fn set_db_size(&mut self, size: u64) -> DbResult<()> {
        self.mem.set_db_size(size)
    }

    fn transaction_type(&self) -> Option<TransactionType> {
        self.mem.transaction_type()
    }

    fn upgrade_read_transaction_to_write(&mut self) -> DbResult<()> {
        self.mem.upgrade_read_transaction_to_write()
    }

    fn rollback(&mut self) -> DbResult<()> {
        self.mem.rollback()
    }

    fn start_transaction(&mut self, ty: TransactionType) -> DbResult<()> {
        self.mem.start_transaction(ty)
    }

    fn new_session(&mut self, id: &ObjectId) -> DbResult<()> {
        self.mem.new_session(id)
    }

    fn remove_session(&mut self, id: &ObjectId) -> DbResult<()> {
        self.mem.remove_session(id)
    }
}
