use objc2::define_class;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, msg_send, sel};
use objc2_app_kit::{NSWindow, NSWindowButton};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSRect, NSString, ns_string};

pub const TOPBAR_HEIGHT_PT: f64 = 48.0;
const PADDING_X_PT: f64 = 20.0;

fn apply(window: &NSWindow) {
    let close = match window.standardWindowButton(NSWindowButton::CloseButton) {
        Some(button) => button,
        None => return,
    };
    let mini = match window.standardWindowButton(NSWindowButton::MiniaturizeButton) {
        Some(button) => button,
        None => return,
    };
    let zoom = match window.standardWindowButton(NSWindowButton::ZoomButton) {
        Some(button) => button,
        None => return,
    };

    let win_frame = window.frame();
    #[allow(unsafe_code)]
    let content_rect: NSRect = unsafe { msg_send![window, contentLayoutRect] };
    let real_titlebar_h = win_frame.size.height - content_rect.size.height;
    let container_h = real_titlebar_h.max(TOPBAR_HEIGHT_PT);

    let close_frame = close.frame();
    let mini_frame = mini.frame();
    let btn_h = close_frame.size.height;
    let spacing = (mini_frame.origin.x - close_frame.origin.x).max(20.0);

    if container_h > real_titlebar_h {
        #[allow(unsafe_code)]
        let titlebar_container = unsafe { close.superview().and_then(|view| view.superview()) };
        if let Some(titlebar_container) = titlebar_container {
            let mut rect = titlebar_container.frame();
            rect.size.height = container_h;
            rect.origin.y = win_frame.size.height - container_h;
            #[allow(unsafe_code)]
            let _: () = unsafe { msg_send![&*titlebar_container, setFrame: rect] };
        }
    }

    let y = (container_h - btn_h) / 2.0;
    let mut x = PADDING_X_PT;

    for button in [&close, &mini, &zoom] {
        let mut frame = button.frame();
        frame.origin.x = x;
        frame.origin.y = y;
        #[allow(unsafe_code)]
        let _: () = unsafe { msg_send![&**button, setFrame: frame] };
        x += spacing;
    }
}

pub struct TrafficLightObserverIvars {
    pub window: Retained<NSWindow>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = TrafficLightObserverIvars]
    pub struct TrafficLightObserver;

    unsafe impl NSObjectProtocol for TrafficLightObserver {}

    impl TrafficLightObserver {
        #[unsafe(method(onNeedsReapply:))]
        fn on_needs_reapply(&self, _notification: &NSNotification) {
            apply(&self.ivars().window);
        }
    }
);

pub fn install(
    mtm: MainThreadMarker,
    window: Retained<NSWindow>,
) -> Retained<TrafficLightObserver> {
    apply(&window);

    let observer = mtm
        .alloc::<TrafficLightObserver>()
        .set_ivars(TrafficLightObserverIvars {
            window: window.clone(),
        });
    #[allow(unsafe_code)]
    let observer: Retained<TrafficLightObserver> = unsafe { msg_send![super(observer), init] };

    let center: Retained<NSNotificationCenter> = NSNotificationCenter::defaultCenter();
    let selector = sel!(onNeedsReapply:);
    let names: &[&NSString] = &[
        ns_string!("NSWindowDidResizeNotification"),
        ns_string!("NSWindowDidBecomeKeyNotification"),
        ns_string!("NSWindowDidResignKeyNotification"),
        ns_string!("NSWindowDidEnterFullScreenNotification"),
        ns_string!("NSWindowDidExitFullScreenNotification"),
        ns_string!("NSWindowDidMiniaturizeNotification"),
        ns_string!("NSWindowDidDeminiaturizeNotification"),
    ];

    for name in names {
        #[allow(unsafe_code)]
        unsafe {
            let window_any: &AnyObject = std::mem::transmute::<&NSWindow, &AnyObject>(&*window);
            let observer_any: &AnyObject =
                std::mem::transmute::<&TrafficLightObserver, &AnyObject>(&*observer);
            let _: () = msg_send![
                &*center,
                addObserver: observer_any,
                selector: selector,
                name: *name,
                object: window_any,
            ];
        }
    }

    observer
}

pub fn install_from_tao(window: &tao::window::Window) -> Option<Retained<TrafficLightObserver>> {
    use tao::platform::macos::WindowExtMacOS;

    let ptr = window.ns_window();
    if ptr.is_null() {
        return None;
    }
    let mtm = MainThreadMarker::new()?;
    #[allow(unsafe_code)]
    let ns_window: Retained<NSWindow> = unsafe {
        let borrowed: &NSWindow = &*(ptr as *const NSWindow);
        Retained::retain(borrowed as *const NSWindow as *mut NSWindow)?
    };
    Some(install(mtm, ns_window))
}
