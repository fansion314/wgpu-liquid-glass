use anyhow::{Context, Result};
use wasm_bindgen::{JsCast, prelude::*};

pub fn canvas() -> Result<web_sys::HtmlCanvasElement> {
    web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("glass-canvas"))
        .context("网页中没有 glass-canvas")?
        .dyn_into()
        .map_err(|_| anyhow::anyhow!("glass-canvas 不是 canvas 元素"))
}

pub fn status(state: &str, message: &str) {
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("status"))
    {
        // A later repaint must not hide an asynchronous GPU or JavaScript error.
        if state == "ready" && element.get_attribute("data-state").as_deref() != Some("loading") {
            return;
        }
        let _ = element.set_attribute("data-state", state);
        element.set_text_content(Some(message));
    }
}

#[wasm_bindgen]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);
    crate::app::run_web().map_err(|error| JsValue::from_str(&format!("{error:#}")))
}
