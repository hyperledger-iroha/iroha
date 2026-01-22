---
lang: ja
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/rollback_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1add138352dabf2433c36c9abac24085af8c7e53dca8bc579d73b37680e470cf
source_last_modified: "2025-11-22T04:59:33.125102+00:00"
translation_last_reviewed: 2026-01-01
---

# Relay Incentive Rollback Plan

ガバナンスが停止を求めた場合、またはテレメトリの guardrails が発火した場合に、自動 relay 支払いを無効化するための playbook です。

1. **自動化を停止。** すべての orchestrator ホストで incentives daemon を停止
   (`systemctl stop soranet-incentives.service` または同等のコンテナデプロイ) し、プロセスが動作していないことを確認。
2. **保留中の指示をドレイン。**
   `iroha app sorafs incentives service daemon --state <state.json> --config <daemon.json> --metrics-dir <spool> --once`
   を実行し、未処理の payout 指示がないことを確認。生成された Norito payloads を監査用にアーカイブ。
3. **ガバナンス承認の取り消し。** `reward_config.json` を編集し、
   `"budget_approval_id": null` を設定し、
   `iroha app sorafs incentives service init` で再デプロイ (長期稼働の daemon の場合は `update-config`)。payout エンジンは `MissingBudgetApprovalId` で fail-closed となり、新しい承認 hash が復元されるまで mint を拒否します。git commit と変更後 config の SHA-256 をインシデントログに記録。
4. **Sora Parliament に通知。** drained payout ledger、shadow-run レポート、短いインシデント概要を添付。Parliament minutes に、取り消した設定の hash と daemon 停止時刻を記載すること。
5. **Rollback 検証。** 以下を満たすまで daemon を無効化したままにする:
   - テレメトリアラート (`soranet_incentives_rules.yml`) が >=24 h グリーン,
   - トレジャリー照合レポートで欠落転送がゼロ,
   - Parliament が新しい予算 hash を承認。

ガバナンスが新しい予算承認 hash を再発行したら、`reward_config.json` を
新しい digest で更新し、最新テレメトリで `shadow-run` を再実行し、
incentives daemon を再起動する。
