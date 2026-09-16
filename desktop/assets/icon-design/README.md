# course2md 应用图标

以用户草图中的「播放三角 + Markdown 向下箭头」为核心，保留横向圆角框。
蓝色底面、半透明玻璃框和两个实心符号形成清晰层次；不加入文字、播放器控件或额外装饰。

## 文件

- `Course2MD.icon`：Icon Composer 可编辑源文件。1024 × 1024 画布，3 个 SVG 前景图层、2 个材质分组，背景与材质由 Icon Composer 渲染。
- `layers/`：原始矢量图层，与 `.icon/Assets/` 中的导入图形一致。
- `exports/Course2MD-Preview.png`：默认和深色外观，以及 16 / 32 / 64 / 128 像素的预览。
- `exports/Course2MD-Light.png`、`Course2MD-Dark.png`：1024 像素原生渲染。Light 对应 Icon Composer 的 Default 外观。
- `exports/Course2MD-Clear-Light.png`、`Course2MD-Clear-Dark.png`：原生 Clear 单色外观。
- `exports/Course2MD.icns`：传统 macOS 图标，含 16–512 pt 的 1× / 2× 资源。
- `exports/Course2MD-macOS.png`：带外部留白的传统 macOS 静态 PNG。

`.icon` 中的图形保持完整画布，由系统处理外形；传统 ICNS 的主体按 824/1024 比例居中，作为静态文件的视觉留白。最终色彩、折射与高光会随系统版本和外观变化。

## 参考

在 macOS Icons 实际检索、查看了以下图标，只参考视觉语言，交付图形是按草图重新绘制的矢量。

- [IINA macOS 27](https://macosicons.com/icon/iina-macos-27-icon-NknuInF4jG)：蓝色立体播放符号、简洁轮廓。
- [MarkDownload](https://macosicons.com/icon/markdownload-GgUeq6ENxA)：Markdown 标志的左右排列和向下箭头。
- [Downie — Liquid Glass](https://macosicons.com/icon/downie-liquid-glass-kcamQ8uLyg)：有分量的箭头、深色外观中的材料层次。
- [Apple：App icons](https://developer.apple.com/design/human-interface-guidelines/app-icons)：简洁的核心概念、分层、清晰边缘、系统外形与外观适配。
- [Apple：Creating your app icon using Icon Composer](https://developer.apple.com/documentation/xcode/creating-your-app-icon-using-icon-composer)：SVG 导入及由 Icon Composer 处理材质。

## 继续编辑与重新导出

双击 `Course2MD.icon`，在 Icon Composer 中修改并保存。图形分别位于「Video to Markdown」与「Glass frame」分组；边框图层不透明度为 48%。

在安装了 Xcode（含 Icon Composer）与 Pillow 的 macOS 环境运行：

```sh
uv run python desktop/assets/icon-design/export.py
```

脚本调用 Apple 的 `ictool` 原生渲染器导出外观，再生成 ICNS 与尺寸预览，并同步更新 `desktop/assets/` 下的打包资源；Pillow 只负责静态排版、颜色转换和缩放。此版本使用 Icon Composer 2.0 / 115.1、设计代际 27。

macOS 使用默认外观的静态 `icon.icns`，兼容 macOS 14 起的现有应用包；`.icon` 源文件保留所有可编辑材质与外观。Windows 的 `icon.ico` 包含 16、20、24、32、40、48、64、96、128、256 像素，在 `build.rs` 中编译为编号 `1` 的资源。Linux 使用 `icon.png`。修改后应运行导出脚本，再重新构建 Windows 程序。
