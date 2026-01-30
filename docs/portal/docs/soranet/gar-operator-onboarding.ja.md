---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 74b0ef4843c441003cd6630f35e0deac4a736adad450270047a739c1b1d0a6fc
source_last_modified: "2025-11-21T13:08:42.404970+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: GAR オペレーター オンボーディング
sidebar_label: GAR オンボーディング
description: SNNet-9 compliance ポリシーを attestation digest と証拠収集で有効化するためのチェックリスト。
---

この brief を使って、SNNet-9 の compliance 設定を再現可能で監査向けの手順で展開する。管轄レビューと組み合わせ、すべてのオペレーターが同じ digests と証拠レイアウトを使うようにする。

## 手順

1. **設定を組み立てる**
   - `governance/compliance/soranet_opt_outs.json` を取り込む。
   - [管轄レビュー](gar-jurisdictional-review) に掲載された attestation digests と `operator_jurisdictions` をマージする。
2. **検証する**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - 任意: `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **証拠を収集する**
   - `artifacts/soranet/compliance/<YYYYMMDD>/` 配下に保存:
     - `config.json` (最終 compliance block)
     - `attestations.json` (URIs + digests)
     - 検証ログ
     - 署名済み PDF/Norito envelope への参照
4. **有効化する**
   - rollout をタグ付け (`gar-opt-out-<date>`)、orchestrator/SDK configs を再デプロイし、
     期待するログに `compliance_*` イベントが出ることを確認する。
5. **クローズする**
   - Evidence bundle を Governance Council に提出する。
   - アクティベーション期間と承認者を GAR logbook に記録する。
   - 次回レビュー日を管轄レビューの表からスケジュールする。

## クイックチェックリスト

- [ ] `jurisdiction_opt_outs` が canonical catalogue と一致している。
- [ ] Attestation digests を正確にコピーした。
- [ ] 検証コマンドを実行し、ログを保管した。
- [ ] Evidence bundle を `artifacts/soranet/compliance/<date>/` に保存した。
- [ ] Rollout タグと GAR logbook を更新した。
- [ ] 次回レビューのリマインダーを設定した。

## 参照

- [GAR Jurisdictional Review](gar-jurisdictional-review)
- [GAR Compliance Playbook (source)](../../../source/soranet/gar_compliance_playbook.md)
