<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## オフライン QR オペレーター ランブック

この Runbook では、ノイズの多いカメラ向けの実用的な `ecc`/dimension/fps プリセットを定義しています。
オフライン QR トランスポートを使用する場合の環境。

### 推奨プリセット

|環境 |スタイル | ECC |寸法 | FPS |チャンクサイズ |パリティグループ |メモ |
| --- | --- | --- | --- | --- | --- | --- | --- |
|制御された照明、短距離 | `sakura` | `M` | `360` | `12` | `360` | `0` |最高のスループット、最小限の冗長性。 |
|モバイルカメラの典型的なノイズ | `sakura-storm` | `Q` | `512` | `12` | `336` | `4` |混合デバイスに推奨されるバランス プリセット (`~3 KB/s`)。 |
|ハイグレア、モーションブラー、ローエンドカメラ | `sakura-storm` | `H` | `640` | `8` | `280` | `6` |低いスループット、最も強力なデコード復元力。 |

### エンコード/デコードのチェックリスト

1. 明示的なトランスポートノブを使用してエンコードします。
2. ロールアウト前にスキャナー ループ キャプチャを使用して検証します。
3. プレビューの同等性を維持するために、SDK 再生ヘルパーに同じスタイル プロファイルをピン留めします。

例:

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

### スキャナーループ検証 (sakura-storm 3 KB/s プロファイル)

すべてのキャプチャ パスで同じトランスポート プロファイルを使用します。

- `chunk_size=336`
- `parity_group=4`
- `fps=12`
- `style=sakura-storm`

検証対象:- iOS: `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
- アンドロイド: `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- ブラウザ/JS: `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

受け入れ:

- 完全なペイロードの再構築は、パリティ グループごとに 1 つのデータ フレームがドロップされても成功します。
- 通常のキャプチャ ループではチェックサム/ペイロード ハッシュの不一致はありません。