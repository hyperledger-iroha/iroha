<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## 離線 QR 操作手冊

此操作手冊定義了實用的 `ecc`/dimension/fps 預設，以應對相機噪音
使用離線 QR 傳輸時的環境。

### 推薦預設

|環境 |風格| ECC |尺寸|第一人稱射擊 |塊大小 |奇偶組|筆記|
| ---| --- | ---| --- | ---| ---| --- | --- |
|短距離受控照明 | `sakura` | `M` | `360` | `12` | `360` | `0` | |
|典型的行動相機噪音| `sakura-storm` | `Q` | `512` | `12` | `336` |NI00500000013014100012X | |
|高眩光、運動模糊、低階相機 | `sakura-storm` | `H` | `640` | `8` | `280` | `sakura-storm` | `280` | ```bash
iroha offline qr encode \
  --style sakura-storm \
  --ecc Q \
  --dimension 512 \
  --fps 12 \
  --chunk-size 336 \
  --parity-group 4 \
  --in payload.bin \
  --out out_dir
``` |

### 編碼/解碼清單

1. 使用明確傳輸旋鈕進行編碼。
2. 在推出之前使用掃描器循環擷取進行驗證。
3. 在 SDK 播放助理中固定相同的樣式設定文件，以保持預覽奇偶性。

範例：

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

### 掃描器循環驗證（sakura-storm 3 KB/s 設定檔）

在所有捕獲路徑中使用相同的傳輸設定檔：

- `chunk_size=336`
- `parity_group=4`
- `fps=12`
- `style=sakura-storm`

驗證目標：- iOS：`OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
- 安卓：`OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- 瀏覽器/JS：`scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

驗收：

- 完整有效負載重建成功，每個奇偶校驗組丟棄一個資料幀。
- 正常捕獲循環中沒有校驗和/有效負載雜湊不匹配。