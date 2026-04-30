pub mod agent;
mod agent_tools;
pub mod app;
pub mod browser;
pub mod capture;
pub mod ipc;
mod project_ipc;
mod project_ipc_campaign;
mod project_ipc_clip_queue;
mod project_ipc_surface;
pub mod store;
#[cfg(target_os = "macos")]
pub mod traffic_light;

pub fn run() {
    app::run();
}
