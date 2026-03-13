---
lang: ja
direction: ltr
source: docs/portal/docs/sns/registrar-api.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ノート フエンテ カノニカ
Esta pagina refleja `docs/source/sns/registrar_api.md` y ahora sirve como la
コピア カノニカ デル ポータル。マンティエン パラ フルホス デ アーカイブ
取引。
:::

# API 登録者 SNS y フック デ ゴベルナンザ (SN-2b)

**エスタード:** Borrador 2026-03-24 -- Nexus コアのバジョ リビジョン  
**ロードマップの詳細:** SN-2b「レジストラー API とガバナンス フック」  
**前提条件:** [`registry-schema.md`](./registry-schema.md) の定義

エンドポイント Torii、gRPC サービス、DTO の要求に関する特定の情報
オペラ レジストラドール デルの必要な処置と芸術品
ソラネームサービス（SNS）。 SDK、ウォレットなどの独自の規制
必要なレジストラの自動化、SNS の改修。

## 1. 輸送と認証

|レクシト |デタル |
|----------|----------|
|プロトコル | REST バジョ `/v2/sns/*` およびサービス gRPC `sns.v1.Registrar`。アンボス アセプタン Norito-JSON (`application/json`) および Norito-RPC ビナリオ (`application/x-norito`)。 |
|認証 |トークン `Authorization: Bearer` o 証明書 mTLS 発行、サフィックス スチュワード。エンドポイントは、`scope=sns.admin` を必要とする gobernanza (凍結/凍結解除、予約の割り当て) を考慮しています。 |
|タサの限界 |レジストラは、サフィックス `sns.register`、`sns.renew`、`sns.controller`、`sns.freeze` の呼び出し元の JSON 制限値をバケット `torii.preauth_scheme_limits` に分割します。 |
|テレメトリア | Torii expone `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` 登録者用パラ ハンドラー (フィルター `scheme="norito_rpc"`)。 API タンビエン インクリメント `sns_registrar_status_total{result, suffix_id}`。 |

## 2. DTO の履歴書

ロス カンポス参照ロス構造体 canonicos definidos en [`registry-schema.md`](./registry-schema.md)。 Todas las cargas include `NameSelectorV1` + `SuffixId` para evitar ruteo ambiguo。

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

## 3. エンドポイント REST

|エンドポイント |メトド |ペイロード |説明 |
|----------|----------|----------|-------------|
| `/v2/sns/registrations` |投稿 | `RegisterNameRequestV1` |登録者は名前を付けません。貴重な情報を再取得し、パゴ/ゴベルナンザのプルエバスを検証し、登録イベントを発行します。 |
| `/v2/sns/registrations/{selector}/renew` |投稿 | `RenewNameRequestV1` |終点を延長します。 Aplica ventanas de gracia/redencion segun la politica. |
| `/v2/sns/registrations/{selector}/transfer` |投稿 | `TransferNameRequestV1` |必要な管理を変更します。 |
| `/v2/sns/registrations/{selector}/controllers` |置く | `UpdateControllersRequestV1` | Reemplaza el set deコントローラー;会社の方向を確認します。 |
| `/v2/sns/registrations/{selector}/freeze` |投稿 | `FreezeNameRequestV1` |フリーズdeガーディアン/カウンシル。保護者と政府の参照用チケットが必要です。 |
| `/v2/sns/registrations/{selector}/freeze` |削除 | `GovernanceHookV1` |トラス修復を解凍します。アセグラは議会登録を無効にします。 |
| `/v2/sns/reserved/{selector}` |投稿 | `ReservedAssignmentRequestV1` |管理者/評議会の任命保護区の割り当て。 |
| `/v2/sns/policies/{suffix_id}` |入手 | -- | Obtiene `SuffixPolicyV1` 実際（キャッシュ可能）。 |
| `/v2/sns/registrations/{selector}` |入手 | -- | Devuelve `NameRecordV1` 実際 + 効果 (アクティブ、グレースなど)。 |

**セレクターのコード:** セグメント `{selector}` アセプタ I105、16 進カノニコ セグン ADDR-5 を含む。 Torii は `NameSelectorV1` 経由で正規化されます。

**エラーのモデル:** todos los endpoints devuelven Norito JSON con `code`、`message`、`details`。ロスコディゴには、`sns_err_reserved`、`sns_err_payment_mismatch`、`sns_err_policy_violation`、`sns_err_governance_missing` が含まれます。

### 3.1 ヘルパー CLI (レジストラの必須マニュアル N0)

Los Stewards de beta cerrada pueden operar el registrator via la CLI sin armar JSON a mano:

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

- `--owner` CLI の構成に問題があるため。補助コントローラの付属品として `--controller` を繰り返します (デフォルトは `[owner]`)。
- ロスフラグは、`PaymentProofV1` にインラインでマップされます。米国 `--payment-json PATH` cuando ya tengas un recibo estructurado。メタデータ (`--metadata-json`) およびガバナンス フック (`--governance-json`) は、ミスモの常連客を表します。

ロス・エンサヨスを完了するソロ講義のヘルパー:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

実装時のバージョン `crates/iroha_cli/src/commands/sns.rs`。ロス コマンド Norito は、Torii での CLI のサリーダに関する文書の記述とバイトごとの一致を示しています。

ヘルパーの改修、転送、保護者への対応:

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
````--governance-json` 登録されたコンテナー `GovernanceHookV1` 有効 (提案 ID、投票ハッシュ、管理者/保護者)。エンドポイント `/v2/sns/registrations/{selector}/...` は、ベータ版のオペラドールの正確な内容に対応しており、Torii は SDK を参照しています。

## 4. gRPC のサービス

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

ワイヤ形式: ハッシュ デル エスケマ Norito コンパイル登録レジストラ
`fixtures/norito_rpc/schema_hashes.json` (フィラス `RegisterNameRequestV1`、
`RegisterNameResponseV1`、`NameRecordV1` など)。

## 5. フック・デ・ゴベルナンザと証拠

必要な追加証拠のAPTAパラリプレイを確認します:

|アクシオン |ゴベルナンサの要求事項 |
|------|--------------------------------|
|レジストロ/リノベーションエスタンダー |和解命令を参照するための手続き。議会の一斉射撃を要求する必要はありません。管理者に対する職務権限を要求する必要はありません。 |
|レジストロ プレミアム / アサインナシオン リザーダ | `GovernanceHookV1` クエリ参照プロポーザル ID + スチュワード承認。 |
|転送 |ハッシュ・デ・投票・デル・カウンシル + ハッシュ・デ・セナルDAO;保護者は、紛争解決のための転送許可を取得します。 |
|凍結/凍結解除 |チケット ガーディアンは評議会を無効にします (凍結を解除します)。 |

Torii 検証ラス プルエバス コンプロバンド:

1. 提案 ID は政府元帳 (`/v2/governance/proposals/{id}`) に存在し、ステータスは `Approved` です。
2. ハッシュはレジストラドの保存に一致します。
3. Firmas de Steward/Guardian Referencian las claves publicas esperadas de `SuffixPolicyV1`。

ファロス デブエルベン `sns_err_governance_missing`。

## 6. フルーホの実行

### 6.1 レジストロ基準

1. 顧客は、`/v2/sns/policies/{suffix_id}` の優先事項、恩恵を受ける条件を参照してください。
2. クライアント アルマ `RegisterNameRequestV1`:
   - `selector` ラベル I105 (プレフェリド) またはコンプリミド (セグンダ メジャー オプション) のデリバド。
   - `term_years` 政治の限界。
   - `payment` 参照、転送、スプリッター テソレリア/スチュワード。
3. Torii の有効性:
   - ラベルの正規化と予備のリスト。
   - 期間/総額対 `PriceTierV1`。
   - 証明金額 >= 高額計算 + 手数料。
4. Torii の出口:
   - `NameRecordV1` を永続化します。
   - `RegistryEventV1::NameRegistered` を発行します。
   - `RevenueAccrualEventV1` を発行します。
   - Devuelve el nuevo レジストロ + イベント。

### 6.2 継続的な改修工事

罰則の適用を含む不正な行為:

- Torii は、`now` と `grace_expires_at` と、`SuffixPolicyV1` の合計表を比較します。
- ラ・プルエバ・デ・パゴ・デベ・キュブリル・エル・レカルゴ。ファロ => `sns_err_payment_mismatch`。
- `RegistryEventV1::NameRenewed` 新規登録 `expires_at`。

### 6.3 保護者と議会の無効化に関する協議

1. Guardian envia `FreezeNameRequestV1` con ticket que Referencia id de Incidente。
2. Torii は `NameStatus::Frozen` を登録し、`NameFrozen` を発行します。
3. トラス修復、エルカウンシルエミットオーバーライド。エル オペラドール envia DELETE `/v2/sns/registrations/{selector}/freeze` と `GovernanceHookV1`。
4. Torii 有効オーバーライド、`NameUnfrozen` を発行します。

## 7. エラーコードの検証

|コディゴ |説明 | HTTP |
|--------|-------------|------|
| `sns_err_reserved` |リザーバド・オ・ブロックアードとラベルを貼ります。 | 409 |
| `sns_err_policy_violation` |政治の責任者を決める用語。 | 422 |
| `sns_err_payment_mismatch` |パゴの価値と資産の不一致。 | 402 |
| `sns_err_governance_missing` |オーセンテス/インバリドスを要求するゴベルナンザの加工品。 | 403 |
| `sns_err_state_conflict` |実際のシステムの操作は許可されていません。 | 409 |

`X-Iroha-Error-Code` とエンベロープ Norito JSON/NRPC 構造体を介した Todos los codigos salen。

## 8. 実装上の注意事項

- Torii は、`NameRecordV1.auction` で保護されたサブバスを保留し、`PendingAuction` で登録を管理する意図を確認します。
- ラス プルエバス デ パゴ レウティリザン レシボス デル レジャー Norito; tesoreria のサービス証明 API ヘルパー (`/v2/finance/sns/payments`)。
- SDK のデベンエンボルバー、エストスエンドポイント、ヘルパー、ウォレット、エラー、エラー (`ERR_SNS_RESERVED` など) を失います。

## 9. プロキシモスパソス

- Torii のハンドラーは、SN-3 の実際のレジストリに登録されています。
- SDK (Rust/JS/Swift) 固有のリファレンス API を公開。
- エクステンダー [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) は、ガバナンス フックの証拠を取り除きます。