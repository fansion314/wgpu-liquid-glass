# egui-winit 0.36.2 wasm compatibility

Source: https://github.com/emilk/egui/tree/0.36.2/crates/egui-winit
Copied from the published 0.36.2 crate. The native implementation is unchanged; the added implementation is gated to wasm32.

The published `NativeFile` implements `DroppedFile::bytes` unconditionally, but egui 0.36.2 replaces this method with `bytes_async` on wasm32. This prevents compiling egui-winit for wasm even with native clipboard features disabled.

The sole source patch in `src/dropped_file.rs` gates the native read method and provides an explicit error for asynchronous reads of path-only handles. Winit does not provide a browser File handle in this event. The application's browser image picker uses `rfd::AsyncFileDialog` and passes bytes directly to `scene::memory_image`.

Remove this local copy when an upstream stable release fixes the wasm trait implementation. Upstream license: MIT OR Apache-2.0, included here.
