---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/priority-snapshot-2025-03.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c96469964efe27ccba44af095b077980813b462c06445dadaec66eb4d284157b
source_last_modified: "2025-11-14T04:43:22.090484+00:00"
translation_last_reviewed: 2026-01-30
---

> 正規ソース: `docs/source/sorafs/priority_snapshot_2025-03.md`
>
> ステータス: **ベータ / steering ACK 待ち** (Networking, Storage, Docs leads)。

## 概要

3月の snapshot は、docs/content-network の取り組みを SoraFS 配送トラック
(SF-3, SF-6b, SF-9) に合わせたままにする。すべての leads が Nexus steering
チャンネルで snapshot を認めたら、上の “Beta” 注記を削除する。

### 重点スレッド

1. **優先度スナップショットを回覧** — acknowledgements を収集し、
   2025-03-05 の council minutes に記録する。
2. **Gateway/DNS kickoff のクローズアウト** — 2025-03-03 の workshop 前に
   新しい facilitation kit (runbook のセクション 6) をリハーサルする。
3. **オペレーター runbook の移行** — ポータル `Runbook Index` は公開済み。
   reviewer onboarding の sign-off 後に beta preview URL を公開する。
4. **SoraFS 配送スレッド** — SF-3/6b/9 の残作業を plan/roadmap に整列:
   - `sorafs-node` の PoR ingestion worker + status endpoint。
   - Rust/JS/Swift orchestrator 統合の CLI/SDK bindings の磨き込み。
   - PoR coordinator の runtime wiring と GovernanceLog イベント。

全テーブル、配布チェックリスト、ログエントリはソースファイルを参照する。
