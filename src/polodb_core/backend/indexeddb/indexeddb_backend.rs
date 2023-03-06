/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use std::cell::RefCell;
use std::num::{NonZeroU32, NonZeroU64};
use std::sync::Arc;
use bson::oid::ObjectId;
use crate::backend::Backend;
use crate::backend::memory::MemoryBackendInner;
use crate::{DbResult, TransactionType};
use crate::page::RawPage;
use crate::IndexedDbContext;

#[allow(dead_code)]
pub(crate) struct IndexedDbBackend {
    inner: RefCell<IndexedDbBackendInner>,
}

unsafe impl Send for IndexedDbBackend {}

impl IndexedDbBackend {

    pub fn open(ctx: IndexedDbContext, page_size: NonZeroU32, init_block_count: NonZeroU64) -> IndexedDbBackend {
        let inner = IndexedDbBackendInner::open(ctx, page_size, init_block_count);
        IndexedDbBackend {
            inner: RefCell::new(inner),
        }
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
    #[allow(dead_code)]
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

    fn read_page(&mut self, page_id: u32, session_id: Option<&ObjectId>) -> DbResult<Arc<RawPage>> {
        self.mem.read_page(page_id, session_id)
    }

    fn write_page(&mut self, page: &RawPage, session_id: Option<&ObjectId>) -> DbResult<()> {
        self.mem.write_page(page, session_id)
    }

    fn commit(&mut self) -> DbResult<()> {
        self.mem.commit()?;
        Ok(())
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
