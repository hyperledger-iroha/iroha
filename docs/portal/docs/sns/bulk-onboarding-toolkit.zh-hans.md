---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a583af55cf8b4cf5070828bfb52146be88f92937c8d7887ab37a2056bf55ec9e
source_last_modified: "2026-01-22T16:26:46.515965+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
id：批量入门工具包
标题：SNS 批量入职工具包
sidebar_label：批量入门工具包
描述：用于 SN-3b 注册器运行的 CSV 到 RegisterNameRequestV1 自动化。
---

:::注意规范来源
镜像 `docs/source/sns/bulk_onboarding_toolkit.md`，以便外部操作员看到
相同的 SN-3b 指南，无需克隆存储库。
:::

# SNS 批量入门工具包 (SN-3b)

**路线图参考：** SN-3b“批量入职工具”  
**文物：** `scripts/sns_bulk_onboard.py`、`scripts/tests/test_sns_bulk_onboard.py`、
`docs/portal/scripts/sns_bulk_release.sh`

大型注册商通常会预先准备数百个 `.sora` 或 `.nexus` 注册
具有相同的治理批准和结算规则。手动制作 JSON
负载或重新运行 CLI 无法扩展，因此 SN-3b 提供了确定性
CSV 到 Norito 构建器，用于准备 `RegisterNameRequestV1` 结构
Torii 或 CLI。助手验证前面的每一行，发出两个
聚合清单和可选的换行符分隔的 JSON，并且可以提交
自动有效负载，同时记录结构化收据以供审计。

## 1. CSV 架构

解析器需要以下标题行（顺序灵活）：

|专栏 |必填|描述 |
|--------|----------|-------------|
| `label` |是的 |请求的标签（接受混合大小写；工具根据 Norm v1 和 UTS-46 进行标准化）。 |
| `suffix_id` |是的 |数字后缀标识符（十进制或 `0x` 十六进制）。 |
| `owner` |是的 | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` |是的 |整数 `1..=255`。 |
| `payment_asset_id` |是的 |结算资产（例如 `61CtjvNd9T3THAR65GsMVHr82Bjc`）。 |
| `payment_gross` / `payment_net` |是的 |表示资产本机单位的无符号整数。 |
| `settlement_tx` |是的 |描述支付交易或哈希的 JSON 值或文字字符串。 |
| `payment_payer` |是的 |授权付款的AccountId。 |
| `payment_signature` |是的 |包含管理员或财务签名证明的 JSON 或文字字符串。 |
| `controllers` |可选|以分号或逗号分隔的控制者帐户地址列表。省略时默认为 `[owner]`。 |
| `metadata` |可选|内联 JSON 或 `@path/to/file.json` 提供解析器提示、TXT 记录等。默认为 `{}`。 |
| `governance` |可选|内联 JSON 或 `@path` 指向 `GovernanceHookV1`。 `--require-governance` 强制执行此列。 |

任何列都可以通过在单元格值前添加 `@` 来引用外部文件。
路径是相对于 CSV 文件解析的。

## 2. 运行助手

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

关键选项：

- `--require-governance` 拒绝没有治理挂钩的行（对于
  优质拍卖或保留转让）。
- `--default-controllers {owner,none}` 决定控制器单元是否为空
  回退到所有者帐户。
- `--controllers-column`、`--metadata-column` 和 `--governance-column` 重命名
  使用上游导出时的可选列。

成功后，脚本会写入聚合清单：

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "i105...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"i105...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"i105...",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```

如果提供了 `--ndjson`，则每个 `RegisterNameRequestV1` 也被写为
单行 JSON 文档，以便自动化可以将请求直接流式传输到
Torii：

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. 自动提交

### 3.1 Torii 休息模式

指定 `--submit-torii-url` 加 `--submit-token` 或
`--submit-token-file` 将每个清单条目直接推送到 Torii：

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- 帮助程序针对每个请求发出一个 `POST /v1/sns/names` 并中止
  第一个 HTTP 错误。响应以 NDJSON 形式附加到日志路径
  记录。
- `--poll-status` 在每次之后重新查询 `/v1/sns/names/{namespace}/{literal}`
  提交（最多`--poll-attempts`，默认5）以确认该记录
  可见。提供 `--suffix-map` （`suffix_id` 到 `"suffix"` 值的 JSON）
  该工具可以派生 `{label}.{suffix}` 文字进行轮询。
- 可调参数：`--submit-timeout`、`--poll-attempts` 和 `--poll-interval`。

### 3.2 iroha CLI 模式

要通过 CLI 路由每个清单条目，请提供二进制路径：

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- 控制器必须是 `Account` 条目 (`controller_type.kind = "Account"`)
  因为 CLI 目前仅公开基于帐户的控制器。
- 元数据和治理 blob 根据请求写入临时文件，
  转发至 `iroha sns register --metadata-json ... --governance-json ...`。
- 记录 CLI stdout 和 stderr 以及退出代码；非零退出代码中止
  奔跑。

两种提交模式可以一起运行（Torii 和 CLI）以交叉检查注册商
部署或演练后备方案。

### 3.3 提交回执

当提供 `--submission-log <path>` 时，脚本会附加 NDJSON 条目
捕获：

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

成功的 Torii 响应包括从以下位置提取的结构化字段
`NameRecordV1` 或 `RegisterNameResponseV1`（例如 `record_status`，
`record_pricing_class`、`record_owner`、`record_expires_at_ms`、
`registry_event_version`、`suffix_id`、`label`）等仪表板和治理
报告可以解析日志而无需检查自由格式文本。将此日志附加到
登记员票据与清单一起提供可复制的证据。

## 4. 文档门户发布自动化

CI 和门户作业调用 `docs/portal/scripts/sns_bulk_release.sh`，它包含
帮助程序并将工件存储在 `artifacts/sns/releases/<timestamp>/` 下：

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

脚本：

1. 构建 `registrations.manifest.json`、`registrations.ndjson`，并复制
   将原始 CSV 复制到发布目录中。
2. 使用 Torii 和/或 CLI（配置后）提交清单，写入
   `submissions.log` 以及上述结构化收据。
3. 发出 `summary.json` 描述该版本（路径、Torii URL、CLI 路径、
   时间戳），以便门户自动化可以将包上传到工件存储。
4. 生成 `metrics.prom`（通过 `--metrics` 覆盖），其中包含
   Prometheus-格式计数器，用于总请求、后缀分布、
   资产总额和提交结果。摘要 JSON 链接到此文件。

工作流程只是将发布目录归档为单个工件，现在
包含审计治理所需的一切。

## 5. 遥测和仪表板

`sns_bulk_release.sh` 生成的指标文件公开了以下内容
系列：

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

将 `metrics.prom` 喂入您的 Prometheus sidecar（例如通过 Promtail 或
批量导入程序）以使注册商、管理员和治理同行保持一致
批量进展。 Grafana板
`dashboards/grafana/sns_bulk_release.json` 使用面板可视化相同的数据
每个后缀计数、付款量和提交成功/失败比率。
该委员会按 `release` 进行筛选，因此审核员可以深入了解单个 CSV 运行。

## 6. 验证和失败模式

- **标签规范化：** 输入使用 Python IDNA plus 进行规范化
  小写和 Norm v1 字符过滤器。无效标签在任何标签之前都会快速失败
  网络通话。
- **数字护栏：** 后缀 ID、学期年份和定价提示必须下降
  在 `u16` 和 `u8` 范围内。付款字段接受十进制或十六进制整数
  高达 `i64::MAX`。
- **元数据或治理解析：**直接解析内联JSON；文件
  引用是相对于 CSV 位置解析的。非对象元数据
  产生验证错误。
- **控制器：** 空白单元符合 `--default-controllers`。提供明确的
  委派给非所有者时的控制器列表（例如 `i105...;i105...`）
  演员。

使用上下文行号报告失败（例如
`error: row 12 term_years must be between 1 and 255`）。脚本退出时显示
验证错误时代码为 `1`，CSV 路径丢失时代码为 `2`。

## 7. 测试和出处

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` 涵盖 CSV 解析，
  NDJSON 发射、治理执行和 CLI 或 Torii 提交
  路径。
- 助手是纯Python（没有额外的依赖）并且可以在任何地方运行
  `python3` 可用。提交历史记录与 CLI 一起跟踪
  可重复性的主存储库。

对于生产运行，请将生成的清单和 NDJSON 捆绑包附加到
注册商票证，以便管理员可以重放已提交的确切有效负载
至 Torii。