---
lang: he
direction: rtl
source: docs/portal/docs/sns/registry-schema.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::הערה מקור קנוניק
Cette page reflete `docs/source/sns/registry_schema.md` et sert desormais de copie canonique du portail. Le fichier source reste pour les mises a jour de traduction.
:::

# Schema du registre Sora Name Service (SN-2a)

**סטוט:** Redige 2026-03-24 -- soumis a la revue du program SNS  
**מפת דרכים של שעבוד:** SN-2a "סכימת רישום ופריסת אחסון"  
**Portee:** מגדירים את המבנים Norito קנוניים, את המחזוריות והאירועים של Sora Name Service (SNS) או את מימושי הרישום ואת הרשם המבוססים על קונטרסטים, SDKs ושערים.

Ce document complete le livrable de schema pour SN-2a במדויק:

1. Identifiants et regles de hashing (`SuffixId`, `NameHash`, derivation des selecteurs).
2. מבנים/מבנים Norito pour les enregistrements de noms, politiques de suffixes, tiers de prix, repartitions de revenus et evenements du registre.
3. Layout de stockage et prefixes d'indexes pour un replay deterministe.
4. Une machine d'etats couvrant l'enregistrement, le renovellement, חסד/גאולה, הקפאה ומצבות.
5. Evenements canoniques consommes par l'automatisation DNS/gateway.

## 1. זיהויים ו-hashing

| מזהה | תיאור | גזירה |
|------------|----------------|-------------|
| `SuffixId` (`u16`) | Identifiant de registre pour les suffixes de premier niveau (`.sora`, `.nexus`, `.dao`). Aligne sur le catalog des suffixes dans [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Attribue par vote de governance; stocke dans `SuffixPolicyV1`. |
| `SuffixSelector` | Forme canonique en chaine du suffixe (ASCII, אותיות קטנות). | דוגמה: `.sora` -> `sora`. |
| `NameSelectorV1` | Selecteur binaire pour le label enregistre. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. תווית NFC + תווית אות קטנה Norm v1. |
| `NameHash` (`[u8;32]`) | Cle primaire de recherche utilisee par contrats, evenements et caches. | `blake3(NameSelectorV1_bytes)`. |

דרישות דטרמיניזם:

- התוויות של Les sont מתנרמלות באמצעות Norm v1 (UTS-46 strict, STD3 ASCII, NFC). Les chaines utilisateur DOIVENT etre normalisees avant le hash.
- Les labels reserves (de `SuffixPolicyV1.reserved_labels`) n'entrent jamais dans le registre; les overrides ייחודיות Governance emettent des evenements `ReservedNameAssigned`.

## 2. מבנים Norito

### 2.1 NameRecordV1| אלוף | הקלד | הערות |
|-------|------|-------|
| `suffix_id` | `u16` | הפניה `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Octets du selecteur brut pour audit/debug. |
| `name_hash` | `[u8; 32]` | Cle pour מפות/אירועים. |
| `normalized_label` | `AsciiString` | תווית ליסible pour l'humain (פוסט נורמה v1). |
| `display_label` | `AsciiString` | מארז fourni par le steward; קוסמטיקה optionnelle. |
| `owner` | `AccountId` | Controle les renouvellements/העברות. |
| `controllers` | `Vec<NameControllerV1>` | הפניות לעומת כתובות תקשורות, פותרות או מטא נתונים של יישום. |
| `status` | `NameStatus` | Indicateur de cycle de vie (בפרק 4). |
| `pricing_class` | `u8` | Index dans les tiers de prix du suffixe (סטנדרט, פרימיום, שמור). |
| `registered_at` | `Timestamp` | חותמת זמן של גוש ההפעלה ראשי תיבות. |
| `expires_at` | `Timestamp` | Fin du terme paye. |
| `grace_expires_at` | `Timestamp` | Fin de grace d'auto-renouvellement (ברירת מחדל +30 ז'ור). |
| `redemption_expires_at` | `Timestamp` | Fin de la fenetre de redemption (ברירת מחדל +60 ז'ור). |
| `auction` | `Option<NameAuctionStateV1>` | הווה quand des Dutch reopen ou encheres premium sont actives. |
| `last_tx_hash` | `Hash` | Pointeur deterministe vers la transaction qui a produit cette גרסה. |
| `metadata` | `Metadata` | Metadata arbitraire du registrar (רשומות טקסט, הוכחות). |

מבני תמיכה:

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

### 2.2 SuffixPolicyV1

| אלוף | הקלד | הערות |
|-------|------|-------|
| `suffix_id` | `u16` | Cle primaire; גרסאות הפוליטיקה היציבות. |
| `suffix` | `AsciiString` | לדוגמה, `sora`. |
| `steward` | `AccountId` | דייל defini dans le charter de governance. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Identifiant d'actif de settlement par defaut (למשל `xor#sora`). |
| `pricing` | `Vec<PriceTierV1>` | מקדמים של מחירים ותקנות. |
| `min_term_years` | `u8` | Plancher pour le terme achete quel que soit l'override de tier. |
| `grace_period_days` | `u16` | ברירת מחדל 30. |
| `redemption_period_days` | `u16` | ברירת מחדל 60. |
| `max_term_years` | `u8` | מקסימום דה רנובלמנט מראש. |
| `referral_cap_bps` | `u16` | <=1000 (10%) סלון לה צ'רטר. |
| `reserved_labels` | `Vec<ReservedNameV1>` | רשימת פורני פר לה גוברנס עם הוראות אהבה. |
| `fee_split` | `SuffixFeeSplitV1` | Tresorerie חלקים / דייל / הפניה (נקודות בסיס). |
| `fund_splitter_account` | `AccountId` | Compte qui detient l'escrow + distribue les fonds. |
| `policy_version` | `u16` | הגדל שינוי צ'אק. |
| `metadata` | `Metadata` | הערות תקצירים (אמנת KPI, hashes de docs de compliance). |

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

### 2.3 רישומים של רווח והסדר| מבנה | Champs | אבל |
|--------|--------|-----|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, Norito, Norito. | Enregistrement deterministe des paiements routes par epoque de settlement (hebdomadaire). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Emis a chaque paiement poste (רישום, renovellement, enchere). |

Tous les champs `TokenValue` utilisent l'encodage fixe canonique de Norito avec le code devise declare dans le `SuffixPolicyV1` associate.

### 2.4 Evenements du registre

Les evenements canoniques fournissent un log de replay pour l'automatization DNS/gateway et l'analytique.

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

Les evenements doivent etre ajoutes un log rejouable (למשל, le domaine `RegistryEvents`) and repletes vers les feeds gateway pour que les caches DNS invalid in les SLA.

## 3. פריסת מלאי ואינדקסים

| קל | תיאור |
|-----|-------------|
| `Names::<name_hash>` | Map primaire de `name_hash` לעומת `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | אינדקס משני לשפוך ארנק ממשק משתמש (ידידותי לעיון). |
| `NamesByLabel::<suffix_id, normalized_label>` | גלה לסכסוכים, alimente la recherche deterministe. |
| `SuffixPolicies::<suffix_id>` | Dernier `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | היסטורי `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | יומן הוספה בלבד ברצף cle par monotone. |

Toutes les cles sont serialisees via des tuples Norito pour garder un hashing deterministe entre hotes. Les mises a jour d'index se font atomiquement avec l'enregistrement main.

## 4. Machine d'etats du cycle de vie

| אטאת | תנאי מנה | היתרי מעברים | הערות |
|-------|--------------------|------------------------|-------|
| זמין | נגזר quand `NameRecord` נעדר. | `PendingAuction` (פרימיום), `Active` (תקן רישום). | La recherche de disponibilite lit seulement les indexes. |
| מכירה פומבית בהמתנה | Cree quand `PriceTierV1.auction_kind` != אין. | `Active` (enchere reglee), `Tombstoned` (aucune enchere). | Les encheres emettent `AuctionOpened` et `AuctionSettled`. |
| פעיל | הרשמה או חידוש מחדש. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` pilote la transition. |
| תקופת חסד | קוואנד אוטומטי `now > expires_at`. | `Active` (חידוש זמני), `Redemption`, `Tombstoned`. | ברירת מחדל +30 יו"ר; resolu mais marque. |
| גאולה | `now > grace_expires_at` mais `< redemption_expires_at`. | `Active` (רנובלמנט טרדיף), `Tombstoned`. | Les commandes exigent des frais de penalite. |
| קפוא | הקפאת השלטון או האפוטרופוס. | `Active` (תיקון אפרה), `Tombstoned`. | אין צורך להעביר בקרים. |
| מצבה | לנטוש את הוולונטייר, תוצאה דה ליטיג לצמיתות, או הגאולה תפוג. | `PendingAuction` (פתיחה מחדש בהולנד) או שאר המצבה. | L'evenement `NameTombstoned` doit inclure une raison. |Les transitions d'etat DOIVENT emettre le `RegistryEventKind` correspondant pour que les caches downstream restent coherentes. משתתף Les noms tombstones encheres הולנדית פתיחה מחדש של קובץ מצורף ללא מטען `AuctionKind::DutchReopen`.

## 5. שער סנכרון קנוני אירועי ערב

השערים נרשמים ל-`RegistryEventV1` ו-DNS/SoraFS סינכרון באמצעות:

1. Recuperer le dernier `NameRecordV1` התייחסות par la sequence d'evenements.
2. Regenerer les templates de resolver (כתובות IH58 העדיפות + דחוסות (`sora`) ובחירה שנייה, רשומות טקסט).
3. Pinner les donnees de zone mises a jour via le workflow SoraDNS decrit dans [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

אחריות לאירועים:

- עסקה צ'אקית משפיעה על `NameRecordV1` *doit* ajouter exactement un evenement avec `version` strictement croissante.
- Les evenements `RevenueSharePosted` referencent les settlements emis par `RevenueShareRecordV1`.
- Les evenements freeze/unfreeze/tombstone incluent les hashes d'artefacts de governance dans `metadata` pour le replay d'audit.

## 6. דוגמאות למטענים Norito

### 6.1 דוגמה לרשומה של שם

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "ih58...",
    controllers: [
        NameControllerV1 {
            controller_type: Account,
            account_address: Some(AccountAddress("0x02000001...")),
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

### 6.2 מדיניות סיומת לדוגמה

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "ih58...",
    status: Active,
    payment_asset_id: "xor#sora",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("ih58..."), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "ih58...",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. Prochaines etapes

- **SN-2b (Registrar API & Governance Hooks):** חושף מבנים דרך Torii (קשרים Norito ו-JSON) ומחברים לבדיקות כניסת חפצי אומנות.
- **SN-3 (מנוע מכירה פומבית ורישום):** ניצול מחדש `NameAuctionStateV1` יישם את הלוגיקה של התחייבות/חשיפה ופתיחה מחדש של הולנדית.
- **SN-5 (תשלום והסדר):** מנצל `RevenueShareRecordV1` pour la reconciliation financiere et l'automatisation des rapports.

Les שאלות או דורשות את השינוי doivent etre deposees avec les mises a jour du מפת הדרכים SNS dans `roadmap.md` et refletees dans `status.md` lors de la fusion.