import { defineConfig } from "vite-plus";
import { copyFileSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";

export default defineConfig({
  root: "web",
  base: "./",
  plugins: [
    {
      name: "resource-notices",
      apply: "build",
      closeBundle() {
        const notices = resolve("outputs/web/licenses");
        mkdirSync(`${notices}/patterns`, { recursive: true });
        copyFileSync("LICENSE", `${notices}/LICENSE`);
        copyFileSync("assets/OFL.txt", `${notices}/OFL.txt`);
        copyFileSync("assets/SOURCES.md", `${notices}/SOURCES.md`);
        copyFileSync("assets/patterns/PROMPTS.md", `${notices}/patterns/PROMPTS.md`);
        mkdirSync(`${notices}/egui-winit`, { recursive: true });
        for (const name of ["LICENSE-MIT", "LICENSE-APACHE", "PATCHES.md"]) {
          copyFileSync(`vendor/egui-winit/${name}`, `${notices}/egui-winit/${name}`);
        }
      },
    },
  ],
  build: {
    outDir: "../outputs/web",
    emptyOutDir: true,
  },
  fmt: {
    ignorePatterns: ["web/pkg/**", "outputs/**", "target/**", "vendor/**", "README.md"],
  },
  lint: {
    ignorePatterns: ["web/pkg/**", "outputs/**", "target/**", "vendor/**"],
  },
});
