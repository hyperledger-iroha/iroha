---
lang: he
direction: rtl
source: docs/portal/docs/sns/registrar-api.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/sns/registrar_api.md` e agora serve como a
Copia canonica do פורטל. O arquivo fonte permanece para fluxos de traducao.
:::

# API לרשם SNS e hooks de governanca (SN-2b)

**סטטוס:** Redigido 2026-03-24 -- sob revisao do Nexus Core  
**קישור למפת הדרכים:** SN-2b "Registrar API & Governance Hooks"  
**דרישות מוקדמות:** Definicoes de esquema em [`registry-schema.md`](./registry-schema.md)

Esta not especifica OS Torii, servicos gRPC, DTOs de requisicao/resposta e
אמנות הממשל הנחוצות למבצע או הרשם בשירות Sora Name Service (SNS).
או אופטימליים עבור ערכות SDK, ארנקים ואוטומטים עבור רשם מדויק,
renovar ou generenciar nomes SNS.

## 1. Transporte e autenticacao

| Requisito | פרטים |
|----------------|--------|
| פרוטוקולים | REST יפח `/v1/sns/*` e servico gRPC `sns.v1.Registrar`. Ambos aceitam Norito-JSON (`application/json`) ו-Norito-RPC בינארי (`application/x-norito`). |
| Auth | אסימונים `Authorization: Bearer` או אישורי mTLS emitidos por suffix steward. נקודות קצה sensiveis a governanca (הקפאה/ביטול הקפאה, atribuicoes reservadas) exigem `scope=sns.admin`. |
| Limites de taxa | הרשמים compartilham os buckets `torii.preauth_scheme_limits` com chamadores JSON mais limites de burst por סיומת: `sns.register`, `sns.renew`, `sns.controller`, Norito. |
| טלמטריה | Torii expoe `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` מטפלים דוגמת רשם (מסנן `scheme="norito_rpc"`); a API tambem incrementa `sns_registrar_status_total{result, suffix_id}`. |

## 2. Visao geral de DTO

Os campos referenciam os structs canonicos definidos em [`registry-schema.md`](./registry-schema.md). כל מטענים כוללים `NameSelectorV1` + `SuffixId` למען הסר ספק.

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

## 3. נקודות קצה REST

| נקודת קצה | Metodo | מטען | תיאור |
|--------|--------|--------|--------|
| `/v1/sns/names` | פוסט | `RegisterNameRequestV1` | הרשם ou reabrir um nome. פתרו את הדרג הקדם, valida provas de pagamento/governanca, emite eventos de registro. |
| `/v1/sns/names/{namespace}/{literal}/renew` | פוסט | `RenewNameRequestV1` | Estende o termo. Aplica Janelas de Grace/Remption da Politica. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | פוסט | `TransferNameRequestV1` | Transfere propriedade quando aprovacoes de governanca forem anexadas. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | PUT | `UpdateControllersRequestV1` | Substitui o conjunto de controls; valida enderecos de conta assinados. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | פוסט | `FreezeNameRequestV1` | הקפאת האפוטרופוס/המועצה. בקש כרטיס אפוטרופוס e referencia ao docket de governanca. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | מחק | `GovernanceHookV1` | הסר את ההקפאה apos remediacao; garante override do Council registrado. |
| `/v1/sns/reserved/{selector}` | פוסט | `ReservedAssignmentRequestV1` | Atribuicao de nomes reservados por דייל/מועצה. |
| `/v1/sns/policies/{suffix_id}` | קבל | -- | Busca `SuffixPolicyV1` atual (cacheavel). |
| `/v1/sns/names/{namespace}/{literal}` | קבל | -- | Retorna `NameRecordV1` atual + estado efetivo (Active, Grace וכו'). |**Codificacao de selector:** o segmento `{selector}` aceita I105, comprimido ou hex canonico conforme ADDR-5; Torii נורמליזציה באמצעות `NameSelectorV1`.

**דגם שגיאות:** נקודות קצה של פעולות הפעלה retornam Norito JSON com `code`, `message`, `details`. מערכת קודים כוללת `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Helpers CLI (דרישת מדריך לרשם N0)

דיילים של ביתא פקהדה מפעילים או רשם דרך CLI אשר מותקן במדריך JSON:

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` padrao e a conta de configuracao da CLI; repita `--controller` עבור קונטרולר נוסף (Padrao `[owner]`).
- דגלי Os inline de pagamento mapeiam direto para `PaymentProofV1`; pass `--payment-json PATH` quando voce ja tiver um recibo estruturado. Metadados (`--metadata-json`) e hooks de governanca (`--governance-json`) seguem o mesmo padrao.

Helpers de leitura somente completam os ensaios:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Veja `crates/iroha_cli/src/commands/sns.rs` para a implementacao; os comandos reutilizam os DTOs Norito תיאור המסמכים הבאים עבור CLI coincida byte por byte com כמו תשובות לעשות Torii.

Helpers adiconais cobrem renovacoes, transferencias e acoes de guardian:

```bash
# Renew an expiring name
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
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

`--governance-json` deve conter um registro `GovernanceHookV1` valido (מזהה הצעה, גיבוב הצבעה, דייל/אפוטרופוס). Cada comando simplesmente espelha o נקודת קצה `/v1/sns/names/{namespace}/{literal}/...` correspondente para que Operatores de Beta Ensaiem Exatamente as superficies Torii que os SDKs chamarao.

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
```

פורמט חוט: hash do esquema Norito בטמפו דה קומפילקאו registrado em
`fixtures/norito_rpc/schema_hashes.json` (linhas `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1` וכו').

## 5. הוכחות דה גוברננה

Toda chamada que altera estado deve anexar evidencias adequadas para replay:

| Acao | Dados de governanca requeridos |
|------|------------------------------|
| Registro/renovacao padrao | Prova de pagamento referenciando uma instrucao de settlement; nao exige voto do Council a menos que o tier exija aprovacao do דייל. |
| Registro de tier premium / atribuicao reservada | `GovernanceHookV1` מזהה הצעת עזר + אישור דייל. |
| Transferencia | Hash de voto do Council + hash de sinal DAO; אישור אפוטרופוס quando a transferencia e acionada por resolucao de disputa. |
| הקפאה/בטל הקפאה | Assinatura do ticket guardian mais override do Council (ביטול הקפאה). |

Torii אימות כבדיקת התייעצות:

1. זיהוי ההצעה קיים ללא ספר חשבונות (`/v1/governance/proposals/{id}`) e o status e `Approved`.
2. Hashes correspondem aos artefatos de voto registrados.
3. Assinaturas דייל/אפוטרופוס referenciam as chaves publicas esperadas de `SuffixPolicyV1`.

Falhas retornam `sns_err_governance_missing`.

## 6. דוגמאות לזרימת טראבלהו

### 6.1 Registro padrao1. O cliente consulta `/v1/sns/policies/{suffix_id}` para precos, Grace e Tiers Disponiveis.
2. O cliente monta `RegisterNameRequestV1`:
   - `selector` derivado de label I105 (preferido) ou comprimido (segunda melhor opcao).
   - `term_years` dentro dos limites da politica.
   - `payment` referenciando a transferencia do splitter tesouraria/דייל.
3. תוקף Torii:
   - Normalizacao de label + List reservada.
   - טווח/מחיר ברוטו לעומת `PriceTierV1`.
   - Prova de pagamento סכום >= preco calculado + עמלות.
4. לאחר מכן Torii:
   - Persiste `NameRecordV1`.
   - Emite `RegistryEventV1::NameRegistered`.
   - Emite `RevenueAccrualEventV1`.
   - Retorna o novo registro + eventos.

### 6.2 Renovacao durante grace

Renovacoes durante grace כולל דרוש:

- Torii השוואת `now` לעומת `grace_expires_at` e adiciona tableas de sobretaxa de `SuffixPolicyV1`.
- A prova de pagamento deve cobrir a sobretaxa. Falha => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` רישום או חדש `expires_at`.

### 6.3 הקפאת האפוטרופוס וביטול המועצה

1. Guardian envia `FreezeNameRequestV1` com ticket referenciando id de incidente.
2. Torii move o registro para `NameStatus::Frozen`, emite `NameFrozen`.
3. Apos remediacao, o מועצה פולטת לעקוף; o operator envia DELETE `/v1/sns/names/{namespace}/{literal}/freeze` com `GovernanceHookV1`.
4. Torii valida o override, emite `NameUnfrozen`.

## 7. Validacao e codigos de ro

| קודיגו | תיאור | HTTP |
|--------|--------|------|
| `sns_err_reserved` | תווית reservado ou bloqueado. | 409 |
| `sns_err_policy_violation` | Termo, tier ou conjunto de controls viola a politica. | 422 |
| `sns_err_payment_mismatch` | אי התאמה דה חיל או נכס נא פרובה דה פאגמנטו. | 402 |
| `sns_err_governance_missing` | Artefatos de governanca requeridos ausentes/invalidos. | 403 |
| `sns_err_state_conflict` | Operacao nao permitida no estado atual do ciclo de vida. | 409 |

Todos os codigos aparecem via `X-Iroha-Error-Code` e envelopes Norito JSON/NRPC estruturados.

## 8. Notas de implementacao

- Torii armazena leiloes pendentes em `NameRecordV1.auction` e rejeita tentativas de registro direto enquanto estiver `PendingAuction`.
- Provas de pagamento reutilizam recibos do book Norito; עוזר API של servicos de tesouraria fornecem (`/v1/finance/sns/payments`).
- SDKs devem envolver esses endpoints com helpers fortemente tipados para que wallets apresentem motivos claros de ro (`ERR_SNS_RESERVED`, וכו').

## 9. Proximos passos

- מטפלי מערכות הפעלה של Conectar מבצעים Torii ובין היתר את הרישום האמיתי של ה-Leiloes SN-3 chegarem.
- הסבר מפורט על ה-SDK (Rust/JS/Swift) המתייחס ל-API.
- Estender [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) com קישורים cruzados para os campos de evidencia de governance hook.