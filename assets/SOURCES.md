# 图片来源与许可

以下三张图片以原始分辨率内嵌于程序。源文件未缩放、未重编码；程序在窗口中等比例绘制，并用 shader 动态模拟折射。

## virtual.jpg — 虚拟场景

- 内容：Sprite Fright 的虚拟森林场景，Blender Studio / Blender Foundation 制作。
- 尺寸：2048 × 858。
- 页面：https://www.blender.org/download/releases/3-0/
- 原图：https://www.blender.org/wp-content/uploads/2021/11/sprite_fright_background_01.jpg
- 署名：(CC) Blender Foundation | studio.blender.org
- 许可：Creative Commons Attribution 1.0（CC BY 1.0）。
- 官方项目许可说明：https://studio.blender.org/projects/sprite-fright/pages/about/
- 许可链接：https://creativecommons.org/licenses/by/1.0/

## photo.jpg — 真实摄影

- 内容：加拿大梦莲湖（Moraine Lake），摄影：Saupra1984。
- 尺寸：6000 × 4000。
- 文件页面：https://commons.wikimedia.org/wiki/File:Morainelake.JPG
- 原图：https://upload.wikimedia.org/wikipedia/commons/2/26/Morainelake.JPG
- 选用许可：Creative Commons Attribution-ShareAlike 3.0（CC BY-SA 3.0）。
- 许可链接：https://creativecommons.org/licenses/by-sa/3.0/

## third-photo.jpg — 用户提供的第三张照片

- 来源：用户指定的 `~/Downloads/CEE3F55C1A9A508EBB592330D7EFC6D2.jpg`，原样复制。
- 尺寸：1080 × 1920。
- 未由本项目指定新的授权，图片版权保留归原权利人。

图片保留各自许可，不适用项目代码的 MIT 许可。窗口面板也显示作者、许可与来源链接；用户提供的图片仅标为“用户提供”。

## 字体

`LXGWWenKai-Regular.ttf` 和 `OFL.txt` 从用户指定的 `~/Downloads/lxgw-wenkai-v1.522/` 原样复制。源字体仅用作构建输入，`build.rs` 自动提取界面字符子集并嵌入程序。中文和正文使用文楷子集；数字等宽内容使用 egui 内置等宽字体，中文回退到文楷子集。额外运行时字符可通过 `font-extra.txt` 加入。字体许可为 SIL OFL 1.1，完整许可和版权声明见 `OFL.txt`。

## 玻璃图案

`patterns/quatrefoil-height.png` 与 `patterns/fern-height.png` 于 2026-10-04 使用 Codex 内置 imagegen 生成，分别是复古四瓣花格与冰花叶脉的艺术化灰度高度图。提示词与处理方式见 [patterns/PROMPTS.md](patterns/PROMPTS.md)。用户参考照片未作为贴图分发，也未复制其水印。橘皮、锤纹、水晶碎面由 `src/patterns.rs` 程序生成。
