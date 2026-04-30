use wasm_bindgen::prelude::*;
use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};

/// Build a `Blob` from `bytes` (typed `mime`), create an `<a>` with
/// `href = blob URL · download = filename`, click it, revoke URL.
/// Standard browser download dance.
pub(super) fn trigger_download(bytes: &[u8], mime: &str, filename: &str) -> Result<(), String> {
    // Wrap the bytes in a JS Uint8Array; Blob takes a sequence of typed arrays.
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&array.buffer());

    let opts = BlobPropertyBag::new();
    opts.set_type(mime);

    let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &opts)
        .map_err(|e| format!("Blob::new: {e:?}"))?;

    let url =
        Url::create_object_url_with_blob(&blob).map_err(|e| format!("create_object_url: {e:?}"))?;

    let win = web_sys::window().ok_or_else(|| "no window".to_string())?;
    let document = win.document().ok_or_else(|| "no document".to_string())?;
    let anchor: HtmlAnchorElement = document
        .create_element("a")
        .map_err(|e| format!("create_element a: {e:?}"))?
        .dyn_into::<HtmlAnchorElement>()
        .map_err(|_| "dyn_into HtmlAnchorElement".to_string())?;
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();

    Url::revoke_object_url(&url).map_err(|e| format!("revoke_object_url: {e:?}"))?;
    Ok(())
}
