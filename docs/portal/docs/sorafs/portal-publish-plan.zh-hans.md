---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/portal-publish-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd5fff7302924f71ca19593cbbcc29352c00f286ab5bc555d4654e2dc43c3daa
source_last_modified: "2026-01-22T16:26:46.525444+00:00"
translation_last_reviewed: 2026-02-07
id: portal-publish-plan
title: Docs Portal → SoraFS Publish Plan
sidebar_label: Portal Publish Plan
description: Step-by-step checklist for shipping the docs portal, OpenAPI, and SBOM bundles via SoraFS.
translator: machine-google-reviewed
---

:::注意规范来源
镜子 `docs/source/sorafs/portal_publish_plan.md`。当工作流程发生变化时更新两个副本。
:::

路线图项目 DOCS-7 需要每个文档 artefact（门户构建、OpenAPI 规范、
SBOM）流经 SoraFS 清单管道并通过 `docs.sora` 提供服务
带有 `Sora-Proof` 标头。该清单将现有的助手缝合在一起
因此 Docs/DevRel、Storage 和 Ops 可以运行版本而无需搜索
多个运行手册。

## 1. 构建和打包有效负载

运行打包助手（试运行时可以使用跳过选项）：

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- 如果 CI 已经生成了 `--skip-build`，则重用 `docs/portal/build`。
- 当 `syft` 不可用时添加 `--skip-sbom`（例如，气隙排练）。
- 该脚本运行门户测试，发出 `portal` 的 CAR + 清单对，
  `openapi`、`portal-sbom` 和 `openapi-sbom`，在以下情况下验证每个 CAR：
  设置 `--proof`，并在设置 `--sign` 时丢弃 Sigstore 捆绑包。
- 输出结构：

```json
{
  "generated_at": "2026-02-19T13:00:12Z",
  "output_dir": "artifacts/devportal/sorafs/20260219T130012Z",
  "artifacts": [
    {
      "name": "portal",
      "car": ".../portal.car",
      "plan": ".../portal.plan.json",
      "car_summary": ".../portal.car.json",
      "manifest": ".../portal.manifest.to",
      "manifest_json": ".../portal.manifest.json",
      "proof": ".../portal.proof.json",
      "bundle": ".../portal.manifest.bundle.json",
      "signature": ".../portal.manifest.sig"
    }
  ]
}
```

保留整个文件夹（或通过 `artifacts/devportal/sorafs/latest` 的符号链接）
治理审查者可以跟踪构建工件。

## 2. Pin 清单 + 别名

使用 `sorafs_cli manifest submit` 将清单推送到 Torii 并绑定别名。
将 `${SUBMITTED_EPOCH}` 设置为最新的共识纪元（从
`curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` 或您的仪表板）。

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="<katakana-i105-account-id>"
KEY_FILE="secrets/docs-admin.key"
ALIAS_PROOF="secrets/docs.alias.proof"
SUBMITTED_EPOCH="$(curl -s ${TORII_URL}/v1/status | jq '.sumeragi.epoch')"

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest="${OUT}/portal.manifest.to" \
  --chunk-plan="${OUT}/portal.plan.json" \
  --torii-url="${TORII_URL}" \
  --submitted-epoch="${SUBMITTED_EPOCH}" \
  --authority="${AUTHORITY}" \
  --private-key-file "${KEY_FILE}" \
  --alias-namespace docs \
  --alias-name portal \
  --alias-proof "${ALIAS_PROOF}" \
  --summary-out "${OUT}/portal.manifest.submit.json" \
  --response-out "${OUT}/portal.manifest.response.json"
```

- 对 `openapi.manifest.to` 和 SBOM 清单重复（省略别名标志）
  SBOM 捆绑，除非治理分配命名空间）。
- 替代方案：`iroha app sorafs pin register` 适用于提交的摘要
  摘要（如果二进制文件已安装）。
- 验证注册表状态
  `iroha app sorafs pin list --alias docs:portal --format json | jq`。
- 要观看的仪表板：`sorafs_pin_registry.json` (`torii_sorafs_replication_*`
  指标）。

## 3. 网关标头和证明

生成HTTP标头块+绑定元数据：

```bash
iroha app sorafs gateway route-plan \
  --manifest-json "${OUT}/portal.manifest.json" \
  --hostname docs.sora \
  --alias docs:portal \
  --route-label docs-portal-20260219 \
  --proof-status ok \
  --headers-out "${OUT}/portal.gateway.headers.txt" \
  --out "${OUT}/portal.gateway.plan.json"
```

- 模板包括 `Sora-Name`、`Sora-CID`、`Sora-Proof` 和
  `Sora-Proof-Status` 标头加上默认的 CSP/HSTS/Permissions-Policy。
- 使用 `--rollback-manifest-json` 渲染成对的回滚标头集。

在公开流量之前，运行：

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- 探针强制执行 GAR 签名新鲜度、别名策略和 TLS 证书
  指纹。
- 自认证线束下载带有 `sorafs_fetch` 的清单并存储
  CAR重播日志；保留审计证据的输出。

## 4. DNS 和遥测护栏

1. 刷新 DNS 框架，以便治理可以证明绑定：

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. 部署期间监控：

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   仪表板：`sorafs_gateway_observability.json`，
   `sorafs_fetch_observability.json`，以及引脚注册板。

3. 删除警报规则 (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) 并
   捕获发布存档的日志/屏幕截图。

## 5. 证据包

在发布票证或治理包中包含以下内容：

- `artifacts/devportal/sorafs/<stamp>/`（CAR、清单、SBOM、证明、
  Sigstore 捆绑包，提交摘要）。
- 网关探头+自认证输出
  （`artifacts/sorafs_gateway_probe/<stamp>/`，
  `artifacts/sorafs_gateway_self_cert/<stamp>/`）。
- DNS 骨架 + 标头模板 (`portal.gateway.headers.txt`,
  `portal.gateway.plan.json`、`portal.dns-cutover.json`）。
- 仪表板屏幕截图+警报确认。
- `status.md` 更新引用清单摘要和别名绑定时间。

遵循此清单可提供 DOCS-7：portal/OpenAPI/SBOM 有效负载是
确定性打包，用别名固定，由 `Sora-Proof` 保护
标头，并通过现有的可观察性堆栈进行端到端监控。