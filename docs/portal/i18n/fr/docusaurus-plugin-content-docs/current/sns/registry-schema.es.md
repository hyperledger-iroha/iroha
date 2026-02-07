---
lang: fr
direction: ltr
source: docs/portal/docs/sns/registry-schema.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Fuente canonica
Cette page reflète `docs/source/sns/registry_schema.md` et maintenant, vous êtes comme la copie canonique du portail. Le fichier est conservé pour l'actualisation de la traduction.
:::

# Esquema del registro del Sora Name Service (SN-2a)

**État :** Réduit 2026-03-24 -- envoyé une révision du programme SNS  
**Enlace del roadmap :** SN-2a "Schéma de registre et disposition du stockage"  
**Alcance :** Définissez les structures canoniques Norito, les états de cycle de vie et les événements émis pour le Sora Name Service (SNS) de manière à ce que les implémentations d'enregistrement et d'enregistrement soient déterminées dans les contrats, les SDK et les passerelles.

Ce document complet contient le schéma pour SN-2a en précisant :

1. Identifiants et règles de hachage (`SuffixId`, `NameHash`, dérivation des sélecteurs).
2. Structs/enums Norito pour les registres de nombres, les politiques de soufijos, les niveaux de prix, les répartitions d'entrées et les événements du registre.
3. Disposition de l'almacenamiento et des préfixes d'index pour replay determinista.
4. Une machine d'état qui cubre registro, renovacion, gracia/redencion, freezes and tombstones.
5. Événements canoniques consommés par l'automatisation DNS/passerelle.

## 1. Identifiants et hachage| Identifiant | Description | Dérivation |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Identifiant du registre des soufijos de niveau supérieur (`.sora`, `.nexus`, `.dao`). Aligneado avec le catalogue de soufijos en [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Asignado por voto de gobernanza; placé sur `SuffixPolicyV1`. |
| `SuffixSelector` | Forme canonique en chaîne du sufijo (ASCII, minuscule). | Exemple : `.sora` -> `sora`. |
| `NameSelectorV1` | Sélecteur binaire pour l'étiquette enregistrée. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. L'étiquette est NFC + minuscule selon Norm v1. |
| `NameHash` (`[u8;32]`) | Clé principale de travail utilisée pour les contrats, événements et caches. | `blake3(NameSelectorV1_bytes)`. |

Conditions requises pour le déterminisme :

- Les étiquettes sont normalisées via Norm v1 (UTS-46 strict, STD3 ASCII, NFC). Les chaînes de l'utilisateur DEBEN normalisent avant le hachage.
- Les étiquettes réservées (de `SuffixPolicyV1.reserved_labels`) n'entrent pas dans le registre ; les remplacements en solo de gouvernement émettent des événements `ReservedNameAssigned`.

## 2. Structures Norito

### 2.1 NomEnregistrementV1| Champ | Type | Notes |
|-------|------|-------|
| `suffix_id` | `u16` | Référence `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Les octets du sélecteur ne sont pas traités pour l'auditoire/le débogage. |
| `name_hash` | `[u8; 32]` | Clé pour cartes/événements. |
| `normalized_label` | `AsciiString` | Etiqueta lisible por humanos (post Norm v1). |
| `display_label` | `AsciiString` | Mise à disposition du boîtier pour l'intendant ; cosmétique en option. |
| `owner` | `AccountId` | Contrôle des rénovations/transferts. |
| `controllers` | `Vec<NameControllerV1>` | Références aux directions du compte objet, aux résolveurs ou aux métadonnées d'application. |
| `status` | `NameStatus` | Bandera de cycle de vida (voir Section 4). |
| `pricing_class` | `u8` | Indice en niveaux de precios del sufijo (standard, premium, réservé). |
| `registered_at` | `Timestamp` | Timestamp de blocage de l’activation initiale. |
| `expires_at` | `Timestamp` | Fin du termino payé. |
| `grace_expires_at` | `Timestamp` | Fin de gracia de auto-renovacion (par défaut +30 jours). |
| `redemption_expires_at` | `Timestamp` | Fin de ventana de redencion (par défaut +60 jours). |
| `auction` | `Option<NameAuctionStateV1>` | Présentez-vous lorsque vous réapprenez le néerlandais ou que les subastas premium sont activés. |
| `last_tx_hash` | `Hash` | Un pointeur déterminant pour la transaction qui produit cette version. || `metadata` | `Metadata` | Métadonnées arbitraires du registraire (enregistrements de texte, preuves). |

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

### 2.2 SuffixPolicyV1| Champ | Type | Notes |
|-------|------|-------|
| `suffix_id` | `u16` | Clave primaire ; estable entre les versions de politique. |
| `suffix` | `AsciiString` | par exemple, `sora`. |
| `steward` | `AccountId` | Steward défini dans la charte de gobernanza. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Identificateur d'actif de règlement par défaut (par exemple `xor#sora`). |
| `pricing` | `Vec<PriceTierV1>` | Coeficientes de precios por tiers y reglas de duracion. |
| `min_term_years` | `u8` | Piso para el termino comprado sin importar overrides de tier. |
| `grace_period_days` | `u16` | Par défaut 30. |
| `redemption_period_days` | `u16` | Par défaut 60. |
| `max_term_years` | `u8` | Maximo de rénovation par adelantado. |
| `referral_cap_bps` | `u16` | <=1000 (10%) selon la charte. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Liste suministrada por gobernanza con instrucciones de asignación. |
| `fee_split` | `SuffixFeeSplitV1` | Porciones de tesoreria / steward / référence (points de base). |
| `fund_splitter_account` | `AccountId` | Compte tenu du maintien du séquestre + de la distribution des fonds. |
| `policy_version` | `u16` | Augmentez à chaque changement. |
| `metadata` | `Metadata` | Notas extendidas (accord KPI, hachages de documents de cumplimiento). |```text
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

### 2.3 Registres des revenus et règlements

| Structure | Campos | Proposé |
|--------|--------|---------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Registro determinista de pagos enroutados por epoca de colonisation (semanal). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Émitido cada vez qu'un pago se registra (registro, renovacion, subasta). |

Tous les champs `TokenValue` utilisent la codification fidèle au canon Norito avec le code de monnaie déclaré dans le `SuffixPolicyV1` associé.

### 2.4 Événements du registre

Les événements canoniques ont prouvé un journal de relecture pour l'automatisation DNS/passerelle et les analyses.

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

Les événements doivent regrouper un journal reproductible (par exemple, le domaine `RegistryEvents`) et réfléchir aux flux de la passerelle pour que les caches DNS invalident dans le SLA.

## 3. Disposition du stockage et des indices| Clave | Description |
|-----|-------------|
| `Names::<name_hash>` | Carte principale de `name_hash` à `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Indice secondaire pour l'interface utilisateur du portefeuille (pagination amiable). |
| `NamesByLabel::<suffix_id, normalized_label>` | Détecta les conflits, habilita busqueda determinista. |
| `SuffixPolicies::<suffix_id>` | Ultimo `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | Historial de `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Journal en annexe uniquement avec la clé de sécurité monotonique. |

Toutes les clés sont sérialisées en utilisant les tuplas Norito pour maintenir le hachage déterministe entre les hôtes. Les actualisations des indices se produisent sous forme atomique conjointement avec le registre primaire.

## 4. Machine d'état pour le cycle de vie| État | Conditions d'entrée | Transitions autorisées | Notes |
|-------|--------------|---------------|-------|
| Disponible | Dérivé lorsque `NameRecord` est ausente. | `PendingAuction` (premium), `Active` (standard d'enregistrement). | La recherche de disponibilité ne contient que des indices. |
| En attente d'enchères | Créé quand `PriceTierV1.auction_kind` != none. | `Active` (la subasta se liquida), `Tombstoned` (sin pujas). | Les subastas émettent `AuctionOpened` et `AuctionSettled`. |
| Actif | Registro o renovacion exitosa. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` impulse la transition. |
| Période de grâce | Automatique lorsque `now > expires_at`. | `Active` (rénovation à temps), `Redemption`, `Tombstoned`. | Par défaut +30 jours ; aun resuelve pero marcado. |
| Rédemption | `now > grace_expires_at` par rapport à `< redemption_expires_at`. | `Active` (rénovation tardive), `Tombstoned`. | Les commandants exigent des frais de pénalité. |
| Congelé | Freeze de gobernanza o tuteur. | `Active` (tras remédiation), `Tombstoned`. | Vous ne pouvez pas transférer ni actualiser les contrôleurs. |
| Tombé | Rendicion volontaire, résultat d'un litige permanent, ou redencion expirada. | `PendingAuction` (réouverture aux Pays-Bas) ou pierre tombale permanente. | L'événement `NameTombstoned` doit inclure la raison. |Les transitions de l'état DEBEN émettent le correspondant `RegistryEventKind` pour que les caches en aval soient cohérents. Les nombres tombstoned qui entrent dans les subastas Dutch rouvrent avec une charge utile `AuctionKind::DutchReopen`.

## 5. Événements canoniques et synchronisation des passerelles

Les passerelles sont inscrites à `RegistryEventV1` et synchronisées avec DNS/SoraFS intermédiaire :

1. Obtenez la dernière référence `NameRecordV1` pour la sécurité des événements.
2. Régénérer les modèles de résolveur (directions IH58 préférées + compressé (`sora`) comme deuxième option, enregistrements de texte).
3. Pinnear données de zone actualisées via le flux SoraDNS décrit en [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Garanties de participation aux événements :

- Chaque transaction qui affecte un `NameRecordV1` *doit* agréger exactement un événement avec un `version` strictement créatif.
- Les événements `RevenueSharePosted` référents liquidaciones émis par `RevenueShareRecordV1`.
- Les événements de gel/dégel/tombstone incluent les hachages des artefacts de gestion à l'intérieur de `metadata` pour la relecture de l'auditoire.

## 6. Exemples de charges utiles Norito

### 6.1 Exemple de NameRecord

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

### 6.2 Exemple de SuffixPolicy

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

## 7. Proximos pasos- **SN-2b (API du registraire et hooks de gouvernance) :** exponer ces structures via Torii (liaisons Norito et JSON) et connecter les vérifications d'admission aux artefacts de gouvernement.
- **SN-3 (moteur d'enchères et d'enregistrement) :** réutiliser `NameAuctionStateV1` pour implémenter la logique de validation/révélation et la réouverture néerlandaise.
- **SN-5 (Paiement et règlement) :** aprovechar `RevenueShareRecordV1` pour la réconciliation financière et l'automatisation des rapports.

Les questions ou demandes de changement doivent être enregistrées conjointement avec les mises à jour de la feuille de route de SNS en `roadmap.md` et réfléchies en `status.md` lorsqu'elles sont intégrées.