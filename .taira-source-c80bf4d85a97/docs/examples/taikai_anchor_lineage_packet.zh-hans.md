---
lang: zh-hans
direction: ltr
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-12-29T18:16:35.095373+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Taikai Anchor Lineage 数据包模板 (SN13-C)

路线图项目 **SN13-C — 清单和 SoraNS 锚点** 需要每个别名
轮换以发送确定性证据包。将此模板复制到您的
推出工件目录（例如
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) 并替换
将数据包提交给治理之前的占位符。

## 1. 元数据

|领域|价值|
|--------|--------|
|事件 ID | `<taikai.event.launch-2026-07-10>` |
|流媒体/演绎| `<main-stage>` |
|别名命名空间/名称 | `<sora / docs>` |
|证据目录| `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
|运营商联系方式 | `<name + email>` |
| GAR / RPT 机票 | `<governance ticket or GAR digest>` |

## 捆绑助手（可选）

复制 spool 工件并在之前发出 JSON（可选签名）摘要
填写剩余部分：

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

助手拉`taikai-anchor-request-*`，`taikai-trm-state-*`，
`taikai-lineage-*`、Taikai 假脱机目录中的信封和哨兵
（`config.da_ingest.manifest_store_dir/taikai`）所以证据文件夹已经
包含下面引用的确切文件。

## 2. 血统账本和提示

附上磁盘上的沿袭分类帐和为此编写的提示 JSON Torii
窗口。这些直接来自
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` 和
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`。

|文物|文件| SHA-256 |笔记|
|----------|------|---------|--------|
|血统分类账 | `taikai-trm-state-docs.json` | `<sha256>` |证明先前的清单摘要/窗口。 |
|血统提示 | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` |在上传到 SoraNS 锚点之前捕获。 |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. 锚点有效负载捕获

记录 Torii 传递给锚点服务的 POST 负载。有效载荷
包括 `envelope_base64`、`ssm_base64`、`trm_base64` 和内联
`lineage_hint` 对象；审计依靠此捕获来证明所暗示的内容
发送至 SoraNS。 Torii 现在自动将此 JSON 写入为
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
在 Taikai spool 目录（`config.da_ingest.manifest_store_dir/taikai/`）内，所以
操作员可以直接复制它，而不是抓取 HTTP 日志。

|文物|文件| SHA-256 |笔记|
|----------|------|---------|--------|
|锚定帖子 | `requests/2026-07-10T18-00Z.json` | `<sha256>` |从 `taikai-anchor-request-*.json`（Taikai 线轴）复制的原始请求。 |

## 4. 清单摘要确认

|领域|价值|
|--------|--------|
|新清单摘要 | `<hex digest>` |
|先前的清单摘要（来自提示）| `<hex digest>` |
|窗口开始/结束 | `<start seq> / <end seq>` |
|接受时间戳| `<ISO8601>` |

参考上面记录的账本/提示哈希值，以便审核者可以验证
被取代的窗口。

## 5. 指标 / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` 快照：`<Prometheus query + export path>`
- `/status taikai_alias_rotations` 转储（每个别名）：`<file path + hash>`

提供显示计数器的 Prometheus/Grafana 导出或 `curl` 输出
增量和此别名的 `/status` 数组。

## 6. 证据目录清单

生成证据目录的确定性清单（假脱机文件，
有效负载捕获、指标快照），因此治理可以验证每个哈希，而无需
解压存档。

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

|文物|文件| SHA-256 |笔记|
|----------|------|---------|--------|
|证据清单 | `manifest.json` | `<sha256>` |将此附加到治理数据包/GAR。 |

## 7. 清单

- [ ] 谱系分类帐复制 + 散列。
- [ ] 沿袭提示已复制+散列。
- [ ] 捕获并散列锚点 POST 有效负载。
- [ ] 已填写清单摘要表。
- [ ] 导出的指标快照（`taikai_trm_alias_rotations_total`、`/status`）。
- [ ] 使用 `scripts/repo_evidence_manifest.py` 生成的清单。
- [ ] 数据包上传至治理，包含哈希值 + 联系信息。

为每个别名轮换维护此模板可以保持 SoraNS 治理
捆绑可重复性并将谱系提示直接与 GAR/RPT 证据联系起来。