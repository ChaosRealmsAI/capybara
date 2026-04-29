//! `MacHeadlessShell` · `DesktopShell` 的 macOS headless 实现。
//!
//! 线程模型：所有 WKWebView / NSWindow 调用必须在 main thread。对外 trait
//! 声明 `Send + Sync` 是为了类型签名通过，真正跨线程需要上层 actor 包装。

mod bundle;
mod javascript;
mod objc_json;
mod snapshot;

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::time::Duration;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{msg_send, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApp, NSApplicationActivationPolicy, NSBackingStoreType, NSWindow, NSWindowStyleMask,
};
use objc2_core_foundation::CFRetained;
use objc2_core_graphics::CGColorSpace;
use objc2_core_image::CIContext;
use objc2_foundation::{NSActivityOptions, NSPoint, NSProcessInfo, NSRect, NSSize, NSString};
use objc2_web_kit::WKWebView;

use crate::carenderer::CARendererSampler;
use crate::webview::{create_webview, NavigationDelegate, ScriptHandler, WebViewEvent};
use crate::{DesktopShell, IOSurfaceHandle, ShellConfig, ShellError};

pub(crate) use objc_json::objc_to_json;

/// 单次 blocking wait 的默认超时。
pub(super) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// bridge 消息 handler 类型别名。
type BridgeHandler = Box<dyn Fn(&str, &serde_json::Value) + Send + Sync + 'static>;

/// `MacHeadlessShell` · headless WKWebView host。
pub struct MacHeadlessShell {
    window: Retained<NSWindow>,
    web_view: Retained<WKWebView>,
    _script_handler: Retained<ScriptHandler>,
    _navigation_delegate: Retained<NavigationDelegate>,
    events_rx: Mutex<Receiver<WebViewEvent>>,
    bridge_handlers: Mutex<Vec<BridgeHandler>>,
    process_info: Retained<NSProcessInfo>,
    activity_token: Retained<ProtocolObject<dyn objc2_foundation::NSObjectProtocol>>,
    _sampler_legacy: Mutex<CARendererSampler>,
    ci_context: Retained<CIContext>,
    color_space: CFRetained<CGColorSpace>,
    _output_surface: IOSurfaceHandle,
    viewport: (u32, u32),
}

// SAFETY: 对外声明 Send/Sync 是为了满足 DesktopShell trait bounds。实际调用必须
// 在 main thread；多线程调用方需要自行加 actor。
unsafe impl Send for MacHeadlessShell {}
unsafe impl Sync for MacHeadlessShell {}

impl DesktopShell for MacHeadlessShell {
    fn new_headless(config: ShellConfig) -> Result<Self, ShellError>
    where
        Self: Sized,
    {
        let mtm = MainThreadMarker::new().ok_or(ShellError::UnsupportedPlatform)?;

        let (w, h) = config.viewport;
        // 屏幕内但几乎全透明：WindowServer 仍完整 paint，但不拦截用户输入。
        let frame = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(f64::from(w), f64::from(h)),
        );

        // SAFETY: NSWindow alloc + initWith... 需 main thread，mtm 已拿到。
        let window: Retained<NSWindow> = unsafe {
            msg_send![
                NSWindow::alloc(mtm),
                initWithContentRect: frame,
                styleMask: NSWindowStyleMask::Borderless,
                backing: NSBackingStoreType::Buffered,
                defer: false
            ]
        };
        window.setFrame_display(frame, false);
        window.setIgnoresMouseEvents(true);
        unsafe {
            let _: () = msg_send![&*window, setOpaque: false];
            let _: () = msg_send![&*window, setAlphaValue: 0.02f64];
            window.setReleasedWhenClosed(false);
        }

        let (events_tx, events_rx): (Sender<WebViewEvent>, Receiver<WebViewEvent>) =
            mpsc::channel();
        let (web_view, script_handler, navigation_delegate) = create_webview(mtm, frame, events_tx);

        window.setContentView(Some(&web_view));
        let app = NSApp(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        window.makeKeyAndOrderFront(None);

        let process_info = NSProcessInfo::processInfo();
        let activity_reason = NSString::from_str("Timeline headless export");
        let activity_token = process_info.beginActivityWithOptions_reason(
            NSActivityOptions::UserInteractive
                | NSActivityOptions::IdleDisplaySleepDisabled
                | NSActivityOptions::IdleSystemSleepDisabled
                | NSActivityOptions::AnimationTrackingEnabled
                | NSActivityOptions::TrackingEnabled
                | NSActivityOptions::SuddenTerminationDisabled
                | NSActivityOptions::AutomaticTerminationDisabled,
            &activity_reason,
        );

        let sampler = CARendererSampler::new(w, h)?;
        let ci_context = unsafe { CIContext::context() };
        let color_space = CGColorSpace::new_device_rgb().ok_or_else(|| {
            ShellError::SnapshotFailed("CGColorSpaceCreateDeviceRGB returned nil".into())
        })?;
        let output_surface_ref = snapshot::create_output_iosurface(w, h)
            .ok_or_else(|| ShellError::SnapshotFailed("IOSurfaceCreate returned nil".into()))?;
        let output_surface = IOSurfaceHandle::from_surface(output_surface_ref);

        Ok(Self {
            window,
            web_view,
            _script_handler: script_handler,
            _navigation_delegate: navigation_delegate,
            events_rx: Mutex::new(events_rx),
            bridge_handlers: Mutex::new(Vec::new()),
            process_info,
            activity_token,
            _sampler_legacy: Mutex::new(sampler),
            ci_context,
            color_space,
            _output_surface: output_surface,
            viewport: (w, h),
        })
    }

    fn load_bundle(&self, path: &Path) -> Result<(), ShellError> {
        self.load_bundle_path(path)
    }

    fn call_async<'a>(
        &'a self,
        script: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, ShellError>> + Send + 'a>> {
        self.call_async_script(script)
    }

    fn snapshot(&self) -> Result<IOSurfaceHandle, ShellError> {
        self.snapshot_iosurface()
    }

    fn on_bridge_message<F>(&self, handler: F)
    where
        F: Fn(&str, &serde_json::Value) + Send + Sync + 'static,
    {
        if let Ok(mut handlers) = self.bridge_handlers.lock() {
            handlers.push(Box::new(handler));
        }
    }
}

impl Drop for MacHeadlessShell {
    fn drop(&mut self) {
        unsafe {
            self.process_info.endActivity(&self.activity_token);
        }
        self.window.orderOut(None);
        self.window.close();
    }
}
