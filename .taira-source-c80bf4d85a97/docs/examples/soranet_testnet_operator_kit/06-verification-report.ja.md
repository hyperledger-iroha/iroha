---
lang: ja
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/06-verification-report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf489427d0eba2beebfdefc44092730c3963cbd77e83669853f4e9681ac9fd2d
source_last_modified: "2025-11-21T14:25:28.923348+00:00"
translation_last_reviewed: 2026-01-01
---

## オペレーター検証レポート（フェーズ T0）

- オペレーター名: ______________________
- relay descriptor ID: ______________________
- 提出日 (UTC): ___________________
- 連絡先 email / matrix: ___________________

### チェックリスト概要

| 項目 | 完了 (Y/N) | 備考 |
|------|-----------------|-------|
| ハードウェアとネットワーク検証 | | |
| compliance ブロック適用 | | |
| admission envelope 検証 | | |
| guard rotation smoke test | | |
| テレメトリ収集とダッシュボード稼働 | | |
| brownout drill 実施 | | |
| PoW チケット成功が目標内 | | |

### メトリクススナップショット

- PQ 比率 (`sorafs_orchestrator_pq_ratio`): ________
- 直近 24h の downgrade 件数: ________
- 平均回路 RTT (p95): ________ ms
- PoW 中央 solve 時間: ________ ms

### 添付資料

添付してください:

1. relay support bundle hash (`sha256`): __________________________
2. ダッシュボードのスクリーンショット (PQ 比率、回路成功、PoW ヒストグラム)。
3. 署名済み drill bundle (`drills-signed.json` + 署名者公開鍵 hex と添付物)。
4. SNNet-10 メトリクスレポート (`cargo xtask soranet-testnet-metrics --input <snapshot> --out metrics-report.json`).

### オペレーター署名

上記の情報が正確であり、必要な手順がすべて完了していることを証明します。

署名: _________________________  日付: ___________________
