use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::AnyObject;
use objc2::MainThreadMarker;
use objc2_foundation::{NSDictionary, NSError, NSString};
use objc2_web_kit::WKContentWorld;

use super::objc_json::objc_to_json;
use super::{MacHeadlessShell, DEFAULT_TIMEOUT};
use crate::webview::{pump_main_run_loop, WebViewEvent};
use crate::ShellError;

fn script_preview(script: &str) -> String {
    const LIMIT: usize = 120;
    let compact = script.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.len() <= LIMIT {
        compact
    } else {
        format!("{}...", &compact[..LIMIT])
    }
}

impl MacHeadlessShell {
    pub fn eval_fire_and_forget(&self, script: &str) -> Result<(), ShellError> {
        let _mtm = MainThreadMarker::new()
            .ok_or_else(|| ShellError::JsCallFailed("not on main thread".into()))?;
        let wrapped_script = format!("(() => {{ {script} }})()");
        let script_ns = autoreleasepool(|_| NSString::from_str(&wrapped_script));
        unsafe {
            self.web_view
                .evaluateJavaScript_completionHandler(&script_ns, None);
        }
        Ok(())
    }

    pub fn pump_for(&self, duration: Duration) {
        autoreleasepool(|_| {
            pump_main_run_loop(duration);
            self.drain_events();
        });
    }

    pub fn eval_sync<'a>(
        &'a self,
        script: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, ShellError>> + Send + 'a>> {
        Box::pin(async move {
            let script_preview = script_preview(script);
            let wrapped_script = format!("(() => {{ {script} }})()");
            let script_ns = autoreleasepool(|_| NSString::from_str(&wrapped_script));
            let outcome: Arc<Mutex<Option<Result<serde_json::Value, String>>>> =
                Arc::new(Mutex::new(None));
            let outcome_clone = Arc::clone(&outcome);

            let completion = RcBlock::new(move |result: *mut AnyObject, error: *mut NSError| {
                let parsed = parse_js_result(result, error);
                if let Ok(mut slot) = outcome_clone.lock() {
                    *slot = Some(parsed);
                }
            });

            unsafe {
                self.web_view
                    .evaluateJavaScript_completionHandler(&script_ns, Some(&completion));
            }

            self.wait_for_js_outcome(&outcome, "evaluateJavaScript", &script_preview)?;
            take_js_outcome(outcome, &script_preview)
        })
    }

    pub(super) fn call_async_script<'a>(
        &'a self,
        script: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, ShellError>> + Send + 'a>> {
        Box::pin(async move {
            let script_preview = script_preview(script);
            let mtm = MainThreadMarker::new()
                .ok_or_else(|| ShellError::JsCallFailed("not on main thread".into()))?;
            let (script_ns, arguments, world) = autoreleasepool(|_| {
                let script_ns = NSString::from_str(script);
                let arguments: Retained<NSDictionary<NSString, AnyObject>> =
                    NSDictionary::from_slices::<NSString>(&[], &[]);
                let world = unsafe { WKContentWorld::pageWorld(mtm) };
                (script_ns, arguments, world)
            });

            let outcome: Arc<Mutex<Option<Result<serde_json::Value, String>>>> =
                Arc::new(Mutex::new(None));
            let outcome_clone = Arc::clone(&outcome);

            let completion = RcBlock::new(move |result: *mut AnyObject, error: *mut NSError| {
                let parsed = parse_js_result(result, error);
                if let Ok(mut slot) = outcome_clone.lock() {
                    *slot = Some(parsed);
                }
            });

            unsafe {
                self.web_view
                    .callAsyncJavaScript_arguments_inFrame_inContentWorld_completionHandler(
                        &script_ns,
                        Some(&arguments),
                        None,
                        &world,
                        Some(&completion),
                    );
            }

            self.wait_for_js_outcome(&outcome, "callAsyncJavaScript", &script_preview)?;
            take_js_outcome(outcome, &script_preview)
        })
    }

    pub(super) fn drain_events(&self) {
        let rx = match self.events_rx.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        while let Ok(ev) = rx.try_recv() {
            if let WebViewEvent::BridgeMessage(value) = ev {
                let name = value
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let handlers = match self.bridge_handlers.lock() {
                    Ok(g) => g,
                    Err(_) => continue,
                };
                for h in handlers.iter() {
                    h(&name, &value);
                }
            }
        }
    }

    pub(super) fn wait_for_navigation_finished(&self, timeout: Duration) -> Result<(), ShellError> {
        let deadline = Instant::now() + timeout;
        loop {
            let navigation_done = autoreleasepool(|_| {
                pump_main_run_loop(Duration::from_millis(8));
                let rx = self.events_rx.lock().map_err(|e| {
                    ShellError::BundleLoadFailed(format!("events rx poisoned: {e}"))
                })?;
                while let Ok(ev) = rx.try_recv() {
                    if matches!(ev, WebViewEvent::NavigationFinished) {
                        return Ok(true);
                    }
                }
                Ok(false)
            })?;
            if navigation_done {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(ShellError::BundleLoadFailed(
                    "timed out waiting for navigation finish".into(),
                ));
            }
        }
    }

    fn wait_for_js_outcome(
        &self,
        outcome: &Arc<Mutex<Option<Result<serde_json::Value, String>>>>,
        api_name: &str,
        script_preview: &str,
    ) -> Result<(), ShellError> {
        let deadline = Instant::now() + DEFAULT_TIMEOUT;
        loop {
            let (completed, expired) = autoreleasepool(|_| {
                pump_main_run_loop(Duration::from_millis(8));
                self.drain_events();
                let completed = outcome.lock().map(|slot| slot.is_some()).unwrap_or(false);
                (completed, Instant::now() >= deadline)
            });
            if completed {
                return Ok(());
            }
            if expired {
                return Err(ShellError::JsCallFailed(format!(
                    "{api_name} timed out · script={script_preview}"
                )));
            }
        }
    }
}

fn parse_js_result(
    result: *mut AnyObject,
    error: *mut NSError,
) -> Result<serde_json::Value, String> {
    unsafe {
        if let Some(err) = error.as_ref() {
            Err(err.localizedDescription().to_string())
        } else if result.is_null() {
            Ok(serde_json::Value::Null)
        } else {
            match Retained::retain(result) {
                Some(obj) => objc_to_json(&obj).map_err(|e| e.to_string()),
                None => Ok(serde_json::Value::Null),
            }
        }
    }
}

fn take_js_outcome(
    outcome: Arc<Mutex<Option<Result<serde_json::Value, String>>>>,
    script_preview: &str,
) -> Result<serde_json::Value, ShellError> {
    let final_result = outcome
        .lock()
        .map_err(|e| ShellError::JsCallFailed(format!("outcome poisoned: {e}")))?
        .take()
        .ok_or_else(|| ShellError::JsCallFailed("no result recorded".into()))?;
    final_result.map_err(|e| ShellError::JsCallFailed(format!("{e} · script={script_preview}")))
}
