import "./style.css";

const status = document.getElementById("status");

function fail(message) {
  status.dataset.state = "error";
  status.textContent = message;
}

window.addEventListener("error", (event) => {
  fail(`运行失败：${event.message}`);
});
window.addEventListener("unhandledrejection", (event) => {
  fail(`运行失败：${event.reason}`);
});

async function boot() {
  if (!window.isSecureContext) {
    fail("WebGPU 需要安全上下文，请使用 HTTPS 或 localhost 访问。");
    return;
  }
  if (!navigator.gpu) {
    fail("此浏览器没有提供 WebGPU，请使用启用 WebGPU 的浏览器并检查硬件加速设置。");
    return;
  }
  try {
    // Check capability before downloading the assets embedded in the wasm module.
    const [{ default: init, start }, { default: wasmUrl }] = await Promise.all([
      import("./pkg/hello_wgpu.js"),
      import("./pkg/hello_wgpu_bg.wasm?url"),
    ]);
    await init({ module_or_path: wasmUrl });
    start();
  } catch (error) {
    fail(`启动失败：${error instanceof Error ? error.message : String(error)}`);
  }
}

void boot();
