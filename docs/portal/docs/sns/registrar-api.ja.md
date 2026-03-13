---
lang: ja
direction: ltr
source: docs/portal/docs/sns/registrar-api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 14373f09e9a691a910fbde08548c5fdfe03581049c85af425c27c94fc4fcafd8
source_last_modified: "2025-11-10T20:06:34.010395+00:00"
translation_last_reviewed: 2026-01-01
---

:::note 正規ソース
このページは `docs/source/sns/registrar_api.md` を反映しており、
現在はポータルの正規コピーとして扱われます。翻訳PRのためにソース
ファイルは維持されます。
:::

# SNS Registrar API とガバナンス・フック (SN-2b)

**状態:** 2026-03-24 起草 - Nexus Core レビュー中  
**ロードマップリンク:** SN-2b "Registrar API & governance hooks"  
**前提条件:** スキーマ定義は [`registry-schema.md`](./registry-schema.md)

このノートは、Sora Name Service (SNS) registrar を運用するために必要な
Torii エンドポイント、gRPC サービス、リクエスト/レスポンス DTO、
ガバナンスアーティファクトを規定します。登録、更新、管理が必要な
SDK、ウォレット、オートメーション向けの権威ある契約です。

## 1. トランスポートと認証

| 要件 | 詳細 |
|------|------|
| プロトコル | REST は `/v2/sns/*`、gRPC サービスは `sns.v1.Registrar`。どちらも Norito-JSON (`application/json`) と Norito-RPC バイナリ (`application/x-norito`) を受け付けます。 |
| Auth | `Authorization: Bearer` トークンまたはサフィックス steward 発行の mTLS 証明書。ガバナンスに敏感なエンドポイント (freeze/unfreeze, 予約割当て) は `scope=sns.admin` が必要です。 |
| レート制限 | registrar は JSON 呼び出しと同じ `torii.preauth_scheme_limits` バケットを共有し、さらにサフィックス別のバースト上限: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`。 |
| テレメトリ | Torii は registrar ハンドラ向けに `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` を公開 ( `scheme="norito_rpc"` でフィルタ ); API も `sns_registrar_status_total{result, suffix_id}` を増加させます。 |

## 2. DTO 概要

フィールドは [`registry-schema.md`](./registry-schema.md) に定義された正規 struct を参照します。全ペイロードに `NameSelectorV1` + `SuffixId` を埋め込み、曖昧なルーティングを回避します。

```text
Struct RegisterNameRequestV1 {
    selector: NameSelectorV1,
    owner: AccountId,
    controllers: Vec<NameControllerV1>,
    term_years: u8,                     // 1..=max_term_years
    pricing_class_hint: Option<u8>,     // steward-advertised tier id
    payment: PaymentProofV1,
    governance: GovernanceHookV1,
    metadata: Metadata,
}

Struct RegisterNameResponseV1 {
    name_record: NameRecordV1,
    registry_event: RegistryEventV1,
    revenue_accrual: RevenueAccrualEventV1,
}

Struct PaymentProofV1 {
    asset_id: AssetId,
    gross_amount: TokenValue,
    net_amount: TokenValue,
    settlement_tx: Hash,
    payer: AccountId,
    signature: Signature,               // steward/treasury cosign
}

Struct GovernanceHookV1 {
    proposal_id: String,
    council_vote_hash: Hash,
    dao_vote_hash: Hash,
    steward_ack: Signature,
    guardian_clearance: Option<Signature>,
}

Struct RenewNameRequestV1 {
    selector: NameSelectorV1,
    term_years: u8,
    payment: PaymentProofV1,
}

Struct TransferNameRequestV1 {
    selector: NameSelectorV1,
    new_owner: AccountId,
    governance: GovernanceHookV1,
}

Struct UpdateControllersRequestV1 {
    selector: NameSelectorV1,
    controllers: Vec<NameControllerV1>,
}

Struct FreezeNameRequestV1 {
    selector: NameSelectorV1,
    reason: String,
    until: Timestamp,
    guardian_ticket: Signature,
}

Struct ReservedAssignmentRequestV1 {
    selector: NameSelectorV1,
    reserved_label: ReservedNameV1,
    governance: GovernanceHookV1,
}
```

## 3. REST エンドポイント

| エンドポイント | メソッド | ペイロード | 説明 |
|----------------|----------|------------|------|
| `/v2/sns/registrations` | POST | `RegisterNameRequestV1` | 名前の登録または再開。価格 tier を解決し、支払い/ガバナンス証明を検証し、レジストリエベントを発行。 |
| `/v2/sns/registrations/{selector}/renew` | POST | `RenewNameRequestV1` | 期間を延長。ポリシーの猶予/救済ウィンドウを適用。 |
| `/v2/sns/registrations/{selector}/transfer` | POST | `TransferNameRequestV1` | ガバナンス承認が付与されたら所有権を移転。 |
| `/v2/sns/registrations/{selector}/controllers` | PUT | `UpdateControllersRequestV1` | controller 集合を置換; 署名済みアカウントアドレスを検証。 |
| `/v2/sns/registrations/{selector}/freeze` | POST | `FreezeNameRequestV1` | guardian/council フリーズ。guardian チケットとガバナンス台帳への参照が必要。 |
| `/v2/sns/registrations/{selector}/freeze` | DELETE | `GovernanceHookV1` | 修復後に解除; council override が記録されていることを確認。 |
| `/v2/sns/reserved/{selector}` | POST | `ReservedAssignmentRequestV1` | steward/council による予約名の割当。 |
| `/v2/sns/policies/{suffix_id}` | GET | -- | 現在の `SuffixPolicyV1` を取得 (キャッシュ可能)。 |
| `/v2/sns/registrations/{selector}` | GET | -- | 現在の `NameRecordV1` と有効状態 (Active, Grace など) を返す。 |

**セレクタのエンコード:** `{selector}` パスセグメントは ADDR-5 に従う I105/圧縮/正規 hex を受け付け、Torii が `NameSelectorV1` で正規化します。

**エラーモデル:** 全エンドポイントは Norito JSON で `code`, `message`, `details` を返します。コードは `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing` を含みます。

### 3.1 CLI ヘルパー (N0 手動 registrar 要件)

クローズドベータの steward は、JSON を手作業で作らずに CLI で registrar を操作できます:

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id xor#sora \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` は CLI 設定アカウントが既定; `--controller` を繰り返して追加 controller アカウントを付与します (既定 `[owner]`).
- インライン支払いフラグは `PaymentProofV1` に直接対応; 既に構造化されたレシートがある場合は `--payment-json PATH` を渡します。メタデータ (`--metadata-json`) とガバナンスフック (`--governance-json`) も同様です。

読み取り専用ヘルパーでリハーサルを補完:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

実装は `crates/iroha_cli/src/commands/sns.rs` を参照。コマンドは本書で説明した Norito DTO を再利用し、CLI 出力が Torii 応答とバイト単位で一致するようにします。

追加のヘルパーで更新、移転、guardian アクションをカバー:

```bash
# Renew an expiring name
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id xor#sora \
  --payment-gross 120 \
  --payment-settlement '"renewal-settlement"' \
  --payment-signature '"steward-signature"'

# Transfer ownership once governance approves
iroha sns transfer \
  --selector makoto.sora \
  --new-owner i105... \
  --governance-json /path/to/hook.json

# Freeze/unfreeze flows
iroha sns freeze \
  --selector makoto.sora \
  --reason "guardian investigation" \
  --until-ms 1750000000000 \
  --guardian-ticket '{"sig":"guardian"}'

iroha sns unfreeze \
  --selector makoto.sora \
  --governance-json /path/to/unfreeze_hook.json
```

`--governance-json` には有効な `GovernanceHookV1` レコード (proposal id, vote hashes, steward/guardian 署名) が必要です。各コマンドは対応する `/v2/sns/registrations/{selector}/...` エンドポイントをそのまま反映するため、ベータオペレーターは SDK が呼び出す Torii サーフェスを正確にリハーサルできます。

## 4. gRPC サービス

```text
service Registrar {
    rpc Register(RegisterNameRequestV1) returns (RegisterNameResponseV1);
    rpc Renew(RenewNameRequestV1) returns (NameRecordV1);
    rpc Transfer(TransferNameRequestV1) returns (NameRecordV1);
    rpc UpdateControllers(UpdateControllersRequestV1) returns (NameRecordV1);
    rpc Freeze(FreezeNameRequestV1) returns (NameRecordV1);
    rpc Unfreeze(GovernanceHookV1) returns (NameRecordV1);
    rpc AssignReserved(ReservedAssignmentRequestV1) returns (NameRecordV1);
    rpc GetRegistration(NameSelectorV1) returns (NameRecordV1);
    rpc GetPolicy(SuffixId) returns (SuffixPolicyV1);
}
```

Wire-format: コンパイル時 Norito スキーマハッシュが
`fixtures/norito_rpc/schema_hashes.json` (行 `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, など) に記録されます。

## 5. ガバナンスフックと証拠

状態を変更する呼び出しは、再生可能な証拠を添付する必要があります:

| アクション | 必要なガバナンスデータ |
|------------|------------------------|
| 標準登録/更新 | 決済指示を参照する支払い証明; tier が steward 承認を要求しない限り council 票は不要。 |
| プレミアム tier 登録 / 予約割当て | `GovernanceHookV1` が proposal id と steward acknowledgement を参照。 |
| 移転 | council vote hash + DAO signal hash; 争議解決での移転時は guardian clearance。 |
| Freeze/Unfreeze | guardian チケット署名 + council override (解除時)。 |

Torii は以下を確認して証明を検証します:

1. proposal id がガバナンス台帳 (`/v2/governance/proposals/{id}`) に存在し、状態が `Approved`。
2. hash が記録された投票アーティファクトと一致。
3. steward/guardian 署名が `SuffixPolicyV1` の期待公開鍵を参照。

検証失敗は `sns_err_governance_missing` を返します。

## 6. ワークフロー例

### 6.1 標準登録

1. クライアントは `/v2/sns/policies/{suffix_id}` を取得し、価格、猶予、利用可能な tier を確認。
2. クライアントは `RegisterNameRequestV1` を構築:
   - `selector` は I105（推奨）/圧縮（次善）ラベルから導出。
   - `term_years` はポリシー範囲内。
   - `payment` は treasury/steward splitter 送金を参照。
3. Torii が検証:
   - ラベル正規化 + 予約リスト。
   - Term/gross price vs `PriceTierV1`.
   - 支払い証明 amount >= 計算価格 + 手数料。
4. 成功時 Torii:
   - `NameRecordV1` を保存。
   - `RegistryEventV1::NameRegistered` を発行。
   - `RevenueAccrualEventV1` を発行。
   - 新しいレコード + イベントを返却。

### 6.2 猶予期間中の更新

猶予期間中の更新には、標準リクエストに加えてペナルティ検知が含まれます:

- Torii は `now` と `grace_expires_at` を比較し、`SuffixPolicyV1` のサーチャージ表を追加。
- 支払い証明はサーチャージをカバーする必要があります。失敗 => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` は新しい `expires_at` を記録。

### 6.3 guardian フリーズと council オーバーライド

1. guardian が incident id を参照するチケット付きで `FreezeNameRequestV1` を提出。
2. Torii はレコードを `NameStatus::Frozen` に移動し、`NameFrozen` を発行。
3. 修復後、council が override を発行; オペレーターは `GovernanceHookV1` を付けて DELETE `/v2/sns/registrations/{selector}/freeze` を送信。
4. Torii が override を検証し、`NameUnfrozen` を発行。

## 7. 検証とエラーコード

| コード | 説明 | HTTP |
|--------|------|------|
| `sns_err_reserved` | ラベルが予約またはブロックされています。 | 409 |
| `sns_err_policy_violation` | 期間、tier、controller 集合がポリシーに違反。 | 422 |
| `sns_err_payment_mismatch` | 支払い証明の値または資産が不一致。 | 402 |
| `sns_err_governance_missing` | 必要なガバナンスアーティファクトが欠落/無効。 | 403 |
| `sns_err_state_conflict` | 現在のライフサイクル状態で操作不可。 | 409 |

全コードは `X-Iroha-Error-Code` と構造化 Norito JSON/NRPC エンベロープで表面化します。

## 8. 実装ノート

- Torii は `NameRecordV1.auction` に保留中オークションを保存し、`PendingAuction` の間は直接登録を拒否します。
- 支払い証明は Norito 台帳レシートを再利用; treasury サービスがヘルパー API (`/v2/finance/sns/payments`) を提供。
- SDK はこれらエンドポイントを強い型のヘルパーでラップし、ウォレットが明確なエラー理由 (`ERR_SNS_RESERVED` など) を提示できるようにします。

## 9. 次のステップ

- SN-3 オークションが入ったら Torii ハンドラを実際のレジストリ契約に接続。
- Rust/JS/Swift の SDK ガイドを公開し、この API を参照。
- [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) を拡張し、ガバナンスフック証拠フィールドへのクロスリンクを追加。
