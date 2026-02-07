---
lang: zh-hans
direction: ltr
source: docs/source/iroha_monitor.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05149d624d680d04433be41a4525538c97bd103ae7f80dda2613a6adb181a93d
source_last_modified: "2025-12-29T18:16:35.968850+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 监视器

重构的 Iroha 显示器将轻量级终端 UI 与动画相结合
节日 ASCII 艺术和传统的 Etenraku 主题。  它重点关注两个
简单的工作流程：

- **Spawn-lite 模式** – 启动模仿对等点的临时状态/指标存根。
- **附加模式** – 将监视器指向现有 Torii HTTP 端点。

UI 在每次刷新时呈现三个区域：

1. **Torii 天际线标题** – 动画鸟居、富士山、锦鲤波浪和星星
   与刷新节奏同步滚动的字段。
2. **摘要条** – 聚合区块/交易/gas 加上刷新时间。
3. **同行桌和节日私语** – 同行行在左边，轮流活动
   登录右侧捕获警告（超时、负载过大等）。
4. **可选气体趋势** – 启用 `--show-gas-trend` 附加迷你图
   总结所有同行的 Gas 使用总量。

此次重构的新内容：

- 带有锦鲤、牌坊和灯笼的日式 ASCII 动画场景。
- 简化的命令界面（`--spawn-lite`、`--attach`、`--interval`）。
- 介绍横幅，带有雅乐主题的可选音频播放（外部 MIDI
  播放器或内置软合成器（当平台/音频堆栈支持时）。
- `--no-theme` / `--no-audio` CI 或快速烟雾运行的标志。
- 每个对等点的“心情”列显示最新的警告、提交时间或正常运行时间。

## 快速入门

构建监视器并针对已存根的对等点运行它：

```bash
cargo run -p iroha_monitor -- --spawn-lite --peers 3
```

连接到现有的 Torii 端点：

```bash
cargo run -p iroha_monitor -- \
  --attach http://127.0.0.1:8080 http://127.0.0.1:8081 \
  --interval 500
```

CI 友好的调用（跳过介绍动画和音频）：

```bash
cargo run -p iroha_monitor -- --spawn-lite --no-theme --no-audio
```

### CLI 标志

```
--spawn-lite         start local status/metrics stubs (default if no --attach)
--attach <URL...>    attach to existing Torii endpoints
--interval <ms>      refresh interval (default 800ms)
--peers <N>          stub count when spawn-lite is active (default 4)
--no-theme           skip the animated intro splash
--no-audio           mute theme playback (still prints the intro frames)
--midi-player <cmd>  external MIDI player for the built-in Etenraku .mid
--midi-file <path>   custom MIDI file for --midi-player
--show-gas-trend     render the aggregate gas sparkline panel
--art-speed <1-8>    multiply the animation step rate (1 = default)
--art-theme <name>   choose between night, dawn, or sakura palettes
--headless-max-frames <N>
                     cap headless fallback to N frames (0 = unlimited)
```

## 主题介绍

默认情况下，启动时播放一段简短的 ASCII 动画，同时 Etenraku 乐谱
开始。  音频选择顺序：

1. 如果提供了 `--midi-player`，则生成演示 MIDI（或使用 `--midi-file`）
   并生成命令。
2. 否则，在 macOS/Windows（或具有 `--features iroha_monitor/linux-builtin-synth` 的 Linux）上
   使用内置雅乐软合成器渲染乐谱（无外部音频
   所需资产）。
3. 如果音频被禁用或初始化失败，简介仍会打印
   动画并立即进入 TUI。

由 CPAL 驱动的合成器在 macOS 和 Windows 上自动启用。在 Linux 上是
选择加入以避免在工作区构建期间丢失 ALSA/Pulse 标头；启用它
如果您的系统提供了 `--features iroha_monitor/linux-builtin-synth`
工作音频堆栈。

在 CI 或无头 shell 中运行时，使用 `--no-theme` 或 `--no-audio`。

软合成器现在遵循 *MIDI 合成器设计中捕获的安排
Rust.pdf*： hichiriki 和 ryūteki 共享异声旋律，而 shō
提供文档中描述的 aitake 垫。  定时笔记数据有效
在 `etenraku.rs` 中；它为 CPAL 回调和生成的演示 MIDI 提供动力。
当音频输出不可用时，显示器会跳过播放但仍会呈现
ASCII 动画。

## 用户界面概述- **标题艺术** – 由 `AsciiAnimator` 生成每帧；锦鲤、鸟居灯笼、
  波浪漂移以产生连续的运动。
- **摘要条** – 显示在线对等点、报告的对等点计数、区块总数、
  非空块总数、交易批准/拒绝、gas 使用量和刷新率。
- **对等表** – 别名/端点、块、事务、队列大小的列，
  Gas 使用量、延迟和“情绪”提示（警告、提交时间、正常运行时间）。
- **节日低语** – 滚动警告日志（连接错误、有效负载
  限制违规、缓慢端点）。  消息被反转（最新的在顶部）。

键盘快捷键：

- `n` / 右 / 下 – 将焦点移至下一个对等点。
- `p` / 向左 / 向上 – 将焦点移至上一个点。
- `q` / Esc / Ctrl-C – 退出并恢复终端。

显示器使用 crossterm +ratatui 和备用屏幕缓冲区；退出时
恢复光标并清除屏幕。

## 冒烟测试

该板条箱提供了执行两种模式和 HTTP 限制的集成测试：

- `spawn_lite_smoke_renders_frames`
- `attach_mode_with_stubs_runs_cleanly`
- `invalid_endpoint_surfaces_warning`
- `status_limit_warning_is_rendered`
- `attach_mode_with_slow_peer_renders_multiple_frames`

仅运行监视器测试：

```bash
cargo test -p iroha_monitor -- --nocapture
```

工作区具有较重的集成测试 (`cargo test --workspace`)。跑步
当您这样做时，单独的监视器测试对于快速验证仍然有用
不需要全套。

## 更新截图

文档演示现在重点关注鸟居天际线和对等桌。  要刷新
资产，运行：

```bash
make monitor-screenshots
```

这包装了 `scripts/iroha_monitor_demo.sh`（spawn-lite 模式，固定种子/视口，
无介绍/音频、黎明调色板、art-speed 1、无头帽 24）并写道
SVG/ANSI 帧加上 `manifest.json` 和 `checksums.json` 到
`docs/source/images/iroha_monitor_demo/`。 `make check-iroha-monitor-docs`
包裹两个 CI 防护（`ci/check_iroha_monitor_assets.sh` 和
`ci/check_iroha_monitor_screenshots.sh`) 所以生成器哈希、清单字段、
并且校验和保持同步；屏幕截图检查也作为
`python3 scripts/check_iroha_monitor_screenshots.py`。将 `--no-fallback` 传递给
如果您希望捕获失败而不是退回到演示脚本
当监视器输出为空时烘焙帧；当使用后备时，原始
`.ans` 文件使用烘焙帧重写，以便清单/校验和保留
确定性的。

## 确定性屏幕截图

发送的快照位于 `docs/source/images/iroha_monitor_demo/` 中：

![监视器概述](images/iroha_monitor_demo/iroha_monitor_demo_overview.svg)
![监控管道](images/iroha_monitor_demo/iroha_monitor_demo_pipeline.svg)

使用固定视口/种子再现它们：

```bash
scripts/iroha_monitor_demo.sh \
  --cols 120 --rows 48 \
  --interval 500 \
  --seed iroha-monitor-demo
```

捕获助手修复了 `LANG`/`LC_ALL`/`TERM`，转发
`IROHA_MONITOR_DEMO_SEED`，静音音频，并固定艺术主题/速度，以便
帧在不同平台上呈现相同的效果。它写入 `manifest.json`（发电机
哈希值 + 大小）和 `checksums.json`（SHA-256 摘要）
`docs/source/images/iroha_monitor_demo/`； CI 运行
`ci/check_iroha_monitor_assets.sh` 和 `ci/check_iroha_monitor_screenshots.sh`
当资产偏离记录的清单时失败。

## 故障排除- **无音频输出** – 显示器返回静音播放并继续。
- **无头后备提前退出** – 监视器将无头运行限制为一对
  无法切换时有十几帧（默认间隔约12秒）
  终端进入原始模式；通过 `--headless-max-frames 0` 使其保持运行
  无限期地。
- **超大状态负载** – 同伴的心情栏和节日日志
  显示 `body exceeds …` 以及配置的限制 (`128 KiB`)。
- **慢速对等点** – 事件日志记录超时警告；关注点
  突出显示该行。

欣赏节日天际线！  对其他 ASCII 主题的贡献或
欢迎使用指标面板 - 保持它们的确定性，以便集群呈现相同的效果
无论终端如何，逐帧进行。