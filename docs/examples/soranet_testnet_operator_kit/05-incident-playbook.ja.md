---
lang: ja
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-11-04T16:28:48.303168+00:00"
translation_last_reviewed: 2026-01-01
---

# Brownout / Downgrade 対応プレイブック

1. **検知**
   - `soranet_privacy_circuit_events_total{kind="downgrade"}` アラートが発火、または governance から brownout webhook が届く。
   - 5 分以内に `kubectl logs soranet-relay` または systemd journal で確認。

2. **安定化**
   - guard rotation を停止 (`relay guard-rotation disable --ttl 30m`).
   - 影響を受けたクライアント向けに direct-only override を有効化
     (`sorafs fetch --transport-policy direct-only --write-mode read-only`).
   - 現在の compliance 設定 hash を取得 (`sha256sum compliance.toml`).

3. **診断**
   - 最新の directory snapshot と relay メトリクスバンドルを収集:
     `soranet-relay support-bundle --output /tmp/bundle.tgz`.
   - PoW キュー深度、throttling カウンタ、GAR カテゴリのスパイクを記録。
   - PQ 不足、compliance override、relay 障害のいずれが原因か特定。

4. **エスカレーション**
   - ガバナンスブリッジ (`#soranet-incident`) に概要とバンドル hash を通知。
   - アラートへのリンクを含むインシデントチケットを作成し、timestamps と緩和手順を記載。

5. **復旧**
   - 根本原因が解消したら rotation を再有効化
     (`relay guard-rotation enable`) し、direct-only overrides を戻す。
   - 30 分間 KPI を監視し、新しい brownout が出ていないことを確認。

6. **事後対応**
   - 48 時間以内に governance テンプレートでインシデント報告を提出。
   - 新しい障害モードが見つかった場合は runbooks を更新。
