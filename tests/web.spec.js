import { expect, test } from "@playwright/test";

test("WebGPU renders from a static subdirectory and responds to drag and resize", async ({
  page,
}) => {
  const errors = [];
  const messages = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    messages.push(message.text());
    if (message.type() === "error") errors.push(message.text());
  });
  await page.goto("/web/");
  await expect(page.locator("#status")).toHaveAttribute("data-state", "ready", { timeout: 45_000 });
  expect(messages.some((message) => message.includes("BrowserWebGpu"))).toBe(true);

  const canvas = page.locator("canvas");
  const before = await canvas.screenshot();
  await page.mouse.move(495, 407);
  await page.mouse.down();
  await page.mouse.move(660, 480, { steps: 12 });
  await page.mouse.up();
  await expect.poll(async () => (await canvas.screenshot()).equals(before)).toBe(false);

  const picker = page.waitForEvent("filechooser");
  await page.mouse.click(950, 306);
  await (await picker).setFiles("assets/virtual.jpg");
  await expect
    .poll(() => messages.some((message) => message.includes("Loaded picture: virtual.jpg")))
    .toBe(true);

  await page.setViewportSize({ width: 960, height: 640 });
  await expect(canvas).toHaveJSProperty("width", 960);
  await expect(canvas).toHaveJSProperty("height", 640);
  await page.screenshot({ path: "target/web-tests/webgpu.png" });
  await expect(page.locator("#status")).toHaveAttribute("data-state", "ready");
  console.log(messages.find((message) => message.includes("BrowserWebGpu")));
  expect(errors).toEqual([]);
});

test("missing WebGPU shows an error before loading wasm", async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "gpu", { value: undefined });
  });
  const wasmRequests = [];
  page.on("request", (request) => {
    if (request.url().endsWith(".wasm")) wasmRequests.push(request.url());
  });
  await page.goto("/web/");
  await expect(page.locator("#status")).toHaveAttribute("data-state", "error");
  await expect(page.locator("#status")).toContainText("WebGPU");
  expect(wasmRequests).toEqual([]);
});

test.describe("high DPI", () => {
  test.use({ deviceScaleFactor: 2 });

  test("canvas resolution follows viewport and device pixel ratio", async ({ page }) => {
    // Chromium's emulated DPR does not change devicePixelContentBoxSize.
    // Exercise winit's CSS-size × DPR fallback (also used on browsers without
    // this optional API): https://github.com/microsoft/playwright/issues/18591
    await page.addInitScript(() => {
      delete ResizeObserverEntry.prototype.devicePixelContentBoxSize;
    });
    await page.goto("/web/");
    await expect(page.locator("#status")).toHaveAttribute("data-state", "ready", {
      timeout: 45_000,
    });
    const canvas = page.locator("canvas");
    await expect(canvas).toHaveJSProperty("width", 2560);
    await expect(canvas).toHaveJSProperty("height", 1600);
    await page.setViewportSize({ width: 960, height: 640 });
    await expect(canvas).toHaveJSProperty("width", 1920);
    await expect(canvas).toHaveJSProperty("height", 1280);
    await expect(page.locator("#status")).toHaveAttribute("data-state", "ready");
  });
});
