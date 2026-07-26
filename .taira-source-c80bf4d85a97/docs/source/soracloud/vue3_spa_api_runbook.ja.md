<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/source/soracloud/vue3_spa_api_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8655a6c1d1e79d6e1c468fd6e27f7266189e47398db7dde48dced6db5c6fba72
source_last_modified: "2026-03-24T18:59:46.536363+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud Vue3 SPA + API ランブック

この Runbook では、次の運用指向の展開と運用について説明します。

- Vue3 静的サイト (`--template site`);そして
- Vue3 SPA + API サービス (`--template webapp`)、

SCR/IVM を前提とした Iroha 3 で Soracloud コントロール プレーン API を使用する (いいえ)
WASM ランタイム依存性はありますが、Docker 依存性はありません)。

## 1. テンプレートプロジェクトを生成する

静的サイト足場:

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

SPA + API 足場:

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

各出力ディレクトリには次のものが含まれます。

- `container_manifest.json`
- `service_manifest.json`
- `site/` または `webapp/` の下のテンプレート ソース ファイル

## 2. アプリケーション成果物をビルドする

静的サイト:

```bash
cd .soracloud-docs/site
npm install
npm run build
```

SPA フロントエンド + API:

```bash
cd .soracloud-agent/webapp
npm install
npm --prefix frontend install
npm --prefix frontend run build
```

## 3. フロントエンドアセットをパッケージ化して公開する

SoraFS 経由の静的ホスティングの場合:

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

SPA フロントエンドの場合:

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. ライブ Soracloud コントロール プレーンにデプロイする

静的サイト サービスをデプロイします。

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

SPA + API サービスを展開します。

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

ルート バインディングとロールアウト状態を検証します。

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

予想されるコントロール プレーン チェック:

- `control_plane.services[].latest_revision.route_host`セット
- `control_plane.services[].latest_revision.route_path_prefix` セット (`/` または `/api`)
- アップグレード直後に `control_plane.services[].active_rollout` が存在する

## 5. ヘルスゲート ロールアウトによるアップグレード

1. サービス マニフェストに `service_version` をバンプします。
2. アップグレードを実行します。

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3. ヘルスチェック後にロールアウトを促進します。

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4. 健全性に問題がある場合は、異常を報告します。```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

異常なレポートがポリシーのしきい値に達すると、Soracloud は自動的にロールします。
ベースライン リビジョンに戻り、ロールバック監査イベントを記録します。

## 6. 手動ロールバックとインシデント対応

以前のバージョンにロールバック:

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

ステータス出力を使用して以下を確認します。

- `current_version` を元に戻しました
- `audit_event_count` が増加しました
- `active_rollout` クリア済み
- `last_rollout.stage` は、自動ロールバックの場合は `RolledBack` です。

## 7. 操作チェックリスト

- テンプレートで生成されたマニフェストをバージョン管理下に置きます。
- トレーサビリティを維持するために、ロールアウト ステップごとに `governance_tx_hash` を記録します。
- `service_health`、`routing`、`resource_pressure`、および
  ロールアウト ゲート入力として `failed_admissions`。
- 直接フルカットではなく、カナリアパーセンテージと明示的なプロモーションを使用する
  ユーザー向けサービスのアップグレード。
- セッション/認証および署名検証の動作を検証します。
  製造前は `webapp/api/server.mjs`。