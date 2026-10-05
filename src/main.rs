#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    hello_wgpu::run()
}

#[cfg(target_arch = "wasm32")]
fn main() {}
