# Windows 桌面端首次配置反馈（2026-09-10）

## 背景

单用户主要在 Windows 使用 course2md，目标是顺利完成“本地视频 → 本地语音识别 → 图文笔记”。AI 校对可后续配置，不应阻塞本地转换。

当前测试环境：

- Windows 11 Pro x64
- AMD Ryzen 7 8845H，约 28 GB 可用内存
- AMD Radeon 780M
- 桌面包：`F:\dev\vibewatching\course2md-desktop-windows-AMD64`
- `ffmpeg` / `ffprobe`：可用
- `llama-server` CPU 版：`F:\dev\vibewatching\course2md-desktop-windows-AMD64\llama\llama-server.exe`
- Qwen3-ASR 模型：`D:\course2md\models\llama-qwen3-1.7b`
- 模型已通过 `models inspect`，状态为 `cached`，无缺失文件

## 反馈 1：桌面端没有采用 config.toml 中的新模型目录

### 现象

在 `%APPDATA%\course2md\config.toml` 中配置：

```toml
[defaults]
provider = "cpu"
asr_model = "qwen3"
model_dir = "D:/course2md/models"
transcript_source = "auto"
```

配置能够成功解析，但桌面端仍显示或读取之前的系统缓存路径，而不是 `D:\course2md\models`。

### 初步代码证据

`desktop/src/main.rs` 启动时读取 `desktop-preferences`：

```rust
let preferences = preferences::Store::open(
    configuration_directory.join("desktop-preferences"),
    credentials::system_vault(),
);
let mut config = preferences.defaults_config();
```

因此桌面端当前设置来源与 CLI 的 `config.toml` 并不完全一致。手工修改 `config.toml` 后，已有桌面偏好可能继续覆盖或忽略它。

### 期望

- 桌面设置中明确显示当前生效的模型根目录。
- 模型目录可以直接编辑或通过文件夹选择器修改。
- 保存后立即重新检查该目录，不要求用户编辑 TOML。
- 已存在且完整的模型应显示“已验证，可以识别”。
- CLI 配置与桌面偏好的优先级必须可解释，不能静默使用旧缓存路径。

## 反馈 2：首次配置不完整时，AI 服务和模型配置难以完成

### 现象

- 模型已经手工下载完成，但桌面端仍可能认为模型未配置完成。
- AI 服务配置入口也出现无法完成配置的情况。
- 尚未确认是字段校验、设置保存、服务引用还是首次使用状态之间的依赖导致。

### 期望

- 本地 ASR 与 AI 校对是独立能力。
- 没有配置 AI 服务时，本地视频、本地模型和 CPU 转写仍能正常使用。
- AI 服务可以保持“未配置/关闭”，不能阻塞设置保存或应用退出。
- 设置页应指出具体缺少哪个字段，不能只表现为“配置未完成”。
- 已保存值、当前编辑值和实际生效值应保持一致。

## 反馈 3：Windows 点击标题栏关闭按钮后桌面端没有退出

### 现象

点击 Windows 标题栏右上角关闭按钮后，桌面端没有正常关闭，表现为窗口消失或关闭动作无效，但应用进程仍可能存在。

用户当时有多项设置尚未配置完成，因此还需验证未完成设置是否会触发保存失败或关闭拦截。

### 初步代码证据

`desktop/src/main.rs` 的窗口关闭处理当前无条件执行：

```rust
if saved {
    cx.hide();
}
false
```

这会隐藏窗口并拒绝真正关闭。该行为符合部分 macOS 应用习惯，但不符合 Windows 标题栏关闭按钮的通常语义。

另有 `request_close()` 实现了保存、取消后台操作和最长 10 秒退出等待，但 Windows 标题栏关闭路径目前没有调用它。

### 期望

- Windows 点击关闭按钮应执行真正退出，而不是仅隐藏窗口。
- 退出前保存必要状态；保存失败时窗口保持显示，并给出可操作错误。
- 未配置可选服务不能阻止退出。
- 有后台转换任务时应按现有退出策略保存/取消，最长等待后仍能退出。
- macOS 是否保留“关闭窗口但不退出应用”的行为应按平台分别处理。

## 建议处理顺序

1. 修复 Windows 标题栏关闭路径，使其复用 `request_close()`。
2. 让模型目录在桌面设置中可编辑并显示实际生效路径。
3. 明确 `config.toml` 与 `desktop-preferences` 的导入、覆盖和优先级。
4. 解耦本地 ASR 与 AI 服务配置，保证 AI 未配置不阻塞本地转换。
5. 在 Windows 原生应用中验证首次启动、设置保存、模型检查、退出和重启后的持久化。
6. 完成本地构建和 Windows ZIP 打包，不修改发布标签。

## 验收场景

1. 清空或隔离桌面偏好后首次启动，选择 CPU 和 `D:\course2md\models`。
2. 重启应用，路径仍为 D 盘且模型状态可用。
3. AI 服务保持关闭，成功转换一个无字幕的本地短视频。
4. AI 服务填写不完整时，能够取消编辑并继续使用本地 ASR。
5. 空闲、模型检查中、设置编辑中和转换任务中分别点击 Windows 关闭按钮。
6. 确认窗口关闭后 `course2md-desktop.exe` 不再驻留；必要后台子进程也被回收。

## 状态

仅记录反馈与初步代码证据，尚未修改代码、编译或打包。
