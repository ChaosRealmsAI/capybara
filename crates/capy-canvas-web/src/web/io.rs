use wasm_bindgen_futures::spawn_local;

use super::{WebApp, downloads, idb_store, log};

impl WebApp {
    /// Pull `pending_save_request` / `pending_load_request` flags off the
    /// state and turn them into IndexedDB I/O. Same shape as native's
    /// `serial_fs::drain_pending_file_requests`, but the I/O is async so
    /// we `spawn_local` once the lock is released.
    pub(super) fn drain_pending_idb_requests(&mut self) {
        let (save_req, load_req, json_to_save) = match self.state.lock() {
            Ok(mut state) => {
                let save_req = state.pending_save_request;
                let load_req = state.pending_load_request;
                state.pending_save_request = false;
                state.pending_load_request = false;
                let json = if save_req {
                    match state.to_json_string() {
                        Ok(j) => Some(j),
                        Err(error) => {
                            log(&format!("[capy-canvas-web] to_json_string: {error}"));
                            None
                        }
                    }
                } else {
                    None
                };
                (save_req, load_req, json)
            }
            Err(_) => return,
        };

        if save_req {
            if let Some(json) = json_to_save {
                spawn_local(async move {
                    match idb_store::idb_save(json).await {
                        Ok(()) => log("[capy-canvas-web] saved to IndexedDB"),
                        Err(error) => {
                            log(&format!("[capy-canvas-web] idb_save: {error}"));
                        }
                    }
                });
            }
        }

        if load_req {
            spawn_local(async move {
                idb_store::perform_idb_load().await;
            });
        }
    }

    /// Drain `pending_svg_export` into a plain browser download.
    /// v0.24 keeps canvas export SVG/vector-first and intentionally avoids
    /// the old PNG readback path.
    pub(super) fn drain_pending_export_requests(&mut self) {
        let svg_to_export = match self.state.lock() {
            Ok(mut state) => {
                let svg = state.pending_svg_export.take();
                svg
            }
            Err(_) => return,
        };

        if let Some(svg) = svg_to_export {
            if let Err(error) =
                downloads::trigger_download(svg.as_bytes(), "image/svg+xml", "canvas.svg")
            {
                log(&format!("[capy-canvas-web] svg download: {error}"));
            } else {
                log("[capy-canvas-web] svg exported");
            }
        }
    }
}
