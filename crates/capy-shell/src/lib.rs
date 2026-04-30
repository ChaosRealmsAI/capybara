pub mod agent;
mod agent_tools;
pub mod app;
pub mod browser;
pub mod capture;
pub mod ipc;
mod project_ipc;
mod project_ipc_campaign;
pub mod store;
#[cfg(target_os = "macos")]
pub mod traffic_light;

pub fn run() {
    app::run();
}
