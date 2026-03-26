---
lang: pt
direction: ltr
source: docs/portal/docs/sns/registry-schema.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sns/registry_schema.md` کی عکاسی کرتا ہے اور پورٹل کی کینونیکل کاپی کے طور پر کام کرتا ہے۔ سورس فائل ترجمہ اپ ڈیٹس کے لیے برقرار رہے گی۔
:::

# Sora Name Service رجسٹری اسکیمہ (SN-2a)

**حیثیت:** مسودہ 2026-03-24 -- SNS پروگرام ریویو کے لیے جمع کیا گیا  
**روڈمیپ لنک:** SN-2a "Esquema de registro e layout de armazenamento"  
**دائرہ:** Sora Name Service (SNS) کے لیے Norito اسٹرکچرز, لائف سائیکل اسٹیٹس اور اخراجی ایونٹس متعین کرنا تاکہ registro e registrador کی implementações کنٹریکٹس, SDKs e gateways میں رہیں۔

یہ دستاویز SN-2a کے اسکیمہ ڈیلیورایبل کو مکمل کرتی ہے, جس میں درج ذیل شامل ہیں:

1. شناختیں اور hashing قواعد (`SuffixId`, `NameHash`, derivação do seletor)۔
2. نام ریکارڈز, sufixo پالیسیز, قیمت níveis, ریونیو سپلٹس اور رجسٹری ایونٹس کے لیے Estruturas/enums Norito۔
3. Reprodução determinística کے لیے layout de armazenamento e prefixos de índice۔
4. رجسٹریشن, تجدید, graça/redenção, congelamento e lápide پر مشتمل máquina de estado۔
5. Automação de DNS/gateway کے لیے eventos canônicos۔

## 1. شناختیں اور hashing

| شناخت | وضاحت | Mais |
|------------|-------------|------------|
| `SuffixId` (`u16`) | ٹاپ لیول sufixos (`.sora`, `.nexus`, `.dao`) کے لیے رجسٹری شناخت۔ [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) کے catálogo de sufixos کے مطابق۔ | voto de governança سے مختص; `SuffixPolicyV1` میں محفوظ۔ |
| `SuffixSelector` | sufixo کی string canônica شکل (ASCII, minúscula)۔ | Exemplo: `.sora` -> `sora`. |
| `NameSelectorV1` | رجسٹر شدہ لیبل کے لیے بائنری seletor۔ | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. لیبل Norm v1 کے مطابق NFC + minúsculas۔ |
| `NameHash` (`[u8;32]`) | کنٹریکٹس, ایونٹس اور caches کے لیے بنیادی سرچ key۔ | `blake3(NameSelectorV1_bytes)`. |

Determinismo:

- لیبلز Norma v1 (UTS-46 estrito, STD3 ASCII, NFC) کے ذریعے normalizar ہوتے ہیں۔ صارف کی strings کو hashing سے پہلے normalize کرنا لازم ہے۔
- Etiquetas reservadas (`SuffixPolicyV1.reserved_labels`) کبھی registro میں داخل نہیں ہوتے؛ substituições somente de governança `ReservedNameAssigned` ایونٹس خارج کرتے ہیں۔

## 2. Norito ساختیں

### 2.1 NomeRegistroV1| فیلڈ | قسم | Não |
|-------|------|-------|
| `suffix_id` | `u16` | `SuffixPolicyV1` کی ریفرنس۔ |
| `selector` | `NameSelectorV1` | auditoria/depuração کے لیے bytes do seletor bruto۔ |
| `name_hash` | `[u8; 32]` | mapas/eventos کے لیے chave۔ |
| `normalized_label` | `AsciiString` | انسانی قابلِ پڑھائی لیبل (Norma v1 کے بعد)۔ |
| `display_label` | `AsciiString` | mordomo کی invólucro؛ اختیاری cosméticos۔ |
| `owner` | `AccountId` | renovações/transferências کنٹرول کرتا ہے۔ |
| `controllers` | `Vec<NameControllerV1>` | اکاؤنٹ ایڈریسز, resolvedores e metadados کے حوالہ جات۔ |
| `status` | `NameStatus` | لائف سائیکل فلیگ (4 دیکھیں)۔ |
| `pricing_class` | `u8` | sufixo کے níveis de preços کا índice (padrão, premium, reservado)۔ |
| `registered_at` | `Timestamp` | Ativação de ativação کا بلاک ٹائم۔ |
| `expires_at` | `Timestamp` | O que é melhor para você |
| `grace_expires_at` | `Timestamp` | graça de renovação automática کا اختتام (padrão +30 dias)۔ |
| `redemption_expires_at` | `Timestamp` | janela de resgate کا اختتام (padrão +60 dias)۔ |
| `auction` | `Option<NameAuctionStateV1>` | Holandês reabre یا leilões premium کی صورت میں موجود۔ |
| `last_tx_hash` | `Hash` | Este é um ponteiro determinístico. |
| `metadata` | `Metadata` | registrador کی metadados arbitrariamente (registros de texto, provas)۔ |

Estruturas principais:

```text
Enum NameStatus {
    Available,          // derived, not stored on-ledger
    PendingAuction,
    Active,
    GracePeriod,
    Redemption,
    Frozen(NameFrozenStateV1),
    Tombstoned(NameTombstoneStateV1)
}

Struct NameFrozenStateV1 {
    reason: String,
    until_ms: u64,
}

Struct NameTombstoneStateV1 {
    reason: String,
}

Struct NameControllerV1 {
    controller_type: ControllerType,   // Account, ResolverTemplate, ExternalLink
    account_address: Option<AccountAddress>,   // Serialized as canonical `0x...` hex in JSON
    resolver_template_id: Option<String>,
    payload: Metadata,                 // Extra selector/value pairs for wallets/gateways
}

Struct TokenValue {
    asset_id: AsciiString,
    amount: u128,
}

Enum ControllerType {
    Account,
    Multisig,
    ResolverTemplate,
    ExternalLink
}

Struct NameAuctionStateV1 {
    kind: AuctionKind,             // Vickrey, DutchReopen
    opened_at_ms: u64,
    closes_at_ms: u64,
    floor_price: TokenValue,
    highest_commitment: Option<Hash>,  // reference to sealed bid
    settlement_tx: Option<Json>,
}

Enum AuctionKind {
    VickreyCommitReveal,
    DutchReopen
}
```

### 2.2 Política de SufixoV1

| فیلڈ | قسم | Não |
|-------|------|-------|
| `suffix_id` | `u16` | Chave بنیادی; پالیسی ورژنز میں مستحکم۔ |
| `suffix` | `AsciiString` | مثال کے طور پر `sora`۔ |
| `steward` | `AccountId` | carta de governança میں متعین steward۔ |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | identificador de ativo de liquidação padrão (مثلا `61CtjvNd9T3THAR65GsMVHr82Bjc`)۔ |
| `pricing` | `Vec<PriceTierV1>` | قیمت کے níveis کے coeficientes اور مدت کے قواعد۔ |
| `min_term_years` | `u8` | خریدی گئی مدت کے لیے کم از کم حد۔ |
| `grace_period_days` | `u16` | Padrão 30. |
| `redemption_period_days` | `u16` | Padrão 60. |
| `max_term_years` | `u8` | پیشگی تجدید کی زیادہ سے زیادہ مدت۔ |
| `referral_cap_bps` | `u16` | <=1000 (10%) fretamento کے مطابق۔ |
| `reserved_labels` | `Vec<ReservedNameV1>` | governança کی فراہم کردہ فہرست مع atribuir ہدایات۔ |
| `fee_split` | `SuffixFeeSplitV1` | tesouraria / administrador / referência حصص (pontos base)۔ |
| `fund_splitter_account` | `AccountId` | escrow رکھنے اور فنڈز تقسیم کرنے والا اکاؤنٹ۔ |
| `policy_version` | `u16` | ہر تبدیلی پر بڑھتا ہے۔ |
| `metadata` | `Metadata` | توسیعی نوٹس (convênio KPI, hashes de documentos de conformidade)۔ |

```text
Struct PriceTierV1 {
    tier_id: u8,
    label_regex: String,       // RE2-syntax pattern describing eligible labels
    base_price: TokenValue,    // Price per one-year term before suffix coefficient
    auction_kind: AuctionKind, // Default auction when the tier triggers
    dutch_floor: Option<TokenValue>,
    min_duration_years: u8,
    max_duration_years: u8,
}

Struct ReservedNameV1 {
    normalized_label: AsciiString,
    assigned_to: Option<AccountId>,
    release_at_ms: Option<u64>,
    note: String,
}

Struct SuffixFeeSplitV1 {
    treasury_bps: u16,     // default 7000 (70%)
    steward_bps: u16,      // default 3000 (30%)
    referral_max_bps: u16, // optional referral carve-out (<= 1000)
    escrow_bps: u16,       // % routed to claw-back escrow
}
```

### 2.3 ریونیو اور liquidação ریکارڈز| Estrutura | فیلڈز | مقصد |
|--------|-------|------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | época de liquidação (ہفتہ وار) کے حساب سے roteado ادائیگیوں کا ریکارڈ۔ |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | ہر ادائیگی پوسٹ ہونے پر emitir (registro, renovação, leilão)۔ |

تمام `TokenValue` فیلڈز Norito کی codificação canônica de ponto fixo استعمال کرتی ہیں اور کرنسی کوڈ متعلقہ `SuffixPolicyV1` میں declarar ہوتا ہے۔

### 2.4 رجسٹری ایونٹس

Eventos canônicos Automação de DNS/gateway اور Analytics کے لیے log de repetição فراہم کرتے ہیں۔

```text
Struct RegistryEventV1 {
    name_hash: [u8; 32],
    suffix_id: u16,
    selector: NameSelectorV1,
    version: u64,               // increments per NameRecord update
    timestamp: Timestamp,
    tx_hash: Hash,
    actor: AccountId,
    event: RegistryEventKind,
}

Enum RegistryEventKind {
    NameRegistered { expires_at: Timestamp, pricing_class: u8 },
    NameRenewed { expires_at: Timestamp, term_years: u8 },
    NameTransferred { previous_owner: AccountId, new_owner: AccountId },
    NameControllersUpdated { controller_count: u16 },
    NameFrozen(NameFrozenStateV1),
    NameUnfrozen,
    NameTombstoned(NameTombstoneStateV1),
    AuctionOpened { kind: AuctionKind },
    AuctionSettled { winning_account: AccountId, clearing_price: TokenValue },
    RevenueSharePosted { epoch_id: u64, treasury_amount: TokenValue, steward_amount: TokenValue },
    SuffixPolicyUpdated { policy_version: u16 },
}
```

ایونٹس کو log repetível (domínio `RegistryEvents`) میں anexar کرنا ضروری ہے اور feeds de gateway میں espelho کرنا ضروری ہے تاکہ DNS caches SLA کے اندر invalidar ہوں۔

## 3. Layout de armazenamento e índices

| Chave | وضاحت |
|-----|-------------|
| `Names::<name_hash>` | `name_hash` سے `NameRecordV1` تک بنیادی mapa۔ |
| `NamesByOwner::<AccountId, suffix_id>` | carteira UI کے لیے ثانوی índice (paginação amigável)۔ |
| `NamesByLabel::<suffix_id, normalized_label>` | detecção de conflitos کرتا ہے اور pesquisa determinística فعال بناتا ہے۔ |
| `SuffixPolicies::<suffix_id>` | Versão `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` ہسٹری۔ |
| `RegistryEvents::<u64>` | log somente anexado جس کی sequência de teclas monotônica ہے۔ |

تمام chaves Norito tuplas کے ساتھ serialize ہوتی ہیں تاکہ hosts کے درمیان hashing determinístico رہے۔ atualizações de índice بنیادی ریکارڈ کے ساتھ atomicamente ہوتی ہیں۔

## 4. Máquina de estado do ciclo de vida

| Estado | Condições de entrada | Transições permitidas | Notas |
|-------|------------------|--------------------|-------|
| Disponível | جب `NameRecord` موجود نہ ہو۔ | `PendingAuction` (premium), `Active` (registro padrão). | pesquisa de disponibilidade صرف índices پڑھتی ہے۔ |
| Leilão Pendente | جب `PriceTierV1.auction_kind` != nenhum ہو۔ | `Active` (liquidação do leilão), `Tombstoned` (sem lances). | leilões `AuctionOpened` e `AuctionSettled` emitem کرتی ہیں۔ |
| Ativo | رجسٹریشن یا تجدید کامیاب ہو۔ | `GracePeriod`, `Frozen`, `Tombstoned`. | Transição `expires_at` چلاتا ہے۔ |
| Período de graça | جب `now > expires_at` ہو۔ | `Active` (renovação dentro do prazo), `Redemption`, `Tombstoned`. | Padrão +30 dias; resolver ہوتا ہے مگر sinalizador ہوتا ہے۔ |
| Redenção | `now > grace_expires_at` é `< redemption_expires_at`. | `Active` (renovação tardia), `Tombstoned`. | کمانڈز پر multa taxa درکار ہے۔ |
| Congelado | governança یا guardião congelamento۔ | `Active` (remediação کے بعد), `Tombstoned`. | transferir یا atualização de controladores نہیں کر سکتے۔ |
| Lápide | رضاکارانہ rendição, مستقل disputa نتیجہ, یا resgate ختم۔ | `PendingAuction` (reabertura holandesa) یا lápide رہتا ہے۔ | `NameTombstoned` ایونٹ میں وجہ شامل ہونی چاہیے۔ |

Transições de estado لازمی طور پر متعلقہ `RegistryEventKind` emitem کریں تاکہ caches downstream ہم آہنگ رہیں۔ Tombstoned نام جو Leilões de reabertura holandeses میں داخل ہوں وہ Carga útil `AuctionKind::DutchReopen` شامل کرتے ہیں۔## 5. Eventos canônicos e sincronização de gateway

Gateways `RegistryEventV1` کو سبسکرائب کرتے ہیں اور DNS/SoraFS کو یوں sincronização کرتے ہیں:

1. ایونٹ sequência میں حوالہ کردہ تازہ ترین `NameRecordV1` حاصل کریں۔
2. resolver templates دوبارہ بنائیں (i105 ترجیحی + i105 second‑best addresses, text records)۔
3. [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) میں بیان کردہ Fluxo de trabalho SoraDNS کے ذریعے pino de dados da zona کریں۔

Garantias de entrega de eventos:

- ہر ٹرانزیکشن جو `NameRecordV1` کے ساتھ صرف ایک ایونٹ شامل کرے جو سختی سے بڑھتی ہو۔
- `RevenueSharePosted` ایونٹس `RevenueShareRecordV1` سے assentamentos emitidos کو referência کرتے ہیں۔
- congelar/descongelar/tombstone ایونٹس repetição de auditoria کے لیے `metadata` میں hashes de artefato de governança شامل کرتے ہیں۔

## 6. Cargas úteis Norito کی مثالیں

### 6.1 NameRecord Nome

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "<katakana-i105-account-id>",
    controllers: [
        NameControllerV1 {
            controller_type: Account,
            account_address: Some(AccountAddress("0x020001...")),
            resolver_template_id: None,
            payload: {}
        }
    ],
    status: Active,
    pricing_class: 0,
    registered_at: 1_776_000_000,
    expires_at: 1_807_296_000,
    grace_expires_at: 1_809_888_000,
    redemption_expires_at: 1_815_072_000,
    auction: None,
    last_tx_hash: 0xa3d4...c001,
    metadata: { "resolver": "wallet.default", "notes": "SNS beta cohort" },
}
```

### 6.2 SuffixPolicy usado

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "<katakana-i105-account-id>",
    status: Active,
    payment_asset_id: "61CtjvNd9T3THAR65GsMVHr82Bjc",
    pricing: [
        PriceTierV1 { tier_id:0, label_regex:"^[a-z0-9]{3,}$", base_price:"120 XOR", auction_kind:VickreyCommitReveal, dutch_floor:None, min_duration_years:1, max_duration_years:5 },
        PriceTierV1 { tier_id:1, label_regex:"^[a-z]{1,2}$", base_price:"10_000 XOR", auction_kind:DutchReopen, dutch_floor:Some("1_000 XOR"), min_duration_years:1, max_duration_years:3 }
    ],
    min_term_years: 1,
    grace_period_days: 30,
    redemption_period_days: 60,
    max_term_years: 5,
    referral_cap_bps: 500,
    reserved_labels: [
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("<katakana-i105-account-id>"), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "<katakana-i105-account-id>",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. اگلے اقدامات

- **SN-2b (API do registrador e ganchos de governança):** structs e Torii کے ذریعے expor کریں (Norito e ligações JSON) e verificações de admissão e artefatos de governança سے جوڑیں۔
- **SN-3 (mecanismo de leilão e registro):** confirmar/revelar اور Lógica de reabertura holandesa کے لیے `NameAuctionStateV1` دوبارہ استعمال کریں۔
- **SN-5 (Pagamento e liquidação):** Reconciliação مالی اور رپورٹنگ automação کے لیے `RevenueShareRecordV1` استعمال کریں۔

سوالات یا تبدیلی کی درخواستیں `roadmap.md` میں SNS اپ ڈیٹس کے ساتھ درج کریں اور mesclar کے وقت `status.md` میں شامل کریں۔