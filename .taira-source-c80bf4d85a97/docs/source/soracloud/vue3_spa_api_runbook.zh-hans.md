<!-- Auto-generated stub for Chinese (Simplified) (zh-hans) translation. Replace this content with the full translation. -->

---
lang: zh-hans
direction: ltr
source: docs/source/soracloud/vue3_spa_api_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8655a6c1d1e79d6e1c468fd6e27f7266189e47398db7dde48dced6db5c6fba72
source_last_modified: "2026-03-24T18:59:46.536363+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud Vue3 SPA + API 运行手册

本操作手册涵盖面向生产的部署和操作：

- Vue3静态站点（`--template site`）；和
- Vue3 SPA + API 服务 (`--template webapp`),

在 Iroha 3 上使用 Soracloud 控制平面 API 以及 SCR/IVM 假设（无
WASM 运行时依赖项，无 Docker 依赖项）。

## 1.生成模板项目

静态站点脚手架：

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

SPA+API脚手架：

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

每个输出目录包括：

- `container_manifest.json`
- `service_manifest.json`
- `site/` 或 `webapp/` 下的模板源文件

## 2. 构建应用程序工件

静态站点：

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

## 3. 打包并发布前端资源

对于通过 SoraFS 的静态托管：

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

对于 SPA 前端：

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. 部署到实时 Soracloud 控制平面

部署静态站点服务：

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

部署SPA+API服务：

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

验证路由绑定和推出状态：

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

预期的控制平面检查：

- `control_plane.services[].latest_revision.route_host` 套装
- `control_plane.services[].latest_revision.route_path_prefix` 集（`/` 或 `/api`）
- 升级后立即出现 `control_plane.services[].active_rollout`

## 5. 通过健康门控升级进行升级

1. 在服务清单中添加 `service_version`。
2.运行升级：

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3.健康检查后推广：

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4.如果健康失败，报告不健康：```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

当不健康报告达到策略阈值时，Soracloud 会自动滚动
返回基线修订并记录回滚审核事件。

## 6. 手动回滚和事件响应

回滚到之前的版本：

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

使用状态输出来确认：

- `current_version` 已恢复
- `audit_event_count` 递增
- `active_rollout` 已清除
- `last_rollout.stage` 是 `RolledBack` 用于自动回滚

## 7. 操作清单

- 将模板生成的清单置于版本控制之下。
- 记录每个推出步骤的 `governance_tx_hash` 以保持可追溯性。
-治疗 `service_health`、`routing`、`resource_pressure` 和
  `failed_admissions` 作为转出门输入。
- 使用金丝雀百分比和明确的促销，而不是直接全切
  面向用户的服务升级。
- 验证会话/身份验证和签名验证行为
  生产前的 `webapp/api/server.mjs`。