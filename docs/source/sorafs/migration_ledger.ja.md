---
lang: ja
direction: ltr
source: docs/source/sorafs/migration_ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0e479cae4018bbbc689fba16e2e59f93af50f1fad35509a65bce80e09e62186
source_last_modified: "2026-01-04T10:50:53.657290+00:00"
translation_last_reviewed: 2026-01-22
---

<!-- 日本語訳: docs/source/sorafs/migration_ledger.md -->

# SoraFS 移行台帳

この台帳は、SoraFS アーキテクチャ RFC に記録された移行変更ログを反映する。
エントリはマイルストーンごとにグループ化され、適用期間、影響を受けるチーム、必要なアクションを記載する。
移行計画を更新する場合は、このファイルと RFC (`docs/source/sorafs_architecture_rfc.md`) の両方を変更し、下流の利用者と整合させなければならない。

| マイルストーン | 適用期間 | 変更概要 | 影響チーム | アクション | ステータス |
|-----------|------------------|----------------|----------------|--------------|--------|
| M1 | 週 7–12 | CI が決定的な fixture を強制し、alias 証明が staging で利用可能。ツールが明示的な期待フラグを提供。 | Docs, Storage, Governance | fixture が署名済みであることを保証し、staging レジストリに alias を登録し、`--car-digest/--root-cid` の強制を含むリリースチェックリストを更新する。 | ⏳ 保留 |

これらのマイルストーンに言及するガバナンス control plane の議事録は `docs/source/sorafs/` に保存される。
重要な出来事が発生したら (例: 新しい alias 登録、レジストリ事故の振り返り)、各行の下に日付付きの箇条書きを追加し、監査可能な記録を残すこと。

## 最近の更新

- 2025-11-01 — `migration_roadmap.md` をガバナンス評議会とオペレーターの連絡網に回覧。次回評議会セッションでの承認待ち (ref: `docs/source/sorafs/council_minutes_2025-10-29.md` のフォローアップ)。
- 2025-11-02 — Pin Registry の register ISI が `sorafs_manifest` ヘルパーを通じて共有 chunker/ポリシー検証を強制し、オンチェーンのパスを Torii チェックと整合させるようになった。
- 2026-02-13 — プロバイダー広告のロールアウトフェーズ (R0–R3) を台帳へ追加し、関連ダッシュボードとオペレーターガイダンスを公開 (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`)。
