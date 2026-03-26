---
lang: fr
direction: ltr
source: docs/portal/docs/sns/registry-schema.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sns/registry_schema.md` کی عکاسی کرتا ہے اور پورٹل کی کینونیکل کاپی کے طور پر کام کرتا ہے۔ سورس فائل ترجمہ اپ ڈیٹس کے لیے برقرار رہے گی۔
:::

# Service de noms Sora رجسٹری اسکیمہ (SN-2a)

**حیثیت:** Mercredi 2026-03-24 -- SNS پروگرام ریویو کے لیے جمع کیا گیا  
**روڈمیپ لنک:** SN-2a "Schéma du registre et disposition du stockage"  
**دائرہ:** Sora Name Service (SNS) est disponible pour Norito. Il existe un registre et un bureau d'enregistrement, des implémentations, des SDK et des passerelles et des solutions déterministes.

Le SN-2a est doté d'un système d'alimentation en carburant et d'un système d'alimentation en carburant:

1. Méthode de hachage (`SuffixId`, `NameHash`, dérivation du sélecteur)
2. نام ریکارڈز، suffixe پالیسیز، قیمت tiers, ریونیو سپلٹس اور رجسٹری ایونٹس کے لیے Norito structures/énumérations۔
3. relecture déterministe avec disposition du stockage et préfixes d'index
4. رجسٹریشن، تجدید، grace/redemption، freeze اور tombstone پر مشتمل state machine۔
5. Automatisation DNS/passerelle et événements canoniques

## 1. شناختیں اور hachage| شناخت | وضاحت | اخذ |
|------------|-------------|------------|
| `SuffixId` (`u16`) | ٹاپ suffixes لیول (`.sora`, `.nexus`, `.dao`) کے لیے رجسٹری شناخت۔ [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) کے catalogue de suffixes کے مطابق۔ | vote sur la gouvernance سے مختص; `SuffixPolicyV1` میں محفوظ۔ |
| `SuffixSelector` | suffixe کی chaîne canonique شکل (ASCII, minuscule)۔ | Utiliser : `.sora` -> `sora`. |
| `NameSelectorV1` | رجسٹر شدہ لیبل کے لیے بائنری sélecteur۔ | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Pour Norm v1, c'est NFC + minuscules |
| `NameHash` (`[u8;32]`) | Les caches et les caches de clé | `blake3(NameSelectorV1_bytes)`. |

Déterminisme کی ضروریات:

- La norme v1 (UTS-46 stricte, STD3 ASCII, NFC) permet de normaliser la norme. صارف کی strings کو hachage سے پہلے normalize کرنا لازم ہے۔
- Étiquettes réservées (`SuffixPolicyV1.reserved_labels`) Registre des étiquettes réservées aux utilisateurs remplacements de gouvernance uniquement `ReservedNameAssigned` ایونٹس خارج کرتے ہیں۔

## 2. Norito ساختیں

### 2.1 NomEnregistrementV1| فیلڈ | قسم | نوٹس |
|-------|------|-------|
| `suffix_id` | `u16` | `SuffixPolicyV1` کی ریفرنس۔ |
| `selector` | `NameSelectorV1` | audit/débogage کے لیے octets de sélecteur brut۔ |
| `name_hash` | `[u8; 32]` | cartes/événements کے لیے clé۔ |
| `normalized_label` | `AsciiString` | انسانی قابلِ پڑھائی لیبل (Norm v1 کے بعد)۔ |
| `display_label` | `AsciiString` | steward کی boîtier؛ اختیاری cosmétiques۔ |
| `owner` | `AccountId` | renouvellements/transferts کنٹرول کرتا ہے۔ |
| `controllers` | `Vec<NameControllerV1>` | Il y a des résolveurs et des métadonnées et des résolveurs |
| `status` | `NameStatus` | لائف سائیکل فلیگ (سیکشن 4 دیکھیں)۔ |
| `pricing_class` | `u8` | suffixe کے niveaux de prix کا indice (standard, premium, réservé)۔ |
| `registered_at` | `Timestamp` | ابتدائی activation کا بلاک ٹائم۔ |
| `expires_at` | `Timestamp` | ادا شدہ مدت کا اختتام۔ |
| `grace_expires_at` | `Timestamp` | grâce au renouvellement automatique کا اختتام (par défaut +30 دن)۔ |
| `redemption_expires_at` | `Timestamp` | fenêtre de remboursement کا اختتام (par défaut +60 دن)۔ |
| `auction` | `Option<NameAuctionStateV1>` | Les Pays-Bas rouvrent leurs enchères premium |
| `last_tx_hash` | `Hash` | Il s'agit d'un pointeur déterministe et d'un pointeur déterministe. |
| `metadata` | `Metadata` | registraire کی métadonnées arbitrairement (enregistrements de texte, preuves)۔ |

Structures similaires :

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

### 2.2 SuffixPolicyV1| فیلڈ | قسم | نوٹس |
|-------|------|-------|
| `suffix_id` | `u16` | بنیادی clé؛ پالیسی ورژنز میں مستحکم۔ |
| `suffix` | `AsciiString` | مثال کے طور پر `sora`۔ |
| `steward` | `AccountId` | charte de gouvernance میں متعین steward۔ |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | identifiant d'actif de règlement par défaut (مثلا `61CtjvNd9T3THAR65GsMVHr82Bjc`)۔ |
| `pricing` | `Vec<PriceTierV1>` | قیمت کے niveaux کے coefficients اور مدت کے قواعد۔ |
| `min_term_years` | `u8` | خریدی گئی مدت کے لیے کم از کم حد۔ |
| `grace_period_days` | `u16` | Par défaut 30. |
| `redemption_period_days` | `u16` | Par défaut 60. |
| `max_term_years` | `u8` | پیشگی تجدید کی زیادہ سے زیادہ مدت۔ |
| `referral_cap_bps` | `u16` | <=1000 (10%) charter کے مطابق۔ |
| `reserved_labels` | `Vec<ReservedNameV1>` | gouvernance کی فراہم کردہ فہرست مع attribuer ہدایات۔ |
| `fee_split` | `SuffixFeeSplitV1` | trésorerie / intendant / référence حصص (points de base)۔ |
| `fund_splitter_account` | `AccountId` | Escrow رکھنے اور فنڈز تقسیم کرنے والا اکاؤنٹ۔ |
| `policy_version` | `u16` | ہر تبدیلی پر بڑھتا ہے۔ |
| `metadata` | `Metadata` | توسیعی نوٹس (accord KPI, hachages de documents de conformité)۔ |

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

### 2.3 ریونیو اور règlement ریکارڈز| Structure | فیلڈز | مقصد |
|--------|-------|------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | époque de règlement (ہفتہ وار) کے حساب سے acheminé ادائیگیوں کا déterministe ریکارڈ۔ |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | ہر ادائیگی پوسٹ ہونے پر émettre (enregistrement, renouvellement, vente aux enchères)۔ |

Le code `TokenValue` correspond au code Norito pour le codage canonique à virgule fixe. `SuffixPolicyV1` میں déclare ہوتا ہے۔

### 2.4 رجسٹری ایونٹس

Événements canoniques Automatisation DNS/passerelle et analyses et journal de relecture

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

Il y a un journal rejouable (domaine `RegistryEvents`) qui ajoute un lien vers les flux de passerelle et un miroir pour les caches DNS SLA et les caches DNS. اندر invalider ہوں۔

## 3. Disposition du stockage et index

| Clé | وضاحت |
|-----|-------------|
| `Names::<name_hash>` | `name_hash` et `NameRecordV1` Carte détaillée |
| `NamesByOwner::<AccountId, suffix_id>` | interface utilisateur du portefeuille et index (pagination conviviale) |
| `NamesByLabel::<suffix_id, normalized_label>` | les conflits détectent کرتا ہے اور recherche déterministe فعال بناتا ہے۔ |
| `SuffixPolicies::<suffix_id>` | Il s'agit de `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` ہسٹری۔ |
| `RegistryEvents::<u64>` | journal en ajout uniquement جس کی séquence de touches monotone ہے۔ |Les clés Norito tuples sont sérialisées et les hôtes sont utilisés pour le hachage déterministe. mises à jour de l'index de manière atomique

## 4. Machine à états du cycle de vie

| État | Conditions d'entrée | Transitions autorisées | Remarques |
|-------|--------|----------|-------|
| Disponible | Pour `NameRecord` موجود نہ ہو۔ | `PendingAuction` (premium), `Active` (enregistrement standard). | recherche de disponibilité صرف index پڑھتی ہے۔ |
| En attente d'enchères | Par `PriceTierV1.auction_kind` != aucun ہو۔ | `Active` (règlement des enchères), `Tombstoned` (aucune offre). | enchères `AuctionOpened` et `AuctionSettled` émettent کرتی ہیں۔ |
| Actif | رجسٹریشن یا تجدید کامیاب ہو۔ | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` transition چلاتا ہے۔ |
| Période de grâce | par `now > expires_at` ہو۔ | `Active` (renouvellement à temps), `Redemption`, `Tombstoned`. | Par défaut +30 dollars ; résoudre ہوتا ہے مگر flag ہوتا ہے۔ |
| Rédemption | `now > grace_expires_at` et `< redemption_expires_at`. | `Active` (renouvellement tardif), `Tombstoned`. | Frais de pénalité درکار ہے۔ |
| Congelé | gouvernance یا gel des gardiens۔ | `Active` (remédiation ici), `Tombstoned`. | transfert et mise à jour des contrôleurs |
| Tombé | رضاکارانہ reddition، مستقل dispute نتیجہ، یا rédemption ختم۔ | `PendingAuction` (réouverture aux Pays-Bas) یا tombstoned رہتا ہے۔ | `NameTombstoned` ایونٹ میں وجہ شامل ہونی چاہیے۔ |Les transitions d'état par exemple `RegistryEventKind` émettent des caches en aval et des caches en aval Tombstoned نام جو Réouverture des enchères aux Pays-Bas et la charge utile `AuctionKind::DutchReopen` est disponible en ligne.

## 5. Événements canoniques et synchronisation de la passerelle

Passerelles `RegistryEventV1` pour la synchronisation avec DNS/SoraFS pour la synchronisation :

1. Séquence de séquence میں حوالہ کردہ تازہ ترین `NameRecordV1` حاصل کریں۔
2. modèles de résolveur دوبارہ بنائیں (i105 ترجیحی + compressé (`sora`) deuxième meilleure adresse, enregistrements texte)۔
3. [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) Pour le flux de travail SoraDNS et la broche de données de zone.

Garanties de livraison d’événements :

- ہر ٹرانزیکشن جو `NameRecordV1` پر اثر ڈالے *لازمی* طور پر `version` کے ساتھ صرف ایک ایونٹ شامل کرے جو سختی سے بڑھتی ہو۔
- `RevenueSharePosted` et `RevenueShareRecordV1` pour les règlements émis et la référence de l'article.
- geler/dégeler/tombstone pour la relecture d'audit et `metadata` pour les hachages d'artefacts de gouvernance.

## 6. Charges utiles Norito en cours

### 6.1 NameRecord Mise à jour

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "<i105-account-id>",
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

### 6.2 SuffixPolicy Plus

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "<i105-account-id>",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("<i105-account-id>"), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "<i105-account-id>",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. اگلے اقدامات- **SN-2b (API du registraire et hooks de gouvernance) :** les structures et les Torii permettent d'exposer les éléments (Norito et les liaisons JSON) et les contrôles d'admission et les artefacts de gouvernance. جوڑیں۔
- **SN-3 (moteur d'enchères et d'enregistrement) :** commit/révélation et logique de réouverture néerlandaise `NameAuctionStateV1` دوبارہ استعمال کریں۔
- **SN-5 (Paiement et règlement) :** Pour le rapprochement et l'automatisation du paiement `RevenueShareRecordV1` استعمال کریں۔

Les réseaux sociaux `roadmap.md` pour SNS sont en cours de fusion. `status.md` میں شامل کریں۔