import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const lock = readFileSync(join(root, "Cargo.lock"), "utf8");
const version = lock.match(/name = "wasm-bindgen"\nversion = "([^"]+)"/)[1];

function capture(command, args) {
  const result = spawnSync(command, args, { cwd: root, encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : undefined;
}

function run(command, args, env = process.env) {
  const result = spawnSync(command, args, { cwd: root, env, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

// Use the real toolchain executables; retain Cargo's configured sccache wrapper.
const cargo = capture("rustup", ["which", "cargo"]);
const rustc = process.env.RUSTC ?? capture("rustup", ["which", "rustc"]);
if (!cargo || !rustc) throw new Error("请先安装稳定版 Rust 和 rustup。");
const env = { ...process.env, RUSTC: rustc };
const target = "wasm32-unknown-unknown";
const installed = capture("rustup", ["target", "list", "--installed"]);
if (!installed?.split(/\s+/).includes(target)) run("rustup", ["target", "add", target]);

const toolRoot = join(root, "target", "tools", `wasm-bindgen-${version}`);
const localTool = join(
  toolRoot,
  "bin",
  process.platform === "win32" ? "wasm-bindgen.exe" : "wasm-bindgen",
);
const candidates = [process.env.WASM_BINDGEN, "wasm-bindgen", localTool].filter(Boolean);
let bindgen = candidates.find(
  (command) => capture(command, ["--version"]) === `wasm-bindgen ${version}`,
);
if (!bindgen) {
  console.log(`安装与 Cargo.lock 匹配的 wasm-bindgen ${version} 到 target/tools…`);
  run(
    cargo,
    ["install", "--locked", "--version", version, "--root", toolRoot, "wasm-bindgen-cli"],
    env,
  );
  bindgen = localTool;
}

run(cargo, ["build", "--locked", "--release", "--target", target, "--lib"], env);
const targetDir = process.env.CARGO_TARGET_DIR ?? join(root, "target");
const wasm = join(targetDir, target, "release", "hello_wgpu.wasm");
if (!existsSync(wasm)) throw new Error(`找不到 wasm 构建产物：${wasm}`);
run(bindgen, [wasm, "--target", "web", "--out-dir", "web/pkg", "--out-name", "hello_wgpu"]);
