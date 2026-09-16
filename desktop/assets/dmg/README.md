# course2md 安装窗口

安装窗口沿用 macOS 常见的「应用 → Applications」布局：浅灰背景、系统字体、单一方向箭头，以及中英文拖动提示。760 × 540 的窗口为用户开启的 Finder 标签栏、路径栏、状态栏留出空间。应用和文件夹使用 Finder 中真正可拖动的项目，背景只包含文字与箭头。

![Finder 实机预览](preview.png)

## 方案

对比了两个 create-dmg 项目与 dmgbuild：

- [sindresorhus/create-dmg](https://github.com/sindresorhus/create-dmg)：现成的简洁安装窗口是视觉参考。
- [create-dmg/create-dmg](https://github.com/create-dmg/create-dmg)：支持定制，但美化步骤依赖 Finder AppleScript，CI 需要额外处理图形会话。
- [dmgbuild](https://dmgbuild.readthedocs.io/en/latest/)：最终采用。直接写入 Finder 的 `.DS_Store` 布局，Python 打包脚本和无 GUI 的 CI 都可使用。

`desktop/scripts/dmg.py` 配置窗口、图标位置和 Applications 链接；`background.tiff` 同时包含 1× / 2× 图像，`background@2x.png` 用于预览。重新渲染背景只需 macOS 的 AppKit 和 Swift：

```sh
swift desktop/assets/dmg/render.swift
uv sync
uv run python desktop/scripts/package.py --debug --no-build
```

`--no-build` 复用已构建的本机二进制。发布时沿用原有签名、公证、staple 顺序；dmgbuild 使用 ditto 复制应用。DMG 根目录只显示应用和 Applications，许可证与构建来源放入已签名应用的 Resources，ZIP 继续包含开发说明。

## 验证记录 · 2026-09-07

- 检查了已发布 `v1.7.0` Windows GUI ZIP：SHA-256 为 `e08beb91320a1ff06ad0b2c8ca10ca380f44a99928303927964f53e27de0a80f`，GUI `.exe` 只有 RT_MANIFEST（24），没有 RT_ICON（3）或 RT_GROUP_ICON（14）。
- 新的 `build.rs` 在本机通过 LLVM 生成 Windows 资源并链接为最小 PE 验证程序，编号 `1` 的全部 10 个尺寸与源 ICO 字节一致；验证脚本也成功拒绝旧版 GUI `.exe`。
- `cargo check --manifest-path desktop/Cargo.toml --locked --offline` 和 `cargo fmt --manifest-path desktop/Cargo.toml --check` 通过。GPUI 上游仍有现存弃用警告。
- 本机实际打包开发版 DMG，并在 Finder 通过双击磁盘镜像验收。打包脚本自动重新挂载成品，核对背景、图标位置、Applications 链接、许可证和应用签名。

本次未在 Windows 桌面运行 GUI；完整 Windows 成品的资源检查已接入开发与发布 CI 的打包步骤。当前 GitHub Release 尚未更新。本机预览 DMG 使用现有开发二进制与 ad-hoc 签名。

原安装窗口：

![原安装窗口](preview-before.png)
