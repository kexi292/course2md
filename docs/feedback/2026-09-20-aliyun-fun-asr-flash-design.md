# 阿里云 Fun-ASR-Flash 接入设计

## 结论

course2md 新增一个明确的阿里云 DashScope 语音协议模式，首个且唯一适配的模型为
`fun-asr-flash-2026-06-15`。程序继续把本地课程音频切成短 WAV，以 Data URL
（`data:audio/wav;base64,...`）放入 JSON 请求，不上传 OSS，不依赖 Python SDK，
也不启动 `curl` 子进程。

远端接口是同步接口，不返回异步 `task_id`；Rust 侧仍沿用现有
`spawn_blocking` 和有界 worker，使桌面任务保持异步、可显示进度且不阻塞 Tokio
运行时。这里的“异步执行”与阿里云“异步任务 API”是两个概念。

## 模型边界

以下三个名称不能混用：

| 模型 | 调用方式 | 本地音频 | 单文件限制 | 本设计 |
| --- | --- | --- | --- | --- |
| `fun-asr` | 异步提交、轮询结果 | 需要公网 URL | 2 GB、12 小时 | 不接入 |
| `fun-asr-flash-2026-06-15` | 同步 HTTP，可关闭 SSE | URL 或 Base64 | 10 MB、5 分钟 | 接入 |
| `qwen3-asr-flash` | 同步/流式 | URL、Base64 或 SDK 本地文件 | 10 MB、5 分钟 | 不接入 |

选择 Fun-ASR-Flash 是为了同时满足：

- 不引入公网音频托管；
- 不把即将下线的 Qwen ASR 型号作为新功能基础；
- 复用 course2md 已有的本地音频切片、并发、重试和检查点能力。

本设计不为 `fun-asr` 增加 OSS 上传，也不为 Qwen 模型保留隐藏回退。明确选择的
云服务失败时应报告失败，不能静默切换模型并产生另一笔费用。

## 文档依据

本地文件 `.test-tmp/non-realtime-speech-recognition-user-guide.md` 已确认：

- Fun-ASR-Flash 使用
  `/api/v1/services/aigc/multimodal-generation/generation`；
- 该模型是 5 分钟以内音频的同步模型，可设置 `X-DashScope-SSE: disable`；
- 返回结构不是 OpenAI `choices`，正文位于 `output.text`，并可能同时出现在
  `output.output.sentence.text`；
- Fun-ASR-Flash 的单文件上限为 10 MB、5 分钟；
- 北京和新加坡地域提供 `fun-asr-flash-2026-06-15`。

“语音识别概述”另外明确标注 Fun-ASR-Flash 输入为 `URL / Base64`。实现前用内置
短 WAV 做一次真实服务测试，确认 Base64 Data URL 在
`input.messages[].content[].input_audio.data` 中可用。若真实响应与文档不符，停止
实现并保留测试证据，不改用公网 URL 掩盖契约差异。

## 当前实现

当前云端识别集中在 `src/asr.rs`：

1. `media::extract_audio` 生成 16 kHz、单声道、signed 16-bit PCM WAV；
2. `ffmpeg_vad` 按静音和 `max_speech` 切段；
3. `run_api` 用 4 个 worker 并发处理未完成段；
4. `transcribe_api` 只支持 OpenAI `/audio/transcriptions` 和
   `/chat/completions`；
5. 每段成功后立即写入 `asr.jsonl`，重启后按时间边界恢复；
6. 整个阻塞流程已放在 `tokio::task::spawn_blocking` 中。

因此无需另建异步任务系统。真正缺少的是第三种请求/响应契约，以及桌面服务配置对
该契约的明确表达。

## 目标请求契约

### 服务地址

用户保存地域和 Workspace 对应的服务根地址：

```text
https://{WorkspaceId}.cn-beijing.maas.aliyuncs.com/api/v1
```

程序追加：

```text
/services/aigc/multimodal-generation/generation
```

如果用户已输入完整端点，不重复追加。首版只接受 HTTPS 且主机名以
`.maas.aliyuncs.com` 结尾，避免把 DashScope API Key 发送到误填的地址。
代理或私有网关不在首版范围内。

### 请求头

```http
Authorization: Bearer <api-key>
Content-Type: application/json
X-DashScope-SSE: disable
```

首版不支持流式响应。现有按段进度比单段 token 流更符合课程转写任务，也避免新增
SSE 解析器。

### 请求体

```json
{
  "model": "fun-asr-flash-2026-06-15",
  "input": {
    "messages": [
      {
        "role": "user",
        "content": [
          {
            "type": "input_audio",
            "input_audio": {
              "data": "data:audio/wav;base64,<BASE64>"
            }
          }
        ]
      }
    ]
  },
  "parameters": {
    "format": "wav",
    "sample_rate": "16000"
  }
}
```

模型名保留在现有 `AsrApi.model` 中，预设值固定为
`fun-asr-flash-2026-06-15`。协议模式决定端点、请求体和响应解析，不能根据模型名
或 URL 字符串猜测协议。

### 响应

优先读取：

```text
/output/text
```

兼容读取：

```text
/output/output/sentence/text
```

两处都不存在时是契约错误，不能当作静音。字段存在但去除首尾空白后为空，才记录为
已成功识别的静音段。错误响应保留 `request_id`、`code` 和 `message` 用于可读错误，
但不得记录 API Key、Base64 音频或完整服务响应。

## 音频大小与切片

默认 `max_speech = 20` 秒远低于模型限制。16 kHz、单声道、16-bit PCM 的 20 秒
片段约 640 KB，Base64 后约 853 KB。

现有配置允许把 `max_speech` 提高到 600 秒，不能原样用于本协议。Fun-ASR-Flash
模式将 VAD 的有效单段上限收紧到 225 秒：原始 PCM 约 7.2 MB，Base64 后约
9.6 MB，给 WAV 头、Data URL 和 JSON 留出余量，同时低于 5 分钟限制。请求前仍按
实际 Data URL 字节数检查 10,000,000 字节上限；超限时在发送前报错，不产生费用。

检查点身份必须包含协议、完整端点、模型和实际切片上限。这样从 OpenAI 服务切到
Fun-ASR-Flash，或切片边界变化时，不会混用旧转写结果。

## 配置设计

### 核心配置

在现有 `AsrApiMode` 增加一个显式值：

```text
dashscope_fun_asr_flash
```

CLI 对应：

```text
--asr-api-mode dashscope-fun-asr-flash
```

不新增 `AsrProvider`；它仍然是 `api`。不新增通用“厂商插件”接口，因为当前只有一个
新契约，现有 enum 已足够表达。

密钥解析顺序：

1. 当前任务捕获的服务密钥或 `[asr_api].api_key`；
2. `COURSE2MD_ASR_API_KEY`；
3. 仅在本协议下读取 `DASHSCOPE_API_KEY`。

地域必须由服务地址和密钥共同决定，程序不猜测或跨地域重试。

### 桌面服务配置

在现有语音“接口类型”单选组增加“阿里云 Fun-ASR-Flash”。它复用现有
`SingleChoiceGroup`、服务地址、模型和密钥字段，不新建卡片、向导页或视觉样式。

选择该协议时：

- 新配置预填模型 `fun-asr-flash-2026-06-15`；
- 地址说明要求填写包含 Workspace ID 的地域根地址；
- 测试按钮旁继续显示现有外部请求后果；
- 已保存的 OpenAI 语音服务不自动迁移，也不被覆盖；
- 提交后的任务继续捕获不可变服务版本，修改默认值不改变排队或运行中的任务。

无需增加“异步/同步”开关。它是协议固有行为，不是用户决策。

## 代码落点

### `src/settings.rs`

- 为 `AsrApiMode` 增加 `DashscopeFunAsrFlash`；
- 保持现有 `transcriptions`、`chat` 序列化值兼容；
- 在配置模板中给出 DashScope 示例，不改变当前默认服务。

### `src/config.rs`

- `asr_endpoint` 为新模式追加 DashScope generation 路径；
- 新模式校验 HTTPS、阿里云主机名、模型和密钥；
- 密钥环境变量增加仅限该模式的 `DASHSCOPE_API_KEY` 回退。

### `src/asr.rs`

- 复用 `ffmpeg_vad`、`cut_wav`、worker、重试和 checkpoint；
- 为新模式构造 Base64 Data URL 和 DashScope JSON；
- 设置 `X-DashScope-SSE: disable`；
- 按新响应路径取正文；
- 在发送前校验实际编码长度；
- 让请求收据的结构校验按 `AsrApiMode` 执行，不能把其他协议的响应误记为成功。

现有 `sanitize_qwen_text` 不用于 Fun-ASR-Flash，除非真实响应证明确实含有相同控制
标记。不要基于另一个模型的历史行为做清洗。

### `desktop/src/preferences.rs`

- 为 `ServiceProtocol` 增加 `SpeechDashscopeFunAsrFlash`；
- 映射到新的 `AsrApiMode` 和 endpoint suffix；
- 保持旧服务版本反序列化兼容。

### `desktop/src/service_test.rs`

- 使用内置 `service-speech.wav` 生成相同 Data URL 请求；
- 使用新响应路径校验样例文本；
- 将 401/403、429、5xx、模型拒绝和响应结构错误保持为不同测试结果；
- 测试响应不持久化 Base64 请求体。

### `desktop/src/onboarding.rs` 与 `desktop/src/settings_ui.rs`

- 仅把新协议加入已有接口类型选择；
- 复用现有字段、帮助信息、保存、取消、测试和错误状态；
- 不新增页面、弹窗、动效或控件样式。

## 并发、重试与取消

- 保持当前最多 4 个分段请求并发，不增加新的并发配置；
- HTTP 429、5xx 和明确的发送前网络失败沿用最多 3 次指数退避；
- 4xx 不重试；
- 已发送但响应中断继续使用现有 `uncertain` 请求语义，不能无界自动重发；
- 用户取消后不再领取新分段；已经进入阻塞 HTTP 调用的请求等待返回或 120 秒超时；
- 每段完成后立即落盘，应用重启时只处理未完成段。

进度仍表示“已完成分段数 / 总分段数”，不显示虚构百分比或阿里云内部处理阶段。

## 不采用的方案

### Python DashScope SDK

不采用。Rust 主程序已有 HTTP、JSON、Base64、重试和任务控制能力。引入 Python 会增加
运行时、包安装、版本和跨平台诊断问题，却没有提供本接口必需的额外能力。

### 调用 `curl`

不采用。`curl` 示例只是在说明 HTTP 契约；子进程会削弱超时、取消、错误分类、密钥
隐藏和跨平台一致性。使用已安装的 `ureq` 直接发出等价请求即可。

### `fun-asr` + OSS

不采用。它需要临时公网对象、上传凭据、生命周期清理和额外费用，明显扩大功能边界。
只有未来确实需要超过 5 分钟的单次上下文或 12 小时文件转写时才重新评估。

### `qwen3-asr-flash` Base64

不采用。它技术上最接近现有 chat 请求，但已知存在下线风险。为它新增正式协议只会
产生马上需要迁移的配置和测试。

## 实施顺序

1. 为新协议补 endpoint、请求体、响应解析和大小边界单元测试。
2. 在 `src/asr.rs` 接入现有 worker/checkpoint，使用本地假 HTTP 服务验证请求字节和
   失败分类。
3. 增加核心配置与 CLI 值，验证旧 TOML 不变。
4. 增加桌面 `ServiceProtocol`、服务测试和现有选择组中的新选项。
5. 使用真实 DashScope 工作区和内置短音频做一次服务测试，再跑一段包含多个 VAD
   分段的课程。
6. 检查暂停、取消、失败、恢复和完成状态；确认没有 Base64 音频或密钥进入日志、
   收据和任务 JSON。

核心协议和桌面配置分成两个原子提交。真实服务验证完成前不把该协议设为默认值。

## 必须覆盖的检查

- Data URL 解码后与输入 WAV 字节完全一致；
- 请求使用正确 endpoint、模型、header、format 和 sample rate；
- `output.text` 与兼容嵌套路径都能解析；
- 缺少文本字段是硬错误，显式空文本记录为已完成静音段；
- 超过编码大小上限时请求尚未发送；
- 429/5xx 有界重试，4xx 不重试；
- 切换协议、endpoint、模型或有效切片上限会使旧 checkpoint 失效；
- 中断后只重跑未完成分段；
- 旧 `transcriptions`、`chat` 服务和配置文件行为不变；
- 桌面服务测试不会把密钥或 Base64 音频写入证据；
- 原生应用中，新选项在首次配置和设置编辑器里都复用现有选择、保存、失败和返回
  行为。

## 验收标准

在 Windows 原生应用中配置北京或新加坡 DashScope 工作区后：

1. 内置语音样例测试通过，并显示实际使用的模型与协议；
2. 本地课程无需 OSS 即可完成多段转写；
3. 暂停或退出后重开，已完成分段不重复计费；
4. 无效密钥、错误 Workspace、模型无权限、429、5xx 和响应缺字段各自给出可区分错误；
5. 任务详情与导出时间线使用分段原始时间，正文无重复、无丢段；
6. 日志、配置展示、请求账本和任务文件中不存在明文密钥或 Base64 音频。

