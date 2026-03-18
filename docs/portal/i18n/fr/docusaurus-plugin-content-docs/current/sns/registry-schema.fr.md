---
lang: fr
direction: ltr
source: docs/portal/docs/sns/registry-schema.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Source canonique
Cette page reflète `docs/source/sns/registry_schema.md` et sert désormais de copie canonique du portail. Le fichier source reste pour les mises à jour de traduction.
:::

# Schéma du registre Sora Name Service (SN-2a)

**Statut:** Redige 2026-03-24 -- soumis à la revue du programme SNS  
**Feuille de route des liens :** SN-2a "Schéma de registre et disposition du stockage"  
**Portée :** Définir les structures Norito canoniques, les états de cycle de vie et les événements émis pour le Sora Name Service (SNS) afin que les implémentations de registre et de registrar restent déterministes dans les contrats, SDK et gateways.

Ce document complet le livrable de schéma pour SN-2a en précisant :

1. Identifiants et règles de hachage (`SuffixId`, `NameHash`, dérivation des sélecteurs).
2. Structs/enums Norito pour les enregistrements de noms, politiques de suffixes, niveaux de prix, répartitions de revenus et événements du registre.
3. Disposition de stockage et préfixes d'index pour une relecture déterministe.
4. Une machine d'états comprenant l'enregistrement, le renouvellement, la grâce/rédemption, les gels et les pierres tombales.
5. Événements canoniques consommés par l'automatisation DNS/gateway.

## 1. Identifiants et hachage| Identifiant | Descriptif | Dérivation |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Identifiant de registre pour les suffixes de premier niveau (`.sora`, `.nexus`, `.dao`). Alignez sur le catalogue des suffixes dans [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Attribute par vote de gouvernance; stocké dans `SuffixPolicyV1`. |
| `SuffixSelector` | Forme canonique en chaîne du suffixe (ASCII, minuscule). | Exemple : `.sora` -> `sora`. |
| `NameSelectorV1` | Sélecteur binaire pour le label enregistré. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Le label est NFC + minuscule selon la norme v1. |
| `NameHash` (`[u8;32]`) | Clé primaire de recherche utilisée par contrats, événements et caches. | `blake3(NameSelectorV1_bytes)`. |

Exigences de déterminisme :

- Les labels sont normalisés via la Norme v1 (UTS-46 strict, STD3 ASCII, NFC). Les chaînes utilisateur DOIVENT sont normalisées avant le hachage.
- Les labels réserves (de `SuffixPolicyV1.reserved_labels`) n'entrent jamais dans le registre ; les overrides uniquement gouvernance emettent des événements `ReservedNameAssigned`.

## 2. Structures Norito

### 2.1 NomEnregistrementV1| Champion | Tapez | Remarques |
|-------|------|-------|
| `suffix_id` | `u16` | Référence `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Octets du sélecteur brut pour audit/debug. |
| `name_hash` | `[u8; 32]` | Clé pour cartes/événements. |
| `normalized_label` | `AsciiString` | Label lisible pour l'humain (post Norm v1). |
| `display_label` | `AsciiString` | Boîtier fourni par le steward; cosmétique optionnelle. |
| `owner` | `AccountId` | Contrôler les renouvellements/transferts. |
| `controllers` | `Vec<NameControllerV1>` | Références vers des adresses de compte cibles, résolveurs ou métadonnées d'application. |
| `status` | `NameStatus` | Indicateur de cycle de vie (voir Section 4). |
| `pricing_class` | `u8` | Index dans les niveaux de prix du suffixe (standard, premium, réservé). |
| `registered_at` | `Timestamp` | Timestamp du bloc de l’activation initiale. |
| `expires_at` | `Timestamp` | Fin du terme payant. |
| `grace_expires_at` | `Timestamp` | Fin de grâce d'auto-renouvellement (par défaut +30 jours). |
| `redemption_expires_at` | `Timestamp` | Fin de la fenêtre de rachat (par défaut +60 jours). |
| `auction` | `Option<NameAuctionStateV1>` | Présent quand les Néerlandais rouvrent ou enchères premium sont actifs. |
| `last_tx_hash` | `Hash` | Pointeur déterministe vers la transaction qui a produit cette version. || `metadata` | `Metadata` | Metadata arbitraire du registrar (enregistrements texte, preuves). |

Structures de support :

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

### 2.2 SuffixPolicyV1| Champion | Tapez | Remarques |
|-------|------|-------|
| `suffix_id` | `u16` | Clé primaire; stable entre versions de politique. |
| `suffix` | `AsciiString` | par exemple, `sora`. |
| `steward` | `AccountId` | Steward défini dans la charte de gouvernance. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Identifiant d'actif de règlement par défaut (par exemple `xor#sora`). |
| `pricing` | `Vec<PriceTierV1>` | Coefficients de prix par niveaux et règles de durée. |
| `min_term_years` | `u8` | Plancher pour le terme acheté quel que soit l'override de tier. |
| `grace_period_days` | `u16` | Par défaut 30. |
| `redemption_period_days` | `u16` | Par défaut 60. |
| `max_term_years` | `u8` | Anticipation maximale de renouvellement. |
| `referral_cap_bps` | `u16` | <=1000 (10%) selon la charte. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Liste fournie par la gouvernance avec instructions d'affectation. |
| `fee_split` | `SuffixFeeSplitV1` | Trésorerie de pièces / steward / référencement (points de base). |
| `fund_splitter_account` | `AccountId` | Compte qui détient l'escrow + distribue les fonds. |
| `policy_version` | `u16` | Incrémentez à chaque changement. |
| `metadata` | `Metadata` | Notes étendues (KPI covenant, hashs de docs de conformité). |

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
```### 2.3 Enregistrements de revenus et de règlement

| Structure | Champions | Mais |
|--------|--------|-----|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Enregistrement déterministe des routes de paiements par époque de règlement (hebdomadaire). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Emis a chaque poste de paiement (enregistrement, renouvellement, enchere). |

Tous les champs `TokenValue` utilisent l'encodage fixe canonique de Norito avec le code devise déclarer dans le `SuffixPolicyV1` associé.

### 2.4 Événements du registre

Les événements canoniques fournissent un log de replay pour l'automatisation DNS/gateway et l'analyse.

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

Les événements doivent être ajoutés un log rejouable (par exemple, le domaine `RegistryEvents`) et reflétés vers les flux gateway pour que les caches DNS invalident dans les SLA.

## 3. Disposition de stockage et index| Clé | Descriptif |
|-----|-------------|
| `Names::<name_hash>` | Carte primaire de `name_hash` vers `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Index secondaire pour UI wallet (pagination conviviale). |
| `NamesByLabel::<suffix_id, normalized_label>` | Détecter les conflits, alimenter la recherche déterministe. |
| `SuffixPolicies::<suffix_id>` | Dernier `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | Historique `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Journal d'ajout uniquement clé par séquence monotone. |

Tous les clés sont sérialisés via des tuples Norito pour garder un hachage déterministe entre les hôtes. Les mises à jour d'index se font atomiquement avec l'enregistrement principal.

## 4. Machine d'états du cycle de vie| État | Conditions d'entrée | Transitions permises | Remarques |
|-------|----------|-----------|-------|
| Disponible | Dérive quand `NameRecord` est absent. | `PendingAuction` (premium), `Active` (standard d'enregistrement). | La recherche de disponibilité lit seulement les index. |
| En attente d'enchères | Cri quand `PriceTierV1.auction_kind` != aucun. | `Active` (enchère réglée), `Tombstoned` (aucune enchère). | Les enchères emettent `AuctionOpened` et `AuctionSettled`. |
| Actif | Enregistrement ou renouvellement réussi. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` pilote la transition. |
| Période de grâce | Automatique quand `now > expires_at`. | `Active` (renouvellement à temps), `Redemption`, `Tombstoned`. | Par défaut +30 jours ; résolument plus marque. |
| Rédemption | `now > grace_expires_at` mais `< redemption_expires_at`. | `Active` (renouvellement tardif), `Tombstoned`. | Les commandes exigeant des frais de pénalité. |
| Congelé | Freeze de gouvernance ou tuteur. | `Active` (après assainissement), `Tombstoned`. | Ne peut pas transférer ni mettre à jour les contrôleurs. |
| Tombé | Abandon volontaire, résultat d'un litige permanent, ou rachat expiré. | `PendingAuction` (réouverture aux Pays-Bas) ou reste tombé. | L'événement `NameTombstoned` doit inclure une raison. |Les transitions d'état DOIVENT émettre le `RegistryEventKind` correspondant pour que les caches restent en aval cohérentes. Les noms tombstoned entrant en encheres Dutch rouvrir attachent une charge utile `AuctionKind::DutchReopen`.

## 5. Événements canoniques et sync gateway

Les passerelles s'abonnent à `RegistryEventV1` et synchronisent DNS/SoraFS via :

1. Récupérer le dernier `NameRecordV1` référence par la séquence d'événements.
2. Régénérer les modèles de résolveur (adresses I105 préférées + compressées (`sora`) en deuxième choix, enregistrements texte).
3. Pinner les données de zone mises à jour via le workflow SoraDNS décrit dans [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Garanties de livraison d'événements :

- Chaque transaction qui affecte un `NameRecordV1` *doit* ajouter exactement un événement avec `version` strictement croissant.
- Les événements `RevenueSharePosted` référencent les colonies émises par `RevenueShareRecordV1`.
- Les événements freeze/unfreeze/tombstone incluent les hashes d'artefacts de gouvernance dans `metadata` pour le replay d'audit.

## 6. Exemples de charges utiles Norito

### 6.1 Exemple NameRecord

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "i105...",
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
    steward: "i105...",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("i105..."), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "i105...",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. Prochaines étapes- **SN-2b (Registrar API & gouvernance hooks) :** exposer ces structs via Torii (bindings Norito et JSON) et connecter les contrôles d'admission aux artefacts de gouvernance.
- **SN-3 (Auction & Registration Engine) :** réutilisez `NameAuctionStateV1` pour implémenter la logique commit/reveal et Dutch open.
- **SN-5 (Paiement & règlement) :** exploiteur `RevenueShareRecordV1` pour la réconciliation financière et l'automatisation des rapports.

Les questions ou demandes de changement doivent être déposées avec les mises à jour du roadmap SNS dans `roadmap.md` et reflétées dans `status.md` lors de la fusion.