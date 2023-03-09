#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use js_sys::Reflect;
#[cfg(target_arch = "wasm32")]
use web_sys::{IdbDatabase, IdbObjectStoreParameters};
#[cfg(target_arch = "wasm32")]
use polodb_core::IndexedDbContext;
use std::rc::Rc;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use polodb_core::{Database, bson};
use getrandom::getrandom;

static ID_CANDIDATES: &'static str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

#[wasm_bindgen(js_name = Database)]
pub struct DatabaseWrapper {
    db:        Rc<RefCell<Option<Database>>>,
    onsuccess: Option<js_sys::Function>,
    onerror:   Option<js_sys::Function>,
}

#[wasm_bindgen(js_class = Database)]
impl DatabaseWrapper {

    #[wasm_bindgen(constructor)]
    pub fn new() -> DatabaseWrapper {
        DatabaseWrapper {
            db: Rc::new(RefCell::new(None)),
            onsuccess: None,
            onerror: None,
        }
    }

    /// If a name is provided, the data will be synced to IndexedDB
    #[wasm_bindgen]
    pub fn open(&mut self, name: Option<String>) -> Result<(), JsError> {
        match name {
            Some(name) => {
                self.open_indexeddb(name.as_str())?;
            },
            None => {
                let db = Database::open_memory()?;
                let mut db_ref = self.db.as_ref().borrow_mut();
                *db_ref = Some(db);
            },
        };
        Ok(())
    }

    fn generate_session_id(len: usize) -> String {
        let mut buffer: Vec<u8> = vec![0; len];
        getrandom(&mut buffer).unwrap();

        let mut result = String::new();

        for byte in buffer {
            let index = (byte as usize) % ID_CANDIDATES.len();
            let char: u8 = ID_CANDIDATES.as_bytes()[index];
            result.push(char as char);
        }

        return result;
    }

    fn open_indexeddb(&mut self, name: &str) -> Result<(), JsError> {
        let window = web_sys::window().unwrap();
        let factory = window.indexed_db().unwrap().expect("indexeddb not supported");

        let open_request = factory.open_with_u32(name, 0).unwrap();

        {
            let db = self.db.clone();
            let name = name.to_string();
            let user_onsuccess = self.onsuccess().clone();
            let onsuccess = Closure::<dyn Fn(JsValue)>::new(move |event: JsValue| {
                let db = db.clone();
                let name = name.to_string();
                let user_onsuccess = user_onsuccess.clone();
                let target = Reflect::get(event.as_ref(), &"target".into()).unwrap();
                let idb = Reflect::get(target.as_ref(), &"result".into()).unwrap().dyn_into::<IdbDatabase>().unwrap();
                let session_id = DatabaseWrapper::generate_session_id(6);

                let loaded = {
                    let db2 = db.clone();
                    Rc::new(move || {
                        let user_onsuccess = user_onsuccess.clone();
                        let db_ref = db2.as_ref().borrow();
                        let _db = db_ref.as_ref().unwrap();

                        if let Some(user_onsuccess) = user_onsuccess {
                            user_onsuccess.call0(&JsValue::UNDEFINED).unwrap();
                        }
                    })
                };

                // val
                let raw_db = Database::open_indexeddb(IndexedDbContext {
                    name,
                    idb,
                    session_id,
                    loaded,
                }).unwrap();
                let mut db_ref = db.as_ref().borrow_mut();
                *db_ref = Some(raw_db);
            });
            open_request.set_onsuccess(Some(onsuccess.as_ref().unchecked_ref()));
            open_request.set_onerror(self.onerror.as_ref());

            let open_request_dup = open_request.clone();
            let on_onupgradeneeded = Closure::<dyn Fn()>::new(move || {
                let open_request_dup = open_request_dup.clone();
                let db = open_request_dup.result().unwrap().dyn_into::<IdbDatabase>().unwrap();
                let mut params = IdbObjectStoreParameters::new();
                params.auto_increment(true);
                db.create_object_store_with_optional_parameters(
                    "db_logs",
                    &params,
                ).unwrap();
            });
            open_request.set_onupgradeneeded(Some(on_onupgradeneeded.as_ref().unchecked_ref()));
        }

        Ok(())
    }

    #[wasm_bindgen(js_name = handleMessage)]
    pub fn handle_message(&self, buf: &[u8]) -> Result<Vec<u8>, JsError> {
        let mut db_ref = self.db.as_ref().borrow_mut();
        let db = db_ref.as_mut().unwrap();
        let bson = bson::from_slice(buf)?;
        let result = db.handle_request_doc(bson)?;
        let result_vec = bson::to_vec(&result.value)?;
        Ok(result_vec)
    }

    #[wasm_bindgen(getter)]
    pub fn onsuccess(&self) -> Option<js_sys::Function> {
        self.onsuccess.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_onsuccess(&mut self, fun: js_sys::Function) {
        self.onsuccess = Some(fun);
    }

    #[wasm_bindgen(getter)]
    pub fn onerror(&self) -> Option<js_sys::Function> {
        self.onerror.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_onerror(&mut self, fun: js_sys::Function) {
        self.onerror = Some(fun);
    }
}
