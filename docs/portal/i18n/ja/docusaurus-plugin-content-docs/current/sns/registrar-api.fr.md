---
lang: ja
direction: ltr
source: docs/portal/docs/sns/registrar-api.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note ソースカノニク
`docs/source/sns/registrar_api.md` などの詳細ページを参照してください
コピー カノニーク デュ ポルタイユ。 Le fichier source est conserve pour les flux de
トラダクション。
:::

# レジストラー SNS および管理用 API (SN-2b)

**ステータス:** Redige 2026-03-24 -- Nexus コアのレビュー  
**先取特権ロードマップ:** SN-2b「レジストラー API とガバナンス フック」  
**前提条件:** スキーマの定義 [`registry-schema.md`](./registry-schema.md)

注: エンドポイント Torii、サービス gRPC、DTO の要求/応答を指定します。
ソラ名レジストラの管理に必要な成果物など
サービス（SNS）。 SDK、ウォレットなどのリファレンスを参照
SNS の登録者、再構築者を自動化します。

## 1. トランスポートと認証

|エクシジェンス |詳細 |
|----------|----------|
|プロトコル | REST は `/v2/sns/*` であり、サービスは gRPC `sns.v1.Registrar` です。レドゥ受け入れ Norito-JSON (`application/json`) および Norito-RPC バイナリ (`application/x-norito`)。 |
|認証 | Jetons `Authorization: Bearer` は、mTLS emis par サフィックス スチュワードを認証します。管理上の賢明なエンドポイント (凍結/凍結解除、影響の予備) は緊急 `scope=sns.admin` です。 |
|借方制限 |登録機関のバケット `torii.preauth_scheme_limits` 控訴人 JSON と接尾辞の制限: `sns.register`、`sns.renew`、`sns.controller`、`sns.freeze`。 |
|テレメトリー | Torii レジストラのハンドラ (フィルタ `scheme="norito_rpc"`) を公開する `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` l'API インクリメントオーストラリア `sns_registrar_status_total{result, suffix_id}`。 |

## 2. DTO の操作

チャンプ参照構造体定義定義 [`registry-schema.md`](./registry-schema.md)。統合された `NameSelectorV1` + `SuffixId` は、ルーティングの曖昧さを回避するための料金を支払います。

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

|エンドポイント |メトーデ |ペイロード |説明 |
|----------|-----------|-----------|---------------|
| `/v2/sns/registrations` |投稿 | `RegisterNameRequestV1` |登録者は名前を付けてください。優先順位の決定、支払い/統治の有効性、登録上のイベントの有効性。 |
| `/v2/sns/registrations/{selector}/renew` |投稿 | `RenewNameRequestV1` |期間を延長します。レ・フェネトル・ド・グレース/償還デピュイ・ラ・ポリティックのアップリケ。 |
| `/v2/sns/registrations/{selector}/transfer` |投稿 | `TransferNameRequestV1` |統治権の承認を譲渡します。 |
| `/v2/sns/registrations/{selector}/controllers` |置く | `UpdateControllersRequestV1` |コントローラーのアンサンブルを置き換えます。有効な住所録署名者。 |
| `/v2/sns/registrations/{selector}/freeze` |投稿 | `FreezeNameRequestV1` |ガーディアン/評議会を凍結します。 Exige とチケット保護者および管理書類の参照。 |
| `/v2/sns/registrations/{selector}/freeze` |削除 | `GovernanceHookV1` |修復後に凍結を解除します。議会の登録を無効にすることを保証します。 |
| `/v2/sns/reserved/{selector}` |投稿 | `ReservedAssignmentRequestV1` |愛情の名前は、スチュワード/評議会の権限を留保します。 |
| `/v2/sns/policies/{suffix_id}` |入手 | -- | Recupere le `SuffixPolicyV1` クーラント (キャッシュ可能)。 |
| `/v2/sns/registrations/{selector}` |入手 | -- | Retourne le `NameRecordV1` courant + etat 効果 (アクティブ、グレースなど)。 |

**セレクターのエンコード:** ファイル セグメント `{selector}` は I105 を受け入れ、圧縮し、16 進数の正規セロン ADDR-5 を受け入れます。 Torii ファイルは、`NameSelectorV1` によって正規化されます。

**エラーのモデル:** エンドポイントの戻り値 Norito JSON 平均 `code`、`message`、`details`。コードには `sns_err_reserved`、`sns_err_payment_mismatch`、`sns_err_policy_violation`、`sns_err_governance_missing` が含まれます。

### 3.1 CLI の補助 (レジストラ マニュアル N0 の義務)

メインの JSON を使用せずに、CLI 経由でベータ版の管理者が登録者を登録します:

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

- `--owner` CLI のデフォルト設定を完了します。 repetez `--controller` は、コント ローラーの追加機能を提供します (デフォルト `[owner]`)。
- `PaymentProofV1` に対するインライン支払いマップペントのフラグのフラグ。 passez `--payment-json PATH` Quand vous avez deja un recu 構造。メタドン (`--metadata-json`) と統治のフック (`--governance-json`) ミーム スキーマの説明。

繰り返しを完了する講義:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Voir `crates/iroha_cli/src/commands/sns.rs` 実装;再利用可能な DTO のコマンド Norito は、出撃後の CLI のドキュメント ダンスを説明し、補助応答 Torii にバイト単位で対応します。

Des aidesSupplementaires couvrent les renouvellements、転送およびアクションの保護者:

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
````--governance-json` 登録の内容を実行します。 `GovernanceHookV1` 有効です (提案 ID、投票ハッシュ、署名スチュワード/保護者)。エンドポイント `/v2/sns/registrations/{selector}/...` の対応者は、ベータ版のレピーターの正確な表面のオペレーターを注ぎ、Torii SDK の申し立てを行います。

## 4. サービス gRPC

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

ワイヤ形式: スキーマのハッシュ Norito コンパイル時の登録一時ファイル
`fixtures/norito_rpc/schema_hashes.json` (ライン `RegisterNameRequestV1`、
`RegisterNameResponseV1`、`NameRecordV1` など)。

## 5. 統治とプルーヴのフック

Chaque appel qui modify l'etat doit joindre des preuves reutilisables pour la relecture:

|アクション | Donnees de governance の要求 |
|------|--------------------------------|
|登録/再ノウベルメント基準 | Preuve de paiement 指示書、和解指示書。オークン投票評議会は承認審査員を要求します。 |
|プレミアム / 愛情予備金の登録 | `GovernanceHookV1` 参照プロポーザル ID + スチュワード承認。 |
|転送 |ハッシュデュ投票評議会 + ハッシュデュシグナル DAO。保護者の許可は、訴訟の解決に伴う譲渡の義務を負うものとします。 |
|凍結/凍結解除 |チケットガーディアンの署名と評議会のオーバーライド（凍結解除）。 |

Torii 検証者によるプレビューの検証:

1. 統治台帳上に存在する提案書の識別子 (`/v2/governance/proposals/{id}`) および法定規則 `Approved`。
2. 投票記録の特派員。
3. 管理者/後見人の署名は、`SuffixPolicyV1` の公開出席者に参照されます。

Les controles en echec renvoient `sns_err_governance_missing`。

## 6. ワークフローの例

### 6.1 登録基準

1. クライアントの質問 `/v2/sns/policies/{suffix_id}` は、優先順位の高い回復者と、責任のあるレベルの回復者を求めます。
2. クライアントは `RegisterNameRequestV1` を構成します:
   - `selector` は、ラベル I105 (優先) または圧縮 (2 番目の選択) を取得します。
   - `term_years` 政治の限界。
   - `payment` スプリッタ トレゾレリー/スチュワードの転送参照。
3. Torii 有効:
   - ラベル + リストの正規化。
   - 期間/総額対 `PriceTierV1`。
   - Montant de preuve de paiement >= prix calcule + frais。
4. Torii が成功します。
   - `NameRecordV1` を永続化します。
   - エメット `RegistryEventV1::NameRegistered`。
   - エメット `RevenueAccrualEventV1`。
   - Retourne le nouveau レコード + 夕方。

### 6.2 ルヌーベルメント ペンダント ラ グレース

Les renouvellements ペンダント ラ グレース インクルエント ラ リクエスト スタンダード プラス ラ ディテクション デ ペナライト:

- Torii では、`now` と `grace_expires_at` を比較し、その他 `SuffixPolicyV1` の追加料金表を参照してください。
- La preuve de paiement doit couvrir la surcharge。 Echec => `sns_err_payment_mismatch`。
- `RegistryEventV1::NameRenewed` は `expires_at` を登録します。

### 6.3 ガーディアンの凍結と評議会のオーバーライド

1. ガーディアン スーメット `FreezeNameRequestV1` avec un ticket Referencant l'id d'incident。
2. Torii は、`NameStatus::Frozen` のファイル レコードを置き換え、`NameFrozen` を置き換えます。
3. 是正後、議会は優先を解除する。特使 DELETE `/v2/sns/registrations/{selector}/freeze` avec `GovernanceHookV1`。
4. Torii はオーバーライドを有効にし、`NameUnfrozen` を満たします。

## 7. 検証とエラーコード

|コード |説明 | HTTP |
|------|---------------|------|
| `sns_err_reserved` |ラベルリザーブオーブロック。 | 409 |
| `sns_err_policy_violation` |用語、政治政治家集団。 | 422 |
| `sns_err_payment_mismatch` |資産と資金の不一致。 | 402 |
| `sns_err_governance_missing` |統治上の成果物には不在/無効が必要です。 | 403 |
| `sns_err_state_conflict` |実際のサイクルを実行するための操作は許可されていません。 | 409 |

ファイル コードは、`X-Iroha-Error-Code` およびエンベロープ Norito JSON/NRPC 構造を介して送信されます。

## 8. 実装に関する注意事項

- Torii は、`NameRecordV1.auction` に注意してオークションをストックし、直接登録された `PendingAuction` を拒否します。
- Les preuves de paiement reutilisent les recus du ledger Norito; API ヘルパーの 4 つのアクセス サービス (`/v2/finance/sns/payments`)。
- SDK の開発エンベロープ CES エンドポイントの平均的なヘルパーの強化タイプ、ウォレットの提供者、レゾン ルール クレール (`ERR_SNS_RESERVED` など)。

## 9. プロカイン・エテープ- Relier les ハンドラー Torii au contrat de registre リール une fois les オークション SN-3 disponibles。
- SDK 固有のガイド (Rust/JS/Swift) 参照 API の発行者。
- Etendre [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) クロワーズ 対 プルーブ ド フック ド ガバナンスの平均。