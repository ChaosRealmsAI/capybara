use std::collections::HashMap;

use serde::Serialize;
#[cfg(target_os = "macos")]
use tao::platform::macos::WindowExtMacOS;
use tao::window::{UserAttentionType, Window, WindowId};

use crate::app::ShellEvent;
use crate::browser::ShellBrowser;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WindowStatus {
    pub window_id: String,
    pub project: String,
    pub focused: bool,
}

#[derive(Debug, Clone)]
struct WindowMeta {
    project: String,
}

pub(crate) struct WindowManager {
    windows: HashMap<String, Window>,
    pub(crate) webviews: HashMap<String, ShellBrowser>,
    id_by_wid: HashMap<WindowId, String>,
    metadata: HashMap<String, WindowMeta>,
    window_numbers: HashMap<String, u32>,
    focused_window_id: Option<String>,
    next_seq: u64,
}

impl WindowManager {
    pub(crate) fn new() -> Self {
        Self {
            windows: HashMap::new(),
            webviews: HashMap::new(),
            id_by_wid: HashMap::new(),
            metadata: HashMap::new(),
            window_numbers: HashMap::new(),
            focused_window_id: None,
            next_seq: 1,
        }
    }

    pub(crate) fn open(
        &mut self,
        target: &tao::event_loop::EventLoopWindowTarget<ShellEvent>,
        proxy: &tao::event_loop::EventLoopProxy<ShellEvent>,
        project: &str,
    ) -> Result<String, String> {
        if let Some(window_id) = self.find_by_project(project, None) {
            self.focus(&window_id)?;
            return Ok(window_id);
        }
        self.open_new(target, proxy, project)
    }

    pub(crate) fn open_new(
        &mut self,
        target: &tao::event_loop::EventLoopWindowTarget<ShellEvent>,
        proxy: &tao::event_loop::EventLoopProxy<ShellEvent>,
        project: &str,
    ) -> Result<String, String> {
        let window_id = self.allocate_window_id();
        let (window, webview) =
            crate::browser::create_window(target, proxy.clone(), &window_id, project)?;
        if let Some(window_number) = native_window_number(&window) {
            self.window_numbers.insert(window_id.clone(), window_number);
        }
        self.id_by_wid.insert(window.id(), window_id.clone());
        self.metadata.insert(
            window_id.clone(),
            WindowMeta {
                project: project.to_string(),
            },
        );
        self.webviews.insert(window_id.clone(), webview);
        self.windows.insert(window_id.clone(), window);
        self.focused_window_id = Some(window_id.clone());
        Ok(window_id)
    }

    pub(crate) fn remove_by_tao_window_id(&mut self, tao_id: WindowId) {
        let Some(window_id) = self.id_by_wid.remove(&tao_id) else {
            return;
        };
        self.windows.remove(&window_id);
        self.webviews.remove(&window_id);
        self.metadata.remove(&window_id);
        self.window_numbers.remove(&window_id);
        if self.focused_window_id.as_deref() == Some(window_id.as_str()) {
            self.focused_window_id = None;
        }
    }

    pub(crate) fn quit_all(&mut self) {
        self.webviews.clear();
        self.windows.clear();
        self.id_by_wid.clear();
        self.metadata.clear();
        self.window_numbers.clear();
        self.focused_window_id = None;
    }

    pub(crate) fn list(&self) -> Vec<WindowStatus> {
        let mut statuses: Vec<WindowStatus> = self
            .metadata
            .iter()
            .map(|(window_id, meta)| WindowStatus {
                window_id: window_id.clone(),
                project: meta.project.clone(),
                focused: self.focused_window_id.as_deref() == Some(window_id.as_str()),
            })
            .collect();
        statuses.sort_by(|left, right| left.window_id.cmp(&right.window_id));
        statuses
    }

    pub(crate) fn webview_for_target(
        &self,
        window_id: Option<&str>,
    ) -> Result<(String, &ShellBrowser), String> {
        let target_id = self
            .find_target(window_id)
            .ok_or_else(|| "no open Capybara window".to_string())?;
        let webview = self
            .webviews
            .get(&target_id)
            .ok_or_else(|| format!("webview missing for {target_id}"))?;
        Ok((target_id, webview))
    }

    pub(crate) fn window_by_id(&self, window_id: &str) -> Result<&Window, String> {
        self.windows
            .get(window_id)
            .ok_or_else(|| format!("no such window: {window_id}"))
    }

    pub(crate) fn webview_by_id(&self, window_id: &str) -> Result<&ShellBrowser, String> {
        self.webviews
            .get(window_id)
            .ok_or_else(|| format!("webview missing for {window_id}"))
    }

    pub(crate) fn native_window_number_for_target(
        &self,
        window_id: Option<&str>,
    ) -> Result<(String, u32), String> {
        let target_id = self
            .find_target(window_id)
            .ok_or_else(|| "no open Capybara window".to_string())?;
        let window_number = self
            .window_numbers
            .get(&target_id)
            .copied()
            .ok_or_else(|| format!("native window number missing for {target_id}"))?;
        Ok((target_id, window_number))
    }

    pub(crate) fn focus(&mut self, window_id: &str) -> Result<(), String> {
        let window = self
            .windows
            .get(window_id)
            .ok_or_else(|| format!("no such window: {window_id}"))?;
        activate_current_app();
        window.set_visible(true);
        window.set_focus();
        if let Some(webview) = self.webviews.get(window_id) {
            webview.set_focus(true);
        }
        window.request_user_attention(Some(UserAttentionType::Informational));
        self.focused_window_id = Some(window_id.to_string());
        Ok(())
    }

    fn find_by_project(&self, project: &str, window_id: Option<&str>) -> Option<String> {
        if let Some(window_id) = window_id {
            return self
                .metadata
                .get(window_id)
                .and_then(|meta| (meta.project == project).then(|| window_id.to_string()));
        }
        self.metadata
            .iter()
            .find(|(_, meta)| meta.project == project)
            .map(|(id, _)| id.clone())
    }

    fn find_target(&self, window_id: Option<&str>) -> Option<String> {
        if let Some(window_id) = window_id {
            return self
                .windows
                .contains_key(window_id)
                .then(|| window_id.to_string());
        }
        self.focused_window_id
            .as_ref()
            .filter(|id| self.windows.contains_key(id.as_str()))
            .cloned()
            .or_else(|| self.windows.keys().next().cloned())
    }

    fn allocate_window_id(&mut self) -> String {
        loop {
            let id = format!("w-{}", self.next_seq);
            self.next_seq += 1;
            if !self.windows.contains_key(&id) {
                return id;
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[allow(clashing_extern_declarations)]
fn activate_current_app() {
    use std::ffi::c_void;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const i8) -> *mut c_void;
        fn sel_registerName(name: *const i8) -> *mut c_void;
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_id(receiver: *mut c_void, selector: *mut c_void) -> *mut c_void;
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_bool(receiver: *mut c_void, selector: *mut c_void, value: i8);
    }

    let app_class = unsafe { objc_getClass(c"NSApplication".as_ptr()) };
    if app_class.is_null() {
        return;
    }
    let shared_selector = unsafe { sel_registerName(c"sharedApplication".as_ptr()) };
    let activate_selector = unsafe { sel_registerName(c"activateIgnoringOtherApps:".as_ptr()) };
    if shared_selector.is_null() || activate_selector.is_null() {
        return;
    }
    let app = unsafe { objc_msg_send_id(app_class, shared_selector) };
    if app.is_null() {
        return;
    }
    unsafe {
        objc_msg_send_bool(app, activate_selector, 1);
    }
}

#[cfg(not(target_os = "macos"))]
fn activate_current_app() {}

#[cfg(target_os = "macos")]
#[allow(clashing_extern_declarations)]
fn native_window_number(window: &Window) -> Option<u32> {
    use std::ffi::c_void;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn sel_registerName(name: *const i8) -> *mut c_void;
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_window_number(receiver: *mut c_void, selector: *mut c_void) -> isize;
    }

    let ns_window = window.ns_window();
    if ns_window.is_null() {
        return None;
    }
    let selector = unsafe { sel_registerName(c"windowNumber".as_ptr()) };
    if selector.is_null() {
        return None;
    }
    let number = unsafe { objc_msg_send_window_number(ns_window.cast(), selector) };
    u32::try_from(number).ok().filter(|number| *number > 0)
}

#[cfg(not(target_os = "macos"))]
fn native_window_number(_window: &Window) -> Option<u32> {
    None
}
