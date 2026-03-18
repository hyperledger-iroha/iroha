---
lang: zh-hant
direction: ltr
source: docs/source/iroha_monitor.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05149d624d680d04433be41a4525538c97bd103ae7f80dda2613a6adb181a93d
source_last_modified: "2025-12-29T18:16:35.968850+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 監視器

重構的 Iroha 顯示器將輕量級終端 UI 與動畫相結合
節日 ASCII 藝術和傳統的 Etenraku 主題。  它重點關注兩個
簡單的工作流程：

- **Spawn-lite 模式** – 啟動模仿對等點的臨時狀態/指標存根。
- **附加模式** – 將監視器指向現有 Torii HTTP 端點。

UI 在每次刷新時呈現三個區域：

1. **Torii 天際線標題** – 動畫鳥居、富士山、錦鯉波浪和星星
   與刷新節奏同步滾動的字段。
2. **摘要條** – 聚合區塊/交易/gas 加上刷新時間。
3. **同行桌和節日私語** – 同行行在左邊，輪流活動
   登錄右側捕獲警告（超時、負載過大等）。
4. **可選氣體趨勢** – 啟用 `--show-gas-trend` 附加迷你圖
   總結所有同行的 Gas 使用總量。

此次重構的新內容：

- 帶有錦鯉、牌坊和燈籠的日式 ASCII 動畫場景。
- 簡化的命令界面（`--spawn-lite`、`--attach`、`--interval`）。
- 介紹橫幅，帶有雅樂主題的可選音頻播放（外部 MIDI
  播放器或內置軟合成器（當平台/音頻堆棧支持時）。
- `--no-theme` / `--no-audio` CI 或快速煙霧運行的標誌。
- 每個對等點的“心情”列顯示最新的警告、提交時間或正常運行時間。

## 快速入門

構建監視器並針對已存根的對等點運行它：

```bash
cargo run -p iroha_monitor -- --spawn-lite --peers 3
```

連接到現有的 Torii 端點：

```bash
cargo run -p iroha_monitor -- \
  --attach http://127.0.0.1:8080 http://127.0.0.1:8081 \
  --interval 500
```

CI 友好的調用（跳過介紹動畫和音頻）：

```bash
cargo run -p iroha_monitor -- --spawn-lite --no-theme --no-audio
```

### CLI 標誌

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

## 主題介紹

默認情況下，啟動時播放一段簡短的 ASCII 動畫，同時 Etenraku 樂譜
開始。  音頻選擇順序：

1. 如果提供了 `--midi-player`，則生成演示 MIDI（或使用 `--midi-file`）
   並生成命令。
2. 否則，在 macOS/Windows（或具有 `--features iroha_monitor/linux-builtin-synth` 的 Linux）上
   使用內置雅樂軟合成器渲染樂譜（無外部音頻
   所需資產）。
3. 如果音頻被禁用或初始化失敗，簡介仍會打印
   動畫並立即進入 TUI。

由 CPAL 驅動的合成器在 macOS 和 Windows 上自動啟用。在 Linux 上是
選擇加入以避免在工作區構建期間丟失 ALSA/Pulse 標頭；啟用它
如果您的系統提供了 `--features iroha_monitor/linux-builtin-synth`
工作音頻堆棧。

在 CI 或無頭 shell 中運行時，使用 `--no-theme` 或 `--no-audio`。

軟合成器現在遵循 *MIDI 合成器設計中捕獲的安排
Rust.pdf*： hichiriki 和 ryūteki 共享異聲旋律，而 shō
提供文檔中描述的 aitake 墊。  定時筆記數據有效
在 `etenraku.rs` 中；它為 CPAL 回調和生成的演示 MIDI 提供動力。
當音頻輸出不可用時，顯示器會跳過播放但仍會呈現
ASCII 動畫。

## 用戶界面概述- **標題藝術** – 由 `AsciiAnimator` 生成每幀；錦鯉、鳥居燈籠、
  波浪漂移以產生連續的運動。
- **摘要條** – 顯示在線對等點、報告的對等點計數、區塊總數、
  非空塊總數、交易批准/拒絕、gas 使用量和刷新率。
- **對等表** – 別名/端點、塊、事務、隊列大小的列，
  Gas 使用量、延遲和“情緒”提示（警告、提交時間、正常運行時間）。
- **節日低語** – 滾動警告日誌（連接錯誤、有效負載
  限制違規、緩慢端點）。  消息被反轉（最新的在頂部）。

鍵盤快捷鍵：

- `n` / 右 / 下 – 將焦點移至下一個對等點。
- `p` / 向左 / 向上 – 將焦點移至上一個點。
- `q` / Esc / Ctrl-C – 退出並恢復終端。

顯示器使用 crossterm +ratatui 和備用屏幕緩衝區；退出時
恢復光標並清除屏幕。

## 冒煙測試

該板條箱提供了執行兩種模式和 HTTP 限制的集成測試：

- `spawn_lite_smoke_renders_frames`
- `attach_mode_with_stubs_runs_cleanly`
- `invalid_endpoint_surfaces_warning`
- `status_limit_warning_is_rendered`
- `attach_mode_with_slow_peer_renders_multiple_frames`

僅運行監視器測試：

```bash
cargo test -p iroha_monitor -- --nocapture
```

工作區具有較重的集成測試 (`cargo test --workspace`)。跑步
當您這樣做時，單獨的監視器測試對於快速驗證仍然有用
不需要全套。

## 更新截圖

文檔演示現在重點關注鳥居天際線和對等桌。  要刷新
資產，運行：

```bash
make monitor-screenshots
```

這包裝了 `scripts/iroha_monitor_demo.sh`（spawn-lite 模式，固定種子/視口，
無介紹/音頻、黎明調色板、art-speed 1、無頭帽 24）並寫道
SVG/ANSI 幀加上 `manifest.json` 和 `checksums.json` 到
`docs/source/images/iroha_monitor_demo/`。 `make check-iroha-monitor-docs`
包裹兩個 CI 防護（`ci/check_iroha_monitor_assets.sh` 和
`ci/check_iroha_monitor_screenshots.sh`) 所以生成器哈希、清單字段、
並且校驗和保持同步；屏幕截圖檢查也作為
`python3 scripts/check_iroha_monitor_screenshots.py`。將 `--no-fallback` 傳遞給
如果您希望捕獲失敗而不是退回到演示腳本
當監視器輸出為空時烘焙幀；當使用後備時，原始
`.ans` 文件使用烘焙幀重寫，以便清單/校驗和保留
確定性的。

## 確定性屏幕截圖

發送的快照位於 `docs/source/images/iroha_monitor_demo/` 中：

![監視器概述](images/iroha_monitor_demo/iroha_monitor_demo_overview.svg)
![監控管道](images/iroha_monitor_demo/iroha_monitor_demo_pipeline.svg)

使用固定視口/種子再現它們：

```bash
scripts/iroha_monitor_demo.sh \
  --cols 120 --rows 48 \
  --interval 500 \
  --seed iroha-monitor-demo
```

捕獲助手修復了 `LANG`/`LC_ALL`/`TERM`，轉發
`IROHA_MONITOR_DEMO_SEED`，靜音音頻，並固定藝術主題/速度，以便
幀在不同平台上呈現相同的效果。它寫入 `manifest.json`（發電機
哈希值 + 大小）和 `checksums.json`（SHA-256 摘要）
`docs/source/images/iroha_monitor_demo/`； CI 運行
`ci/check_iroha_monitor_assets.sh` 和 `ci/check_iroha_monitor_screenshots.sh`
當資產偏離記錄的清單時失敗。

## 故障排除- **無音頻輸出** – 顯示器返回靜音播放並繼續。
- **無頭後備提前退出** – 監視器將無頭運行限制為一對
  無法切換時有十幾幀（默認間隔約12秒）
  終端進入原始模式；通過 `--headless-max-frames 0` 使其保持運行
  無限期地。
- **超大狀態負載** – 同伴的心情欄和節日日誌
  顯示 `body exceeds …` 以及配置的限制 (`128 KiB`)。
- **慢速對等點** – 事件日誌記錄超時警告；關注點
  突出顯示該行。

欣賞節日天際線！  對其他 ASCII 主題的貢獻或
歡迎使用指標面板 - 保持它們的確定性，以便集群呈現相同的效果
無論終端如何，逐幀進行。