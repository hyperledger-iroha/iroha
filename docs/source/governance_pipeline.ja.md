---
lang: ja
direction: ltr
source: docs/source/governance_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 750f2fef72cdaf0a1160cb4f49158e4f43a11516b735a8c9d5946a524443521c
source_last_modified: "2025-12-27T09:09:07.696213+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/governance_pipeline.md -->

% Governance Pipeline (Iroha 2 and SORA Parliament)

# 現在の状態 (v1)
- ガバナンス提案は「提案者 → 国民投票 → 集計 → 発効」で進行する。国民投票のウィンドウと
  参加/承認の閾値は `gov.md` に記載のとおりに強制される。ロックは extend‑only で、
  期限切れで解除される。
- 議会選定は VRF ベースの抽選で決定論的な順序と任期境界を持つ。永続 roster がない場合、
  Torii は `gov.parliament_*` 設定からフォールバックを導出する。評議会のゲーティングと
  クオラム検証は `gov_parliament_bodies` / `gov_pipeline_sla` テストで確認される。
- 投票モード: ZK（デフォルト、inline bytes の `Active` VK が必要）と Plain（平方重み）。
  モード不一致は拒否され、ロックの作成/延長は両モードで単調。ZK と plain の再投票に
  対する回帰テストがある。
- バリデータの不正行為はエビデンスパイプライン
  (`/v1/sumeragi/evidence*`, CLI ヘルパー) を通じて処理され、
  `NextMode` + `ModeActivationHeight` により joint‑consensus ハンドオフが強制される。
- 保護名前空間、ランタイムアップグレードフック、ガバナンスマニフェストの admission は
  `governance_api.md` に記載され、テレメトリ
  (`governance_manifest_*`, `governance_protected_namespace_total`) でカバーされる。

# 進行中 / バックログ
- VRF 抽選アーティファクト（seed, proof, ordered roster, alternates）を公開し、
  不参加者の置換ルールを成文化する。抽選と置換の golden fixtures を追加する。
- 議会ボディの Stage‑SLA（rules → agenda → study → review → jury → enact）には
  明示的なタイマー、エスカレーション経路、テレメトリカウンタが必要。
- policy‑jury の秘密/commit–reveal 投票と贈収賄耐性監査は未実装。
- 役割ボンド倍率、高リスクボディへの不正スラッシング、サービススロット間の
  クールダウンは設定配線とテストが必要。
- ガバナンスレーンの封印と国民投票ウィンドウ/参加率ゲートは
  `gov.md`/`status.md` で追跡。受け入れテストが進むたびにロードマップ項目を更新する。
