---
lang: ja
direction: ltr
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T15:38:30.662574+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# レポ決済ランブック

このガイドでは、Iroha のレポ契約およびリバースレポ契約の決定的なフローについて説明します。
CLI オーケストレーション、SDK ヘルパー、および予想されるガバナンス ノブについて説明しているため、オペレーターは次のことを行うことができます。
生の Norito ペイロードを書き込むことなく、契約を開始、マージン、およびアンワインドします。ガバナンスのため
チェックリスト、証拠の収集、不正行為/ロールバック手順については、を参照してください。
[`repo_ops.md`](./repo_ops.md)、ロードマップ項目 F1 を満たします。

## CLI コマンド

`iroha app repo` コマンドは、リポジトリ固有のヘルパーをグループ化します。

```bash
# Stage an initiation instruction without submitting
iroha --config client.toml --output \
  repo initiate \
  --agreement-id daily_repo \
  --initiator soraカタカナ... \
  --counterparty soraカタカナ... \
  --custodian soraカタカナ... \
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
  --initiator soraカタカナ... \
  --counterparty soraカタカナ... \
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

* `repo initiate` および `repo unwind` は `--input/--output` を尊重するため、生成される `InstructionBox`
  ペイロードは他の CLI フローにパイプしたり、すぐに送信したりできます。
* `--custodian <account>` を渡して担保を三者保管者にルーティングします。省略した場合、
  取引相手は質権を直接受け取ります（二国間レポ）。
* `repo margin` は `FindRepoAgreements` 経由でレジャーをクエリし、次に予想されるマージンを報告します
  タイムスタンプ (ミリ秒単位) と、現在マージンコールバックの期限があるかどうかを示します。
* `repo margin-call` は `RepoMarginCallIsi` 命令を追加し、マージン チェックポイントと
  すべての参加者にイベントを発行します。一定のリズムが経過していない場合、または次の場合、通話は拒否されます。
  参加者以外から指示が提出された場合。

## Python SDK ヘルパー

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

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="soraカタカナ..."))
draft.repo_initiate(
    agreement_id="daily_repo",
    initiator="soraカタカナ...",
    counterparty="soraカタカナ...",
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

* どちらのヘルパーも、PyO3 バインディングを呼び出す前に数値とメタデータ フィールドを正規化します。
* `RepoAgreementRecord` は実行時スケジュール計算を反映するため、台帳外の自動化が可能です。
  手動でリズムを再計算することなく、コールバックの期限を決定します。

## DvP / PvP 決済

`iroha app settlement` コマンドは、配送対支払い、および支払い対支払いの指示をステージングします。

```bash
# Delivery leg first, then payment
iroha --config client.toml --output \
  settlement dvp \
  --settlement-id trade_dvp \
  --delivery-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --delivery-quantity 10 \
  --delivery-from soraカタカナ... \
  --delivery-to soraカタカナ... \
  --delivery-instrument-id US0378331005 \
  --payment-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --payment-quantity 1000 \
  --payment-from soraカタカナ... \
  --payment-to soraカタカナ... \
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
  --primary-from soraカタカナ... \
  --primary-to soraカタカナ... \
  --counter-asset 5tPkFK6s2zUcd1qUHyTmY7fDVa2n \
  --counter-quantity 460 \
  --counter-from soraカタカナ... \
  --counter-to soraカタカナ... \
  --iso-xml-out trade_pvp.xml
```

* レッグ数量は整数値または 10 進数値を受け入れ、資産の精度に対して検証されます。
* `--atomicity` は、`all-or-nothing`、`commit-first-leg`、または `commit-second-leg` を受け入れます。これらのモードを使用します
  `--order` を使用して、後続の処理が失敗した場合にコミットされたままになるレッグを表現します (`commit-first-leg`
  最初の脚を適用したままにします。 `commit-second-leg` は 2 番目を保持します)。
* 現在、CLI 呼び出しは空の命令メタデータを生成します。決済レベルで Python ヘルパーを使用する
  メタデータを添付する必要があります。
* ISO 20022 フィールド マッピングについては、[`settlement_iso_mapping.md`](./settlement_iso_mapping.md) を参照してください。
  これらの指示 (`sese.023`、`sese.025`、`colr.007`、`pacs.009`、`camt.054`) をサポートします。
* `--iso-xml-out <path>` を渡すと、CLI は Norito とともに正規の XML プレビューを出力します
  指示;ファイルは上記のマッピングに従います (DvP の場合は `sese.023`、PvP の場合は `sese.025``)。ペアリングする
  `--iso-reference-crosswalk <path>` でフラグを設定するため、CLI は `--delivery-instrument-id` を
  Torii が実行時受付中に使用するのと同じスナップショット。

Python ヘルパーは CLI サーフェスをミラーリングします。

```python
from iroha_python import (
    SettlementLeg,
    SettlementPlan,
    SettlementExecutionOrder,
    TransactionConfig,
    TransactionDraft,
)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="soraカタカナ..."))
delivery = SettlementLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="10",
    from_account="soraカタカナ...",
    to_account="soraカタカナ...",
    metadata={"isin": "ABC123"},
)
payment = SettlementLeg(
    asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
    quantity="1000",
    from_account="soraカタカナ...",
    to_account="soraカタカナ...",
)
plan = SettlementPlan(order=SettlementExecutionOrder.PAYMENT_THEN_DELIVERY)

draft.settlement_dvp("trade_dvp", delivery, payment, plan=plan, metadata={"desk": "rates"})
draft.settlement_pvp(
    "trade_pvp",
    SettlementLeg(
        asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        quantity="500",
        from_account="soraカタカナ...",
        to_account="soraカタカナ...",
    ),
    SettlementLeg(
        asset_definition_id="5tPkFK6s2zUcd1qUHyTmY7fDVa2n",
        quantity="460",
        from_account="soraカタカナ...",
        to_account="soraカタカナ...",
    ),
)
```

## 決定論とガバナンスへの期待

リポジトリ命令は、Norito でエンコードされた数値型と共有
`RepoGovernance::with_defaults` ロジック。次の不変条件に留意してください。* 数量は決定的な `NumericSpec` 値でシリアル化されます: キャッシュ レッグを使用
  `fractional(2)` (小数点以下 2 桁)、側副脚は `integer()` を使用します。提出しないでください
  より高い精度の値 - ランタイム ガードがそれらを拒否し、ピアが分岐します。
* トライパーティ リポジトリは、カストディアン アカウント ID を `RepoAgreement` に保持します。ライフサイクルとマージンイベント
  `RepoAccountRole::Custodian` ペイロードを発行して、管理者が在庫をサブスクライブして調整できるようにします。
* ヘアカットは 10000bps (100%) に固定され、マージン頻度は整数秒です。提供する
  これらの正規単位のガバナンス パラメータを実行時の期待に合わせて調整します。
* タイムスタンプは常に unix ミリ秒です。すべてのヘルパーは、それらを変更せずに Norito に転送します。
  ペイロードを使用するため、ピアは同一のスケジュールを取得します。
* 開始命令とアンワインド命令は同じアグリーメント識別子を再利用します。ランタイムが拒否する
  ID が重複し、不明な契約が解除される。 CLI/SDK ヘルパーは、これらのエラーを早期に発見します。
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` は正規のケイデンスを返します。いつも
  古いスケジュールの再実行を避けるために、コールバックをトリガーする前にこのスナップショットを参照してください。