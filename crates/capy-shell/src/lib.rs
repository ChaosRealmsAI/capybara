pub mod agent;
pub mod app;
pub mod capture;
pub mod ipc;
pub mod store;
#[cfg(target_os = "macos")]
pub mod traffic_light;
pub mod webview;

pub fn run() {
    app::run();
}
