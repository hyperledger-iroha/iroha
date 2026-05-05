<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: docs/source/soracloud/vue3_spa_api_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8655a6c1d1e79d6e1c468fd6e27f7266189e47398db7dde48dced6db5c6fba72
source_last_modified: "2026-03-24T18:59:46.536363+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud Vue3 SPA + API 運作手冊

本操作手冊涵蓋以生產為導向的部署和操作：

- Vue3靜態站點（`--template site`）；和
- Vue3 SPA + API 服務 (`--template webapp`),

在 Iroha 3 上使用 Soracloud 控制平面 API 以及 SCR/IVM 假設（無
WASM 運作時依賴項，無 Docker 依賴項）。

## 1.產生範本項目

靜態站點腳手架：

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

SPA+API鷹架：

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

每個輸出目錄包括：

- `container_manifest.json`
- `service_manifest.json`
- `site/` 或 `webapp/` 下的範本來源文件

## 2. 建立應用程式工件

靜態站點：

```bash
cd .soracloud-docs/site
npm install
npm run build
```

SPA前端+API：

```bash
cd .soracloud-agent/webapp
npm install
npm --prefix frontend install
npm --prefix frontend run build
```

## 3. 打包並發佈前端資源

對於透過 SoraFS 的靜態託管：

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

對於 SPA 前端：

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. 部署到即時 Soracloud 控制平面

部署靜態站點服務：

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

部署SPA+API服務：

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

驗證路由綁定和推出狀態：

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

預期的控制平面檢查：

- `control_plane.services[].latest_revision.route_host` 套裝
- `control_plane.services[].latest_revision.route_path_prefix` 集（`/` 或 `/api`）
- 升級後立即出現 `control_plane.services[].active_rollout`

## 5. 透過健康門控升級進行升級

1. 在服務清單中新增 `service_version`。
2.運行升級：

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3.健康檢查後推廣：

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4.如果健康失敗，報告不健康：```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

當不健康報告達到策略閾值時，Soracloud 會自動滾動
返回基線修訂並記錄回滾審核事件。

## 6. 手動回滾與事件回應

回滾到之前的版本：

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

使用狀態輸出來確認：

- `current_version` 已恢復
- `audit_event_count` 遞增
- `active_rollout` 已清除
- `last_rollout.stage` 是 `RolledBack` 用於自動回滾

## 7. 操作清單

- 將範本產生的清單置於版本控制之下。
- 記錄每個推出步驟的 `governance_tx_hash` 以保持可追溯性。
-治療 `service_health`、`routing`、`resource_pressure` 和
  `failed_admissions` 作為轉出門輸入。
- 使用金絲雀百分比和明確的促銷，而不是直接全切
  面向用戶的服務升級。
- 驗證會話/身份驗證和簽名驗證行為
  生產前的 `webapp/api/server.mjs`。