<!-- Japanese translation of docs/source/generated/sumeragi_da_report.md -->

---
lang: ja
direction: ltr
source: docs/source/generated/sumeragi_da_report.md
status: needs-update
translator: manual
---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to `docs/source/generated/sumeragi_da_report.md` for current semantics.

# Sumeragi データ可用性レポート

`artifacts/sumeragi-da/20251005T190335Z` から 3 件のサマリーファイルを処理しました。

## サマリー

| シナリオ | 実行数 | ピア数 | ペイロード (MiB) | RBC deliver 中央値 (ms) | RBC deliver 最大値 (ms) | Commit 中央値 (ms) | Commit 最大値 (ms) | スループット中央値 (MiB/s) | スループット最小値 (MiB/s) | RBC≤Commit | BG キュー最大値 | P2P ドロップ最大値 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| sumeragi_rbc_da_large_payload_four_peers | 1 | 4 | 10.50 | 3120 | 3120 | 3380 | 3380 | 3.28 | 3.28 | yes | 18 | 0 |
| sumeragi_rbc_da_large_payload_six_peers | 1 | 6 | 10.50 | 3280 | 3280 | 3560 | 3560 | 3.21 | 3.21 | yes | 24 | 0 |
| sumeragi_rbc_recovers_after_peer_restart | 1 | 4 | 10.50 | 3340 | 3340 | 3620 | 3620 | 3.16 | 3.16 | yes | 19 | 0 |

### sumeragi_rbc_da_large_payload_four_peers

- 実行数: 1
- ピア数: 4
- ペイロード: 11010048 バイト (10.50 MiB)
- 観測された RBC チャンク数: 168
- READY 投票数: 4
- RBC deliver が Commit を保護: yes
- RBC deliver 平均 (ms): 3120.00
- Commit 平均 (ms): 3380.00
- スループット平均 (MiB/s): 3.28
- BG 投稿キュー深さ 最大/中央値: 18 / 18
- P2P キュードロップ 最大/中央値: 0 / 0
- ピアごとのペイロードバイト: 11010048 - 11010048
- ピアごとの deliver ブロードキャスト: 1 - 1
- ピアごとの READY ブロードキャスト: 1 - 1

| 実行 | ソース | ブロック | 高さ | View | RBC deliver (ms) | Commit (ms) | スループット (MiB/s) | RBC≤Commit | READY | 総チャンク数 | 受信数 | BG キュー最大値 | P2P ドロップ |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_four_peers.summary.json | 0x8f2fd8804f... | 58 | 2 | 3120 | 3380 | 3.28 | yes | 4 | 168 | 168 | 18 | 0 |

### sumeragi_rbc_da_large_payload_six_peers

- 実行数: 1
- ピア数: 6
- ペイロード: 11010048 バイト (10.50 MiB)
- 観測された RBC チャンク数: 168
- READY 投票数: 5
- RBC deliver が Commit を保護: yes
- RBC deliver 平均 (ms): 3280.00
- Commit 平均 (ms): 3560.00
- スループット平均 (MiB/s): 3.21
- BG 投稿キュー深さ 最大/中央値: 24 / 24
- P2P キュードロップ 最大/中央値: 0 / 0
- ピアごとのペイロードバイト: 11010048 - 11010048
- ピアごとの deliver ブロードキャスト: 1 - 1
- ピアごとの READY ブロードキャスト: 1 - 1

| 実行 | ソース | ブロック | 高さ | View | RBC deliver (ms) | Commit (ms) | スループット (MiB/s) | RBC≤Commit | READY | 総チャンク数 | 受信数 | BG キュー最大値 | P2P ドロップ |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_six_peers.summary.json | 0x71e3cdebcf... | 59 | 3 | 3280 | 3560 | 3.21 | yes | 5 | 168 | 168 | 24 | 0 |

### sumeragi_rbc_recovers_after_peer_restart

- 実行数: 1
- ピア数: 4
- ペイロード: 11010048 バイト (10.50 MiB)
- 観測された RBC チャンク数: 168
- READY 投票数: 4
- RBC deliver が Commit を保護: yes
- RBC deliver 平均 (ms): 3340.00
- Commit 平均 (ms): 3620.00
- スループット平均 (MiB/s): 3.16
- BG 投稿キュー深さ 最大/中央値: 19 / 19
- P2P キュードロップ 最大/中央値: 0 / 0
- ピアごとのペイロードバイト: 11010048 - 11010048
- ピアごとの deliver ブロードキャスト: 1 - 1
- ピアごとの READY ブロードキャスト: 1 - 1

| 実行 | ソース | ブロック | 高さ | View | RBC deliver (ms) | Commit (ms) | スループット (MiB/s) | RBC≤Commit | READY | 総チャンク数 | 受信数 | BG キュー最大値 | P2P ドロップ |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_recovers_after_peer_restart.summary.json | 0xeaf2198957... | 60 | 3 | 3340 | 3620 | 3.16 | yes | 4 | 168 | 168 | 19 | 0 |
