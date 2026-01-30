---
lang: ur
direction: rtl
source: docs/portal/docs/sns/registry-schema.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Source canonique
Cette page reflete `docs/source/sns/registry_schema.md` et sert desormais de copie canonique du portail. Le fichier source reste pour les mises a jour de traduction.
:::

# Schema du registre Sora Name Service (SN-2a)

**Statut:** Redige 2026-03-24 -- soumis a la revue du programme SNS  
**Lien roadmap:** SN-2a "Registry schema & storage layout"  
**Portee:** Definir les structures Norito canoniques, les etats de cycle de vie et les evenements emis pour le Sora Name Service (SNS) afin que les implementations de registre et de registrar restent deterministes dans les contrats, SDKs et gateways.

Ce document complete le livrable de schema pour SN-2a en precisant:

1. Identifiants et regles de hashing (`SuffixId`, `NameHash`, derivation des selecteurs).
2. Structs/enums Norito pour les enregistrements de noms, politiques de suffixes, tiers de prix, repartitions de revenus et evenements du registre.
3. Layout de stockage et prefixes d'indexes pour un replay deterministe.
4. Une machine d'etats couvrant l'enregistrement, le renouvellement, grace/redemption, freezes et tombstones.
5. Evenements canoniques consommes par l'automatisation DNS/gateway.

## 1. Identifiants et hashing

| Identifiant | Description | Derivation |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Identifiant de registre pour les suffixes de premier niveau (`.sora`, `.nexus`, `.dao`). Aligne sur le catalogue des suffixes dans [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Attribue par vote de gouvernance; stocke dans `SuffixPolicyV1`. |
| `SuffixSelector` | Forme canonique en chaine du suffixe (ASCII, lower-case). | Exemple: `.sora` -> `sora`. |
| `NameSelectorV1` | Selecteur binaire pour le label enregistre. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Le label est NFC + lower-case selon Norm v1. |
| `NameHash` (`[u8;32]`) | Cle primaire de recherche utilisee par contrats, evenements et caches. | `blake3(NameSelectorV1_bytes)`. |

Exigences de determinisme:

- Les labels sont normalises via Norm v1 (UTS-46 strict, STD3 ASCII, NFC). Les chaines utilisateur DOIVENT etre normalisees avant le hash.
- Les labels reserves (de `SuffixPolicyV1.reserved_labels`) n'entrent jamais dans le registre; les overrides uniquement gouvernance emettent des evenements `ReservedNameAssigned`.

## 2. Structures Norito

### 2.1 NameRecordV1

| Champ | Type | Notes |
|-------|------|-------|
| `suffix_id` | `u16` | Reference `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Octets du selecteur brut pour audit/debug. |
| `name_hash` | `[u8; 32]` | Cle pour maps/evenements. |
| `normalized_label` | `AsciiString` | Label lisible pour l'humain (post Norm v1). |
| `display_label` | `AsciiString` | Casing fourni par le steward; cosmetique optionnelle. |
| `owner` | `AccountId` | Controle les renouvellements/transferts. |
| `controllers` | `Vec<NameControllerV1>` | References vers des adresses de compte cibles, resolvers ou metadata d'application. |
| `status` | `NameStatus` | Indicateur de cycle de vie (voir Section 4). |
| `pricing_class` | `u8` | Index dans les tiers de prix du suffixe (standard, premium, reserved). |
| `registered_at` | `Timestamp` | Timestamp de bloc de l'activation initiale. |
| `expires_at` | `Timestamp` | Fin du terme paye. |
| `grace_expires_at` | `Timestamp` | Fin de grace d'auto-renouvellement (default +30 jours). |
| `redemption_expires_at` | `Timestamp` | Fin de la fenetre de redemption (default +60 jours). |
| `auction` | `Option<NameAuctionStateV1>` | Present quand des Dutch reopen ou encheres premium sont actives. |
| `last_tx_hash` | `Hash` | Pointeur deterministe vers la transaction qui a produit cette version. |
| `metadata` | `Metadata` | Metadata arbitraire du registrar (text records, proofs). |

Structs de support:

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

| Champ | Type | Notes |
|-------|------|-------|
| `suffix_id` | `u16` | Cle primaire; stable entre versions de politique. |
| `suffix` | `AsciiString` | par exemple, `sora`. |
| `steward` | `AccountId` | Steward defini dans le charter de gouvernance. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Identifiant d'actif de settlement par defaut (par exemple `xor#sora`). |
| `pricing` | `Vec<PriceTierV1>` | Coefficients de prix par tiers et regles de duree. |
| `min_term_years` | `u8` | Plancher pour le terme achete quel que soit l'override de tier. |
| `grace_period_days` | `u16` | Default 30. |
| `redemption_period_days` | `u16` | Default 60. |
| `max_term_years` | `u8` | Maximum de renouvellement anticipe. |
| `referral_cap_bps` | `u16` | <=1000 (10%) selon le charter. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Liste fournie par la gouvernance avec instructions d'affectation. |
| `fee_split` | `SuffixFeeSplitV1` | Parts tresorerie / steward / referral (basis points). |
| `fund_splitter_account` | `AccountId` | Compte qui detient l'escrow + distribue les fonds. |
| `policy_version` | `u16` | Incremente a chaque changement. |
| `metadata` | `Metadata` | Notes etendues (KPI covenant, hashes de docs de compliance). |

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

### 2.3 Enregistrements de revenus et de settlement

| Struct | Champs | But |
|--------|--------|-----|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Enregistrement deterministe des paiements routes par epoque de settlement (hebdomadaire). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Emis a chaque paiement poste (enregistrement, renouvellement, enchere). |

Tous les champs `TokenValue` utilisent l'encodage fixe canonique de Norito avec le code devise declare dans le `SuffixPolicyV1` associe.

### 2.4 Evenements du registre

Les evenements canoniques fournissent un log de replay pour l'automatisation DNS/gateway et l'analytique.

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

Les evenements doivent etre ajoutes a un log rejouable (par exemple, le domaine `RegistryEvents`) et refletes vers les feeds gateway pour que les caches DNS invalident dans les SLA.

## 3. Layout de stockage et indexes

| Cle | Description |
|-----|-------------|
| `Names::<name_hash>` | Map primaire de `name_hash` vers `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Index secondaire pour UI wallet (pagination friendly). |
| `NamesByLabel::<suffix_id, normalized_label>` | Detecte les conflits, alimente la recherche deterministe. |
| `SuffixPolicies::<suffix_id>` | Dernier `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | Historique `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Log append-only cle par sequence monotone. |

Toutes les cles sont serialisees via des tuples Norito pour garder un hashing deterministe entre hotes. Les mises a jour d'index se font atomiquement avec l'enregistrement principal.

## 4. Machine d'etats du cycle de vie

| Etat | Conditions d'entree | Transitions permises | Notes |
|-------|--------------------|---------------------|-------|
| Available | Derive quand `NameRecord` est absent. | `PendingAuction` (premium), `Active` (enregistrement standard). | La recherche de disponibilite lit seulement les indexes. |
| PendingAuction | Cree quand `PriceTierV1.auction_kind` != none. | `Active` (enchere reglee), `Tombstoned` (aucune enchere). | Les encheres emettent `AuctionOpened` et `AuctionSettled`. |
| Active | Enregistrement ou renouvellement reussi. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` pilote la transition. |
| GracePeriod | Automatique quand `now > expires_at`. | `Active` (renouvellement a temps), `Redemption`, `Tombstoned`. | Default +30 jours; resolu mais marque. |
| Redemption | `now > grace_expires_at` mais `< redemption_expires_at`. | `Active` (renouvellement tardif), `Tombstoned`. | Les commandes exigent des frais de penalite. |
| Frozen | Freeze de gouvernance ou guardian. | `Active` (apres remediation), `Tombstoned`. | Ne peut pas transferer ni mettre a jour les controllers. |
| Tombstoned | Abandon volontaire, resultat de litige permanent, ou redemption expiree. | `PendingAuction` (Dutch reopen) ou reste tombstoned. | L'evenement `NameTombstoned` doit inclure une raison. |

Les transitions d'etat DOIVENT emettre le `RegistryEventKind` correspondant pour que les caches downstream restent coherentes. Les noms tombstoned entrant en encheres Dutch reopen attachent un payload `AuctionKind::DutchReopen`.

## 5. Evenements canoniques et sync gateway

Les gateways s'abonnent a `RegistryEventV1` et synchronisent DNS/SoraFS via:

1. Recuperer le dernier `NameRecordV1` reference par la sequence d'evenements.
2. Regenerer les templates de resolver (adresses IH58 preferees + compressed (`sora`) en second choix, text records).
3. Pinner les donnees de zone mises a jour via le workflow SoraDNS decrit dans [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Garanties de livraison d'evenements:

- Chaque transaction qui affecte un `NameRecordV1` *doit* ajouter exactement un evenement avec `version` strictement croissante.
- Les evenements `RevenueSharePosted` referencent les settlements emis par `RevenueShareRecordV1`.
- Les evenements freeze/unfreeze/tombstone incluent les hashes d'artefacts de gouvernance dans `metadata` pour le replay d'audit.

## 6. Exemples de payloads Norito

### 6.1 Exemple NameRecord

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

### 6.2 Exemple SuffixPolicy

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

- **SN-2b (Registrar API & governance hooks):** exposer ces structs via Torii (bindings Norito et JSON) et connecter les checks d'admission aux artefacts de gouvernance.
- **SN-3 (Auction & registration engine):** reutiliser `NameAuctionStateV1` pour implementer la logique commit/reveal et Dutch reopen.
- **SN-5 (Payment & settlement):** exploiter `RevenueShareRecordV1` pour la reconciliation financiere et l'automatisation des rapports.

Les questions ou demandes de changement doivent etre deposees avec les mises a jour du roadmap SNS dans `roadmap.md` et refletees dans `status.md` lors de la fusion.
