---
lang: zh-hans
direction: ltr
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T16:26:46.568155+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 回购结算操作手册

本指南记录了 Iroha 中回购协议和逆回购协议的确定性流程。
它涵盖 CLI 编排、SDK 帮助程序以及预期的治理旋钮，以便操作员能够
无需编写原始 Norito 有效负载即可启动、保证金和解除协议。用于治理
检查表、证据捕获和欺诈/回滚程序请参阅
[`repo_ops.md`](./repo_ops.md)，满足路线图项 F1。

## CLI 命令

`iroha app repo` 命令对特定于存储库的帮助程序进行分组：

```bash
# Stage an initiation instruction without submitting
iroha --config client.toml --output \
  repo initiate \
  --agreement-id daily_repo \
  --initiator <i105-account-id> \
  --counterparty <i105-account-id> \
  --custodian <i105-account-id> \
  --cash-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --cash-quantity 1000 \
  --collateral-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --collateral-quantity 1050 \
  --rate-bps 250 \
  --maturity-timestamp-ms 1704000000000 \
  --haircut-bps 1500 \
  --margin-frequency-secs 86400

# Generate the unwind leg
iroha --config client.toml --output \
  repo unwind \
  --agreement-id daily_repo \
  --initiator <i105-account-id> \
  --counterparty <i105-account-id> \
  --cash-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --cash-quantity 1005 \
  --collateral-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --collateral-quantity 1055 \
  --settlement-timestamp-ms 1704086400000

# Inspect the next margin checkpoint for an active agreement
iroha --config client.toml repo margin --agreement-id daily_repo

# Trigger a margin call when cadence elapses
iroha --config client.toml repo margin-call --agreement-id daily_repo
```

* `repo initiate` 和 `repo unwind` 尊重 `--input/--output` 因此生成的 `InstructionBox`
  有效负载可以通过管道传输到其他 CLI 流中或立即提交。
* 通过 `--custodian <account>` 将抵押品传送给三方托管人。当省略时，
  交易对手直接收到质押（双边回购）。
* `repo margin` 通过 `FindRepoAgreements` 查询账本并报告下一个预期保证金
  时间戳（以毫秒为单位）以及当前是否到期保证金回调。
* `repo margin-call`附加一条`RepoMarginCallIsi`指令，记录margin检查点和
  为所有参与者发出事件。如果节奏尚未过去或如果
  指示由非参与者提交。

## Python SDK 帮助器

```python
from iroha_python import (
    create_torii_client,
    RepoAgreementRecord,
    RepoCashLeg,
    RepoCollateralLeg,
    RepoGovernance,
    TransactionConfig,
    TransactionDraft,
)

client = create_torii_client("client.toml")

cash = RepoCashLeg(asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1", quantity="1000")
collateral = RepoCollateralLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="1050",
    metadata={"isin": "ABC123"},
)
governance = RepoGovernance(haircut_bps=1500, margin_frequency_secs=86_400)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="<i105-account-id>"))
draft.repo_initiate(
    agreement_id="daily_repo",
    initiator="<i105-account-id>",
    counterparty="<i105-account-id>",
    cash_leg=cash,
    collateral_leg=collateral,
    rate_bps=250,
    maturity_timestamp_ms=1_704_000_000_000,
    governance=governance,
)
# ... additional instructions ...
envelope = draft.sign_with_keypair(my_keypair)
client.submit_transaction_envelope(envelope)

# Margin schedule
agreements = client.list_repo_agreements()
record = RepoAgreementRecord.from_payload(agreements[0])
next_margin = record.next_margin_check_after(at_timestamp_ms=now_ms)
```

* 两个帮助器在调用 PyO3 绑定之前都会标准化数字数量和元数据字段。
* `RepoAgreementRecord` 镜像运行时计划计算，因此账外自动化可以
  确定回调何时到期，而无需手动重新计算节奏。

## DvP / PvP 结算

`iroha app settlement` 命令阶段交付与付款和付款与付款指令：

```bash
# Delivery leg first, then payment
iroha --config client.toml --output \
  settlement dvp \
  --settlement-id trade_dvp \
  --delivery-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --delivery-quantity 10 \
  --delivery-from <i105-account-id> \
  --delivery-to <i105-account-id> \
  --delivery-instrument-id US0378331005 \
  --payment-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --payment-quantity 1000 \
  --payment-from <i105-account-id> \
  --payment-to <i105-account-id> \
  --order payment-then-delivery \
  --atomicity all-or-nothing \
  --iso-reference-crosswalk /opt/iso/isin_crosswalk.json \
  --iso-xml-out trade_dvp.xml

# Cross-currency swap (payment-versus-payment)
iroha --config client.toml --output \
  settlement pvp \
  --settlement-id trade_pvp \
  --primary-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --primary-quantity 500 \
  --primary-from <i105-account-id> \
  --primary-to <i105-account-id> \
  --counter-asset 5tPkFK6s2zUcd1qUHyTmY7fDVa2n \
  --counter-quantity 460 \
  --counter-from <i105-account-id> \
  --counter-to <i105-account-id> \
  --iso-xml-out trade_pvp.xml
```

* 支线数量接受整数或小数值，并根据资产精度进行验证。
* `--atomicity` 接受 `all-or-nothing`、`commit-first-leg` 或 `commit-second-leg`。使用这些模式
  使用 `--order` 来表示如果后续处理失败，哪个分支仍保持提交（`commit-first-leg`
  保持第一条腿处于应用状态； `commit-second-leg` 保留第二个）。
* CLI 调用今天发出空指令元数据；在结算级别时使用Python助手
  需要附加元数据。
* 请参阅 [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) 了解 ISO 20022 字段映射
  支持这些指令（`sese.023`、`sese.025`、`colr.007`、`pacs.009`、`camt.054`）。
* 传递 `--iso-xml-out <path>` 以使 CLI 与 Norito 一起发出规范的 XML 预览
  指示；该文件遵循上面的映射（DvP 为 `sese.023`，PvP`为 `sese.025`）。配对
  带有 `--iso-reference-crosswalk <path>` 标记，以便 CLI 对照 `--delivery-instrument-id` 进行验证
  Torii 在运行时准入期间使用相同的快照。

Python 助手镜像 CLI 界面：

```python
from iroha_python import (
    SettlementLeg,
    SettlementPlan,
    SettlementExecutionOrder,
    TransactionConfig,
    TransactionDraft,
)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="<i105-account-id>"))
delivery = SettlementLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="10",
    from_account="<i105-account-id>",
    to_account="<i105-account-id>",
    metadata={"isin": "ABC123"},
)
payment = SettlementLeg(
    asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
    quantity="1000",
    from_account="<i105-account-id>",
    to_account="<i105-account-id>",
)
plan = SettlementPlan(order=SettlementExecutionOrder.PAYMENT_THEN_DELIVERY)

draft.settlement_dvp("trade_dvp", delivery, payment, plan=plan, metadata={"desk": "rates"})
draft.settlement_pvp(
    "trade_pvp",
    SettlementLeg(
        asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        quantity="500",
        from_account="<i105-account-id>",
        to_account="<i105-account-id>",
    ),
    SettlementLeg(
        asset_definition_id="5tPkFK6s2zUcd1qUHyTmY7fDVa2n",
        quantity="460",
        from_account="<i105-account-id>",
        to_account="<i105-account-id>",
    ),
)
```

## 决定论和治理期望

Repo 指令完全依赖于 Norito 编码的数字类型和共享的
`RepoGovernance::with_defaults` 逻辑。请记住以下不变量：* 数量按确定性 `NumericSpec` 值进行序列化：现金支线使用
  `fractional(2)`（两位小数），侧支腿使用 `integer()`。不提交
  更精确的值——运行时守卫会拒绝它们，并且同行会产生分歧。
* 三方存储库将托管账户 ID 保留在 `RepoAgreement` 中。生命周期和保证金事件
  发出 `RepoAccountRole::Custodian` 有效负载，以便保管人可以订阅和协调库存。
* 理发率被限制为 10000bps (100%)，边际频率为整秒。提供
  这些规范单元中的治理参数与运行时期望保持一致。
* 时间戳始终为 unix 毫秒。所有助手将其原样转发至 Norito
  有效负载，以便对等方得出相同的时间表。
* 启动和展开指令重用相同的协议标识符。运行时拒绝
  重复 ID 并解除未知协议； CLI/SDK 帮助程序会尽早发现这些错误。
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` 返回规范节奏。总是
  在触发回调之前查阅此快照以避免重播过时的计划。