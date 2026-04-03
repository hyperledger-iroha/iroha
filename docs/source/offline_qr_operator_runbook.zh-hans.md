<!-- Auto-generated stub for Chinese (Simplified) (zh-hans) translation. Replace this content with the full translation. -->

---
lang: zh-hans
direction: ltr
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## 离线 QR 操作手册

此操作手册定义了实用的 `ecc`/dimension/fps 预设，以应对相机噪音
使用离线 QR 传输时的环境。

### 推荐预设

|环境 |风格| ECC |尺寸|第一人称射击 |块大小|奇偶组|笔记|
| ---| ---| ---| ---| ---| ---| ---| ---|
|短距离受控照明 | `sakura` | `M` | `360` | `12` | `360` | `0` |最高吞吐量，最小冗余。 |
|典型的移动相机噪音| `sakura-storm` | `Q` | `512` | `12` | `336` | `4` |混合设备的首选平衡预设 (`~3 KB/s`)。 |
|高眩光、运动模糊、低端相机 | `sakura-storm` | `H` | `640` | `8` | `280` | `6` |吞吐量较低，解码弹性最强。 |

### 编码/解码清单

1. 使用显式传输旋钮进行编码。
2. 在推出之前使用扫描仪循环捕获进行验证。
3. 在 SDK 播放助手中固定相同的样式配置文件，以保持预览奇偶性。

示例：

```bash
iroha offline qr encode \
  --style sakura-storm \
  --ecc Q \
  --dimension 512 \
  --fps 12 \
  --chunk-size 336 \
  --parity-group 4 \
  --in payload.bin \
  --out out_dir
```

### 扫描仪循环验证（sakura-storm 3 KB/s 配置文件）

在所有捕获路径中使用相同的传输配置文件：

- `chunk_size=336`
- `parity_group=4`
- `fps=12`
- `style=sakura-storm`

验证目标：- iOS：`OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
- 安卓：`OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- 浏览器/JS：`scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

验收：

- 完整有效负载重建成功，每个奇偶校验组丢弃一个数据帧。
- 正常捕获循环中没有校验和/有效负载哈希不匹配。