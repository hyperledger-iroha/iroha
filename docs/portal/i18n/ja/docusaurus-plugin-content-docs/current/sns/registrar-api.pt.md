---
lang: ja
direction: ltr
source: docs/portal/docs/sns/registrar-api.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note フォンテ カノニカ
Esta pagina espelha `docs/source/sns/registrar_api.md` e a goraserve como a
コピアカノニカドポータル。永遠に貿易を続けてください。
:::

# レジストラ SNS e フック ド ガバナンカを行う API (SN-2b)

**ステータス:** Redigido 2026-03-24 -- 泣きながら Nexus コアを見直しました  
**ロードマップへのリンク:** SN-2b「レジストラー API とガバナンス フック」  
**前提条件:** 定義 [`registry-schema.md`](./registry-schema.md)

OS エンドポイント Torii、サービス gRPC、DTO の要求/応答に関する詳細情報
ソラ ネーム サービス (SNS) のレジストラの管理に必要な技術。
SDK、ウォレット、自動レジストラの自動制御、
SNS を革新します。

## 1. 安全な輸送

|レクシト |デタルヘ |
|----------|----------|
|プロトコル | REST sob `/v1/sns/*` および servico gRPC `sns.v1.Registrar`。アンボス アセタム Norito-JSON (`application/json`)、Norito-RPC ビナリオ (`application/x-norito`)。 |
|認証 |トークン `Authorization: Bearer` は、サフィックス スチュワードの mTLS 発行証明書です。エンドポイントは管理 (凍結/凍結解除、属性保持) に準拠しており、`scope=sns.admin` です。 |
|分類限界 |レジストラは、OS バケット `torii.preauth_scheme_limits` com Chamadores JSON のバースト制限サフィックスを比較します: `sns.register`、`sns.renew`、`sns.controller`、`sns.freeze`。 |
|テレメトリア | Torii expoe `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` パラ ハンドラはレジストラを実行します (filtrar `scheme="norito_rpc"`)。 API タンベム増分 `sns_registrar_status_total{result, suffix_id}`。 |

## 2. Visao geral de DTO

OS カンポ参照 OS 構造体 canonicos 定義 [`registry-schema.md`](./registry-schema.md)。カーガスとしての Todas には、`NameSelectorV1` + `SuffixId` パラ エビタール ローテアメントがあいまいなものが含まれます。

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
|----------|----------|----------|-----------|
| `/v1/sns/registrations` |投稿 | `RegisterNameRequestV1` |レジストラはあなたの名前です。事前の準備、承認/政府の承認、登録イベントの発行を解決します。 |
| `/v1/sns/registrations/{selector}/renew` |投稿 | `RenewNameRequestV1` |エステンデ・オ・テルモ。政治的猶予/救済の適用。 |
| `/v1/sns/registrations/{selector}/transfer` |投稿 | `TransferNameRequestV1` |政府関連の権限を譲渡します。 |
| `/v1/sns/registrations/{selector}/controllers` |置く | `UpdateControllersRequestV1` |コントローラーを接続して置き換えます。バリダ・エンデレコス・デ・コンタ・アッシナドス。 |
| `/v1/sns/registrations/{selector}/freeze` |投稿 | `FreezeNameRequestV1` |フリーズdeガーディアン/カウンシル。チケット ガーディアンと行政文書の参照を要求します。 |
| `/v1/sns/registrations/{selector}/freeze` |削除 | `GovernanceHookV1` | apos remediacao を解凍します。議会登録を無効にすることを保証します。 |
| `/v1/sns/reserved/{selector}` |投稿 | `ReservedAssignmentRequestV1` |スチュワード/評議会によるノーム保護区。 |
| `/v1/sns/policies/{suffix_id}` |入手 | -- | Busca `SuffixPolicyV1` 実物 (cacheavel)。 |
| `/v1/sns/registrations/{selector}` |入手 | -- | Retorna `NameRecordV1` atual + estado efetivo (アクティブ、グレースなど)。 |

**セレクターコード:** セグメント `{selector}` ACEITA IH58、ADDR-5 に準拠した 16 進数の互換性。 Torii は `NameSelectorV1` 経由で正規化されます。

**モデルエラー:** todos os エンドポイント retornam Norito JSON com `code`、`message`、`details`。 OS コードには、`sns_err_reserved`、`sns_err_payment_mismatch`、`sns_err_policy_violation`、`sns_err_governance_missing` が含まれます。

### 3.1 ヘルパー CLI (レジストラ マニュアル N0 が必要)

CLI 経由のベータ版アゴラ ポデム オペラまたはレジストラの管理者は、JSON マニュアルを参照します。

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

- `--owner` CLI の設定を含むパッド。レピタ `--controller` パラアネクサー コンタス コントローラー アディショナ (パドラオ `[owner]`)。
- OS フラグは、`PaymentProofV1` パラメタのマップをインラインで表示します。 Passe `--payment-json PATH` は、すべてのメッセージを受信して​​ください。 Metadados (`--metadata-json`) は、ガバナンカのフック (`--governance-json`) を管理します。

OS を完了するためのヘルパー:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Veja `crates/iroha_cli/src/commands/sns.rs` を実装するためのパラメータ。 OS コマンドは再利用され、OS DTO Norito は、Torii を実行するときに CLI と同じドキュメントを記述します。

ヘルパーは、リノバの管理者、保護者への転送を支援します:

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
  --new-owner ih58... \
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

`--governance-json` レジストリの開発 `GovernanceHookV1` 有効 (提案 ID、投票ハッシュ、アシナチュア スチュワード/ガーディアン)。エンドポイントの簡単なコマンド `/v1/sns/registrations/{selector}/...` は、ベータ版のオペラドールに対応しており、権限 Torii の SDK がサポートされています。

## 4. Servico gRPC

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
```ワイヤ形式: ハッシュ ド エスケーマ Norito のテンポ デ コンピラカオ レジストラード
`fixtures/norito_rpc/schema_hashes.json` (`RegisterNameRequestV1`、
`RegisterNameResponseV1`、`NameRecordV1` など)。

## 5. 統治と証拠のフック

戸田氏は、再生に関する証拠を取得するために、次のことを行います:

|アカオ |政府の要求事項 |
|------|------------------------------|
|レジストロ/レノバカオ パドラオ |和解の指示を参照するための情報。 nao exige voto do Council a menos que o tier exija aprovacao do Steward. |
|レジストロ デ ティア プレミアム / アトリブイカオ リザーダ | `GovernanceHookV1` 参照プロポーザル ID + スチュワード承認。 |
|トランスファーレンシア |ハッシュデ投票評議会 + ハッシュデシナルDAO;保護者の許可は、紛争解決のための転送と活動の許可を与えられます。 |
|凍結/凍結解除 | Assinatura は、チケット ガーディアンによって、評議会 (凍結解除) を無効にします。 |

Torii verifica as provas conferindo:

1. 提案 ID は統治台帳が存在しません (`/v1/governance/proposals/{id}`) ステータス `Approved`。
2. ハッシュは登録証明書に対応します。
3. Assinaturas のスチュワード/保護者は、`SuffixPolicyV1` の公的機関として参照されます。

ファルハス・レトルナム`sns_err_governance_missing`。

## 6. トラバリョの実践例

### 6.1 レジストロ・パドラオ

1. お客様は、事前の手続き、猶予期間の拒否について `/v1/sns/policies/{suffix_id}` にご相談ください。
2. クライアント モンタ `RegisterNameRequestV1`:
   - `selector` ラベル IH58 (preferido) または comprimido (segunda melhor opcao) のデリバド。
   - `term_years` 政治の限界を超えてください。
   - `payment` 参照、転送、スプリッター tesouraria/スチュワード。
3. Torii の有効性:
   - 通常のラベル + リストの予約。
   - 期間/総額対 `PriceTierV1`。
   - プロバ デ パガメントの金額 >= プレコ計算 + 手数料。
4. Torii を実行します:
   - `NameRecordV1` を永続化します。
   - `RegistryEventV1::NameRegistered` を発行します。
   - `RevenueAccrualEventV1` を発行します。
   - 新しいレジストロとイベントを記録します。

### 6.2 レノバカオ デュランテ グレース

猶予期間の延長には、刑罰の要求が含まれます:

- Torii は、`now` と `grace_expires_at` を比較し、`SuffixPolicyV1` の症状を改善します。
- ソブレタキサの研究を開始します。ファルハ => `sns_err_payment_mismatch`。
- `RegistryEventV1::NameRenewed` 新規登録 `expires_at`。

### 6.3 ガーディアンの凍結と評議会の無効化

1. Guardian envia `FreezeNameRequestV1` com チケット参照番号。
2. Torii は、`NameStatus::Frozen` のレジストリから移動し、`NameFrozen` を発行します。
3. Apos remediacao、評議会はオーバーライドを発行します。 o オペレーター envia DELETE `/v1/sns/registrations/{selector}/freeze` com `GovernanceHookV1`。
4. Torii はオーバーライドを有効にし、`NameUnfrozen` を発行します。

## 7. バリダカオとコディゴス・デ・エロ

|コディゴ |説明 | HTTP |
|----------|----------|------|
| `sns_err_reserved` | Reservado ou bloqueado にラベルを付けます。 | 409 |
| `sns_err_policy_violation` |テルモ、政治家としての管理者と協力してください。 | 422 |
| `sns_err_payment_mismatch` |価値と資産の価値が一致していません。 | 402 |
| `sns_err_governance_missing` |オーセンテス/インバリドスを要求する統治技術。 | 403 |
| `sns_err_state_conflict` | Operacao nao permitida no estado atual do ciclo de vida. | 409 |

Todos os codigos aparecem via `X-Iroha-Error-Code` e envelopes Norito JSON/NRPC estruturados。

## 8. 実装上の注意事項

- Torii は、`NameRecordV1.auction` を保持しています。`PendingAuction` を参照してください。
- 元帳 Norito を再利用するための手順。 tesouraria fornecem API ヘルパーのサービス (`/v1/finance/sns/payments`)。
- SDK は、エンドポイント コム ヘルパーを開発し、情報を提供し、財布を提示し、クラロス デ エラー (`ERR_SNS_RESERVED` など) をサポートします。

## 9. プロキシモス・パソス

- Conectar os ハンドラーは、Torii を実際の quando os leiloes SN-3 chegarem に登録します。
- SDK (Rust/JS/Swift) 固有のリファレンス API を公開。
- Estender [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) com リンク クルザドス パラオス カンポス デ ガバナンス フックの証拠。