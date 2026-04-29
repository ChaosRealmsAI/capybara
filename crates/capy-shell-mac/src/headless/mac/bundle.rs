use std::path::Path;

use objc2::rc::Retained;
use objc2::{msg_send, AnyThread, MainThreadMarker};
use objc2_foundation::{NSString, NSURL};

use super::{MacHeadlessShell, DEFAULT_TIMEOUT};
use crate::ShellError;

impl MacHeadlessShell {
    pub(super) fn load_bundle_path(&self, path: &Path) -> Result<(), ShellError> {
        {
            let rx = self
                .events_rx
                .lock()
                .map_err(|e| ShellError::BundleLoadFailed(format!("events rx poisoned: {e}")))?;
            while rx.try_recv().is_ok() {}
        }

        let is_data_url = path.to_string_lossy().starts_with("data:");
        let url_str = if is_data_url {
            path.to_string_lossy().into_owned()
        } else {
            let abs = if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir()
                    .map_err(|e| ShellError::BundleLoadFailed(format!("cwd: {e}")))?
                    .join(path)
            };
            format!("file://{}", abs.display())
        };

        let ns_url_str = NSString::from_str(&url_str);
        let _mtm = MainThreadMarker::new().ok_or(ShellError::UnsupportedPlatform)?;
        let url = NSURL::URLWithString(&ns_url_str)
            .ok_or_else(|| ShellError::BundleLoadFailed(format!("invalid URL: {url_str}")))?;

        // SAFETY: NSURLRequest init and WKWebView load must run on the main thread.
        let request: Retained<objc2_foundation::NSURLRequest> = unsafe {
            let alloc = objc2_foundation::NSURLRequest::alloc();
            msg_send![alloc, initWithURL: &*url]
        };
        unsafe {
            let _: Option<Retained<objc2_web_kit::WKNavigation>> =
                msg_send![&*self.web_view, loadRequest: &*request];
        }

        self.wait_for_navigation_finished(DEFAULT_TIMEOUT)
    }
}
