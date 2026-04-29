use wasm_bindgen::prelude::*;

use idb::{DatabaseEvent, Factory, KeyPath, ObjectStoreParams, TransactionMode};

use super::{log, redraw_via_shared, shared_state};

const DB_NAME: &str = "capy-canvas";
const STORE_NAME: &str = "snapshots";
const SNAPSHOT_KEY: &str = "main";
const DB_VERSION: u32 = 1;

/// Open or upgrade the `capy-canvas` database. Idempotent: if the
/// `snapshots` store already exists the upgrade callback never fires.
async fn open_db() -> Result<idb::Database, String> {
    let factory = Factory::new().map_err(|e| format!("Factory::new: {e}"))?;
    let mut open_request = factory
        .open(DB_NAME, Some(DB_VERSION))
        .map_err(|e| format!("factory.open: {e}"))?;
    open_request.on_upgrade_needed(|event| {
        let database = match event.database() {
            Ok(d) => d,
            Err(error) => {
                log(&format!("[capy-canvas-web] upgrade.database: {error}"));
                return;
            }
        };
        let names = database.store_names();
        if names.iter().any(|n| n == STORE_NAME) {
            return;
        }
        let mut params = ObjectStoreParams::new();
        params.auto_increment(false);
        params.key_path(None::<KeyPath>);
        if let Err(error) = database.create_object_store(STORE_NAME, params) {
            log(&format!("[capy-canvas-web] create_object_store: {error}"));
        }
    });
    let db = open_request
        .await
        .map_err(|e| format!("open_request.await: {e}"))?;
    Ok(db)
}

/// Persist a single snapshot under key `"main"`.
pub(super) async fn idb_save(json: String) -> Result<(), String> {
    let db = open_db().await?;
    let tx = db
        .transaction(&[STORE_NAME], TransactionMode::ReadWrite)
        .map_err(|e| format!("transaction: {e}"))?;
    let store = tx
        .object_store(STORE_NAME)
        .map_err(|e| format!("object_store: {e}"))?;
    let value = JsValue::from_str(&json);
    let key = JsValue::from_str(SNAPSHOT_KEY);
    store
        .put(&value, Some(&key))
        .map_err(|e| format!("put: {e}"))?
        .await
        .map_err(|e| format!("put.await: {e}"))?;
    tx.commit()
        .map_err(|e| format!("commit: {e}"))?
        .await
        .map_err(|e| format!("commit.await: {e}"))?;
    db.close();
    Ok(())
}

/// Read back the snapshot under key `"main"`. `Ok(None)` means the store
/// is empty (first run, or save hasn't happened yet).
pub(super) async fn idb_load() -> Result<Option<String>, String> {
    let db = open_db().await?;
    let tx = db
        .transaction(&[STORE_NAME], TransactionMode::ReadOnly)
        .map_err(|e| format!("transaction: {e}"))?;
    let store = tx
        .object_store(STORE_NAME)
        .map_err(|e| format!("object_store: {e}"))?;
    let key = JsValue::from_str(SNAPSHOT_KEY);
    let result: Option<JsValue> = store
        .get(key)
        .map_err(|e| format!("get: {e}"))?
        .await
        .map_err(|e| format!("get.await: {e}"))?;
    db.close();
    Ok(result.and_then(|v| v.as_string()))
}

/// Shared logic for "load JSON from IDB and stuff it into AppState".
/// Used by both the keyboard drain path and the JS-callable `load()`.
pub(super) async fn perform_idb_load() {
    match idb_load().await {
        Ok(Some(json)) => {
            let state_arc = match shared_state() {
                Some(s) => s,
                None => {
                    log("[capy-canvas-web] perform_idb_load: no shared state");
                    return;
                }
            };
            let load_result = match state_arc.lock() {
                Ok(mut state) => state.load_from_json_str(&json),
                Err(_) => return,
            };
            match load_result {
                Ok(()) => {
                    log("[capy-canvas-web] loaded from IndexedDB");
                    redraw_via_shared();
                }
                Err(error) => {
                    log(&format!("[capy-canvas-web] load_from_json_str: {error}"));
                }
            }
        }
        Ok(None) => log("[capy-canvas-web] idb_load: no snapshot yet"),
        Err(error) => log(&format!("[capy-canvas-web] idb_load: {error}")),
    }
}
