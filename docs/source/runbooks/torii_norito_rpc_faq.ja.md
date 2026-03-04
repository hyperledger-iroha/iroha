---
lang: ja
direction: ltr
source: docs/source/runbooks/torii_norito_rpc_faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 969f24fd53af5e5714eb4b257dc086b3990550a5085c9464805f689c7d8ee324
source_last_modified: "2025-12-14T09:53:36.247266+00:00"
translation_last_reviewed: 2025-12-28
---

# Norito-RPC 運用 FAQ

この FAQ は **NRPC-2** と **NRPC-4** に記載された rollout/rollback の
設定・テレメトリ・エビデンスを要約し、canary/brownout/インシデント
ドリル時に参照できる単一ページを提供します。オンコール引継ぎの
入口として使い、詳細手順は
`docs/source/torii/norito_rpc_rollout_plan.md` と
`docs/source/runbooks/torii_norito_rpc_canary.md` を参照してください。

## 1. 設定ノブ

| パス | 目的 | 値 / 注記 |
|------|---------|------------------------|
| `torii.transport.norito_rpc.enabled` | Norito トランスポートのハード ON/OFF。 | `true` で HTTP ハンドラを維持、`false` で stage に関係なく停止。 |
| `torii.transport.norito_rpc.require_mtls` | Norito エンドポイントの mTLS 強制。 | デフォルト `true`。隔離された staging のみで無効化。 |
| `torii.transport.norito_rpc.allowed_clients` | Norito 利用可能なサービスアカウント/API トークンの allowlist。 | CIDR ブロック、トークンハッシュ、OIDC クライアント ID を環境に合わせて指定。 |
| `torii.transport.norito_rpc.stage` | SDK に広告する rollout stage。 | `disabled`（Norito 拒否＋JSON 強制）、`canary`（allowlist のみ＋強化テレメトリ）、`ga`（全認証クライアントでデフォルト）。 |
| `torii.preauth_scheme_limits.norito_rpc` | スキーム別の同時実行 + バースト上限。 | HTTP/WS のスロットルと同じキー（`max_in_flight`, `rate_per_sec` など）を使用。Alertmanager を更新せずに上限を上げるとガードレールが崩れます。 |
| `transport.norito_rpc.*`（`docs/source/config/client_api.md`） | クライアント側の上書き（CLI/SDK discovery）。 | `cargo xtask client-api-config diff` で変更点を確認してから Torii に反映。 |

**推奨 brownout フロー**

1. `torii.transport.norito_rpc.stage=disabled` を設定。
2. `enabled=true` を維持し、probe/alert テストがハンドラを継続検証できるようにする。
3. 即時停止が必要なら `torii.preauth_scheme_limits.norito_rpc.max_in_flight` を 0 に設定。
4. 運用ログを更新し、新しい config digest を stage report に添付。

## 2. 運用チェックリスト

- **Canary / staging** — `docs/source/runbooks/torii_norito_rpc_canary.md` に従う。
  同じ設定キーを参照し、`scripts/run_norito_rpc_smoke.sh` と
  `scripts/telemetry/test_torii_norito_rpc_alerts.sh` が生成するエビデンスを列挙。
- **本番昇格** — `docs/source/torii/norito_rpc_stage_reports.md` の
  stage report テンプレートを実行。config hash、allowlist hash、smoke bundle
  digest、Grafana export hash、alert drill ID を記録。
- **Rollback** — `stage` を `disabled` に戻し、allowlist を維持したまま
  stage report とインシデントログに記録。原因修正後、canary チェックリストを
  再実行してから `stage=ga` を設定。

## 3. テレメトリとアラート

| 資産 | 場所 | 備考 |
|-------|----------|-------|
| Dashboard | `dashboards/grafana/torii_norito_rpc_observability.json` | リクエストレート、エラーコード、ペイロードサイズ、デコード失敗、採用率を追跡。 |
| Alerts | `dashboards/alerts/torii_norito_rpc_rules.yml` | `NoritoRpcErrorBudget`, `NoritoRpcDecodeFailures`, `NoritoRpcFallbackSpike` のゲート。 |
| Chaos script | `scripts/telemetry/test_torii_norito_rpc_alerts.sh` | アラート式のドリフトで CI を失敗させる。設定変更後に必ず実行。 |
| Smoke tests | `python/iroha_python/scripts/run_norito_rpc_smoke.sh`, `cargo xtask norito-rpc-verify` | 各昇格のエビデンスバンドルにログを含める。 |

ダッシュボードは必ずエクスポートし、リリースチケットに添付
（CI の `make docs-portal-dashboards`）して、オンコールが本番 Grafana なしで
メトリクスを再現できるようにします。

## 4. よくある質問

**Canary 中に新しい SDK を許可するには？**  
`torii.transport.norito_rpc.allowed_clients` にサービスアカウント/トークンを追加し、
Torii をリロード、`docs/source/torii/norito_rpc_tracker.md` の NRPC-2R に記録します。
SDK オーナーは `scripts/run_norito_rpc_fixtures.sh --sdk <label>` で fixture 実行を
記録してください。

**Norito のデコードが rollout 中に失敗した場合は？**  
`stage=canary` と `enabled=true` を維持し、`torii_norito_decode_failures_total` で
トリアージします。SDK は `Accept: application/x-norito` を外すことで JSON に戻れます。
Torii は `stage=ga` に戻るまで JSON を提供し続けます。

**Gateway が正しいマニフェストを提供していることを証明するには？**  
`scripts/telemetry/run_sorafs_gateway_probe.sh --scenario norito-rpc --host <gateway-host>` を実行し、
`Sora-Proof` ヘッダと Norito config digest を記録させます。JSON 出力を stage report に添付します。

**Redaction の一時 override はどこに記録する？**  
stage report の `Notes` 欄に記録し、Norito config パッチは変更管理に記録します。
オーバーライドは config で自動失効するため、オンコールが後始末を忘れないようにこの FAQ に明記しています。

ここにない質問は canary ランブック（`docs/source/runbooks/torii_norito_rpc_canary.md`）の
連絡チャネルでエスカレーションしてください。

## 5. リリースノート用スニペット（OPS-NRPC フォローアップ）

**OPS-NRPC** は、運用が一貫して Norito‑RPC の rollout を告知できるように
短いリリースノートを要求します。以下のブロックを次回リリース投稿に
コピーし（角括弧を置換）、記載のエビデンスを添付してください。

> **Torii Norito-RPC transport** — Norito エンベロープが JSON API と並行で提供されます。
> `torii.transport.norito_rpc.stage` は **[stage: disabled/canary/ga]** に設定され、
> `docs/source/torii/norito_rpc_rollout_plan.md` の段階的 checklist に従います。
> 一時的に無効化する場合は `torii.transport.norito_rpc.stage=disabled` を設定し、
> `torii.transport.norito_rpc.enabled=true` を維持してください。SDK は自動的に JSON に
> フォールバックします。テレメトリダッシュボード
>（`dashboards/grafana/torii_norito_rpc_observability.json`）と
> アラートドリル（`scripts/telemetry/test_torii_norito_rpc_alerts.sh`）は
> stage 引き上げ前に必須であり、`python/iroha_python/scripts/run_norito_rpc_smoke.sh`
> の canary/smoke アーティファクトをリリースチケットに添付します。

公開前の確認:

1. **[stage: …]** を Torii で広告する stage に置換。
2. リリースチケットに最新の stage report（`docs/source/torii/norito_rpc_stage_reports.md`）をリンク。
3. Grafana/Alertmanager のエクスポートと `scripts/run_norito_rpc_smoke.sh` の smoke bundle hash をアップロード。

このスニペットにより、インシデント司令が毎回言い換えずに OPS-NRPC の要件を満たせます。
