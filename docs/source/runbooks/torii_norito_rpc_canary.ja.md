---
lang: ja
direction: ltr
source: docs/source/runbooks/torii_norito_rpc_canary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44aa7b618a377a4a62f3be210ce6b9ccc6906b7791c4bbf6d3f60899fe08e7b8
source_last_modified: "2025-12-14T09:53:36.245431+00:00"
translation_last_reviewed: 2025-12-28
---

# Torii Norito-RPC Canary ランブック (NRPC-2C)

このランブックは **NRPC-2** の rollout 計画を運用化し、Norito‑RPC
トランスポートを staging 検証から本番 “canary” へ昇格する手順を示します。
以下と併せて参照してください。

- [`docs/source/torii/nrpc_spec.md`](../torii/nrpc_spec.md)（プロトコル契約）
- [`docs/source/torii/norito_rpc_rollout_plan.md`](../torii/norito_rpc_rollout_plan.md)
- [`docs/source/torii/norito_rpc_telemetry.md`](../torii/norito_rpc_telemetry.md)

## 役割と入力

| 役割 | 役割責任 |
|------|----------------|
| Torii Platform TL | config 差分を承認し、smoke テストをサインオフ。 |
| NetOps | ingress/envoy 変更を適用し、canary プールの健全性を監視。 |
| Observability liaison | ダッシュボード/アラートを検証し、証跡を取得。 |
| Platform Ops | 変更チケットを主導、ロールバックリハを調整、トラッカーを更新。 |

必要なアーティファクト:

- `transport.norito_rpc.stage = "canary"` と `transport.norito_rpc.allowed_clients` を含む
  最新の `iroha_config` Norito パッチ。
- `Content-Type: application/x-norito` を保持し、canary クライアントの mTLS プロファイルを
  強制する Envoy/Nginx 設定（`defaults/torii_ingress_mtls.yaml`）。
- Canary クライアント向けのトークン allowlist（YAML または Norito マニフェスト）。
- `dashboards/grafana/torii_norito_rpc_observability.json` 用の Grafana URL + API トークン。
- パリティ smoke ハーネス（`python/iroha_python/scripts/run_norito_rpc_smoke.sh`）と
  アラートドリルスクリプト（`scripts/telemetry/test_torii_norito_rpc_alerts.sh`）へのアクセス。

## 事前チェックリスト

1. **仕様凍結の確認。** `docs/source/torii/nrpc_spec.md` の hash が最新署名済み
   リリースと一致し、Norito の header/layout に触れる PR が無いこと。
2. **設定検証。** 以下を実行して `transport.norito_rpc.*` がパースされることを確認。
   ```bash
   cargo xtask validate-config --config <patch.json> --schema client_api
   ```
3. **スキーム上限。** `torii.preauth_scheme_limits.norito_rpc` を保守的に設定
   （例: 同時接続 25）し、バイナリ呼び出しが JSON を圧迫しないようにする。
4. **Ingress リハ。** staging に Envoy パッチを適用し、負のテスト
   （`cargo test -p iroha_torii -- norito_ingress`）を実行、ヘッダ削除が
   HTTP 415 で拒否されることを確認。
5. **テレメトリ健全性。** staging で `scripts/telemetry/test_torii_norito_rpc_alerts.sh
   --env staging --dry-run` を実行し、生成された証跡バンドルを添付。
6. **トークン棚卸し。** canary allowlist に地域ごと最低 2 名が入っていることを確認し、
   `artifacts/norito_rpc/<YYYYMMDD>/allowlist.json` に保存。
7. **チケット化。** 変更チケットに開始/終了ウィンドウ、ロールバック計画、
   本ランブックとテレメトリ証跡へのリンクを記載。

## Canary 昇格手順

1. **設定パッチ適用。**
   - `iroha_config` 差分（stage=`canary`、allowlist 設定、スキーム上限）を admission 経由で適用。
   - Torii を再起動またはホットリロードし、`torii.config.reload` ログで反映を確認。
2. **Ingress 更新。**
   - Envoy/Nginx 設定をデプロイし、Norito ヘッダルーティング/mTLS を canary プールに適用。
   - `curl -vk --cert <client.pem>` の応答が Norito の `X-Iroha-Error-Code` ヘッダを
     必要時に含むことを確認。
3. **Smoke テスト。**
   - `python/iroha_python/scripts/run_norito_rpc_smoke.sh --profile canary` を canary
     bastion から実行し、JSON + Norito のトランスクリプトを
     `artifacts/norito_rpc/<YYYYMMDD>/smoke/` に保存。
   - ハッシュを `docs/source/torii/norito_rpc_stage_reports.md` に記録。
4. **テレメトリ監視。**
   - `torii_active_connections_total{scheme="norito_rpc"}` と
     `torii_request_duration_seconds_bucket{scheme="norito_rpc"}` を 30 分以上監視。
   - Grafana ダッシュボードを API でエクスポートし、変更チケットに添付。
5. **アラートリハ。**
   - `scripts/telemetry/test_torii_norito_rpc_alerts.sh --env canary` を実行し、
     不正 Norito エンベロープを注入。Alertmanager が合成インシデントを記録し
     自動解消することを確認。
6. **証跡収集。**
   - `docs/source/torii/norito_rpc_stage_reports.md` を更新し、以下を記録:
     - Config digest
     - Allowlist マニフェストの hash
     - Smoke テスト時刻
     - Grafana export の checksum
     - Alert drill ID
   - `artifacts/norito_rpc/<YYYYMMDD>/` にアーティファクトをアップロード。

## 監視と退出基準

以下が 72 時間以上満たされるまで canary を継続:

- エラーレート（`torii_request_failures_total{scheme="norito_rpc"}`）≤1 % かつ
  `torii_norito_decode_failures_total` の持続的スパイクが無い。
- レイテンシパリティ（Norito の p95 と JSON の p95 が 10 % 以内）。
- 予定ドリル以外でアラートダッシュボードが静穏。
- Allowlist にいるオペレーターがスキーマ不一致のないパリティレポートを提出。

日次ステータスは変更チケットに記録し、`docs/source/status/norito_rpc_canary_log.md`
（存在する場合）にスナップショットを残します。

## ロールバック手順

1. `transport.norito_rpc.stage` を `"disabled"` に戻し、`allowed_clients` を
   クリアして admission で適用。
2. Envoy/Nginx の route/mTLS stanza を削除し、プロキシを再読み込み、
   新規 Norito 接続が拒否されることを確認。
3. Canary トークンを失効（または bearer 認証を無効化）して既存セッションを落とす。
4. `torii_active_connections_total{scheme="norito_rpc"}` が 0 になるまで監視。
5. JSON‑only の smoke ハーネスを再実行してベースライン動作を確認。
6. 24 時間以内に `docs/source/postmortems/norito_rpc_rollback.md` にポストモーテムの
   stub を作成し、影響概要 + メトリクスを変更チケットに追記。

## Canary 後のクローズ

退出基準を満たしたら:

1. `docs/source/torii/norito_rpc_stage_reports.md` に GA 推奨を記載。
2. `status.md` に canary 結果と証跡バンドルのサマリを追加。
3. SDK リードに通知し、staging fixture を Norito に切り替えて parity を取る。
4. GA 用 config パッチ（stage=`ga`、allowlist 削除）を準備し、NRPC-2 に従って昇格。

このランブックに従うことで、canary 昇格の証跡が統一され、rollback が決定論的になり、
NRPC-2 の受け入れ条件を満たします。
