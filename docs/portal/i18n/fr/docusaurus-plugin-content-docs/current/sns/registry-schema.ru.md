---
lang: fr
direction: ltr
source: docs/portal/docs/sns/registry-schema.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Cette page ouvre le `docs/source/sns/registry_schema.md` et ouvre le portail de copie canonique. Il s'agit d'un projet intéressant pour les événements actuels.
:::

# Gestionnaire de schéma Sora Name Service (SN-2a)

**Status :** Noir 2026-03-24 -- publié dans la publication du programme SNS  
**Ссылка на roadmap :** SN-2a "Schéma de registre et disposition du stockage"  
**Область:** Développer la structure canonique Norito, qui gère le cycle de vie et le logiciel pour Sora Name Service (SNS), pour réaliser le registre et le registraire. Installez les paramètres des contrats, des SDK et des passerelles.

Ce document contient des schémas postaux pour le SN-2a, qui correspondent à :

1. Identificateurs et paramètres de dérivation (`SuffixId`, `NameHash`, sélecteurs de dérivation).
2. Structures/énumérations Norito pour les domaines d'activité, les suffixes politiques, les niveaux supérieurs, les maisons de retraite et les restaurants locaux.
3. Structure de mise en page et index des préférences pour la relecture.
4. Машину состояний, охватывающую регистрацию, proдление, grâce/rédemption, gel et pierre tombale.
5. Fonctions canoniques pour la configuration automatique du DNS/passerelle.

## 1. Identification et identification| Identificateur | Description | Proizvodna |
|------------|-------------|------------|
| `SuffixId` (`u16`) | La recherche d'identifiant pour les suffixes de votre entreprise (`.sora`, `.nexus`, `.dao`). Согласован с каталогом суфиксов в [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Назначается голосованием по gouvernance; хранится в `SuffixPolicyV1`. |
| `SuffixSelector` | Каноническая строковая форма суффикса (ASCII, minuscule). | Exemple : `.sora` -> `sora`. |
| `NameSelectorV1` | Le sélecteur binaire est activé. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Le code NFC + minuscules est conforme à la norme v1. |
| `NameHash` (`[u8;32]`) | Il y a généralement des contrats qui utilisent des contrats, des contrats et des contrats. | `blake3(NameSelectorV1_bytes)`. |

Détermination du problème :

- Les règles sont normalisées par la norme v1 (UTS-46 strict, STD3 ASCII, NFC). Les mouvements polycycliques du DOS doivent être normalisés avant l'opération.
- Les billets de banque (à partir de `SuffixPolicyV1.reserved_labels`) ne sont pas disponibles dans le dossier ; les remplacements de gouvernance uniquement выпускают события `ReservedNameAssigned`.

## 2. Structures Norito

### 2.1 NomEnregistrementV1| Pôle | Astuce | Première |
|-------|------|---------------|
| `suffix_id` | `u16` | Recherchez `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Ouvrez le sélecteur de batterie pour l'audio/debug. |
| `name_hash` | `[u8; 32]` | Pochette pour carte/boîte. |
| `normalized_label` | `AsciiString` | Il s'agit d'un texte (après Norm v1). |
| `display_label` | `AsciiString` | Boîtier de l'intendant ; cosmétique. |
| `owner` | `AccountId` | Управляет продлениями/transféramis. |
| `controllers` | `Vec<NameControllerV1>` | Recherches sur les comptes d'adresses, les résolveurs ou l'utilisation de métadonnées. |
| `status` | `NameStatus` | Флаг жизненного цикла (см. Раздел 4). |
| `pricing_class` | `u8` | Индекс в ценовых niveaux суффикса (standard, premium, réservé). |
| `registered_at` | `Timestamp` | Il s'agit d'un bloc d'activation automatique. |
| `expires_at` | `Timestamp` | Конец оплаченного срока. |
| `grace_expires_at` | `Timestamp` | Cela grâce au renouvellement automatique (par défaut +30 jours). |
| `redemption_expires_at` | `Timestamp` | Конец окна rachat (par défaut +60 дней). |
| `auction` | `Option<NameAuctionStateV1>` | Prise en compte de la réouverture des Pays-Bas ou des enchères premium. |
| `last_tx_hash` | `Hash` | Déterminez la version de transmission. |
| `metadata` | `Metadata` | Произвольная registraire de métadonnées (enregistrements de texte, preuves). |

Structure améliorée :

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

### 2.2 SuffixPolicyV1| Pôle | Astuce | Première |
|-------|------|---------------|
| `suffix_id` | `u16` | Первичный ключ; стабилен между версиями политики. |
| `suffix` | `AsciiString` | par exemple, `sora`. |
| `steward` | `AccountId` | Intendant, определенный в charte de gouvernance. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Le règlement actif est disponible (par exemple `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | Les cafés sont situés à des niveaux différents et offrent des services complets. |
| `min_term_years` | `u8` | Les demandes de paiement minimales incluent des informations sur les remplacements de niveau. |
| `grace_period_days` | `u16` | Par défaut 30. |
| `redemption_period_days` | `u16` | Par défaut 60. |
| `max_term_years` | `u8` | Le plus grand nombre de pré-plaques. |
| `referral_cap_bps` | `u16` | <=1000 (10%) par charter. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Список от gouvernance с инструкциями назначения. |
| `fee_split` | `SuffixFeeSplitV1` | Доли trésorerie / intendant / référence (points de base). |
| `fund_splitter_account` | `AccountId` | Аккаунт Escrow + распределение средств. |
| `policy_version` | `u16` | Увеличивается при каждом изменении. |
| `metadata` | `Metadata` | Paramètres précis (convention KPI, documents de hachage pour la conformité). |

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

### 2.3 Записи доходов и règlement| Structure | Pol | Назначение |
|--------|------|------------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Детерминированная запись распределенных выплат по эпохам règlement (неделя). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Эмитируется при каждом платеже (enregistrement, renouvellement, vente aux enchères). |

Le `TokenValue` utilise la codification physique canonique Norito avec les valeurs correspondantes au `SuffixPolicyV1`.

### 2.4 Restauration du restaurant

Les canons contiennent le journal de relecture des données pour l'automatisation et l'analyse DNS/passerelle.

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

Ils sont désormais créés dans le journal rejouable (par exemple, dans le domaine `RegistryEvents`) et enregistrés dans les flux de passerelle, ce qui permet aux caches DNS d'être invalidés dans le cadre du SLA précédent.

## 3. Présentation et index de la mise en page

| Clé | Description |
|-----|----------|
| `Names::<name_hash>` | Carte détaillée `name_hash` -> `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Index complet de l'interface utilisateur du portefeuille (pagination conviviale). |
| `NamesByLabel::<suffix_id, normalized_label>` | Обнаружение конфликтов, детерминированный поиск. |
| `SuffixPolicies::<suffix_id>` | Actuel `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | Histoire `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Journal en ajout uniquement avec le suivi monotone. |Les clés de série sont les tuples Norito pour la détermination de votre hébergement. Les informations relatives à l'enregistrement automatique correspondent à un enregistrement automatique.

## 4. La machine à laver le vélo| Service | Условия входа | Pré-commandes | Première |
|-------|----------------|-----------|------------|
| Disponible | En conséquence, le code `NameRecord` est disponible. | `PendingAuction` (premium), `Active` (enregistrement standard). | Il est possible que les index soient complets. |
| En attente d'enchères | Donc, c'est `PriceTierV1.auction_kind` != aucun. | `Active` (règlement des enchères), `Tombstoned` (aucune offre). | Les enchères émettent `AuctionOpened` et `AuctionSettled`. |
| Actif | L'enregistrement ou la livraison sont possibles. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` est disponible avant. |
| Période de grâce | Automatique pour `now > expires_at`. | `Active` (renouvellement à temps), `Redemption`, `Tombstoned`. | Par défaut +30 jours ; резолвится, но помечено. |
| Rédemption | `now > grace_expires_at` et `< redemption_expires_at`. | `Active` (renouvellement tardif), `Tombstoned`. | Les commandes s'attaquent à la plate-forme stratégique. |
| Congelé | Gel de la gouvernance ou du tuteur. | `Active` (après correction), `Tombstoned`. | Ne pas remplacer ou remplacer les contrôleurs. |
| Tombé | Il s'agit d'une affaire ou d'une rédemption historique. | `PendingAuction` (réouverture aux Pays-Bas) ou остается tombstoned. | L'appareil `NameTombstoned` doit toujours être activé. |Avant que le DOS émette le code `RegistryEventKind`, les caches en aval sont installés. Tombstoned имена, входящие в Dutch rouvrir аукционы, прикрепляют payload `AuctionKind::DutchReopen`.

## 5. Passerelle canonique et synchronisation

Les passerelles compatibles avec `RegistryEventV1` et synchronisation DNS/SoraFS correspondent :

1. Connectez-vous après `NameRecordV1` pour que votre appareil soit correctement installé.
2. Modèles de résolution de résolution (I105 précédemment + compressé (`sora`) pour votre utilisateur, enregistrements de texte).
3. Épinglez les zones actuelles du workflow SoraDNS via [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Garanties de livraison applicables :

- Lorsque le transfert s'effectue sur `NameRecordV1`, *dollжна* doit s'assurer de votre sécurité avec le courant `version`.
- `RevenueSharePosted` concerne les colonies à partir de `RevenueShareRecordV1`.
- Le gel/dégel/tombstone inclut les éléments de gouvernance dans `metadata` pour la relecture d'audit.

## 6. Exemples de charges utiles Norito

### 6.1 Exemple NameRecord

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "soraカタカナ...",
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

### 6.2 Exemple SuffixPolicy

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "soraカタカナ...",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("soraカタカナ..."), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "soraカタカナ...",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. Les petits chats- **SN-2b (API du registraire et hooks de gouvernance) :** ouvrez ces structures à partir de Torii (Norito et liaisons JSON) et autorisez les contrôles d'admission des éléments de gouvernance.
- **SN-3 (moteur d'enchères et d'enregistrement) :** переиспользовать `NameAuctionStateV1` pour les logiciels de validation/révélation et de réouverture néerlandaise.
- **SN-5 (Paiement et règlement) :** utilisez `RevenueShareRecordV1` pour les transactions financières et les opérations automatiques.

Les projets et projets de création doivent être adaptés à la mise à jour de la feuille de route SNS dans `roadmap.md` et mis à jour dans `status.md`. слиянии.