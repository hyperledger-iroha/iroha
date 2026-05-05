---
lang: fr
direction: ltr
source: docs/portal/docs/sns/registry-schema.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Fonte canonica
Cette page espelha `docs/source/sns/registry_schema.md` et sert maintenant de copie canonique du portail. L'archive est permanente pour l'actualisation des traductions.
:::

# Esquema do registro do Sora Name Service (SN-2a)

**Statut :** Redigido 2026-03-24 -- soumis pour la révision du programme SNS  
**Lien vers la feuille de route :** SN-2a "Schéma de registre et disposition du stockage"  
**Escopo :** Définissez les structures Norito canoniques, les états de cycle de vie et les événements émis pour le Sora Name Service (SNS) pour que les implémentations de registre et de registre déterminent les contrats, les SDK et les passerelles.

Ce document complet ou entregavel de esquema pour SN-2a précise :

1. Identifiants et paramètres de hachage (`SuffixId`, `NameHash`, dérivé des sélections).
2. Structs/enums Norito pour les registres de noms, les suffixes politiques, les niveaux de paiement, les répartitions de réception et les événements du registre.
3. Disposition de l'armement et des préfixes d'index pour une relecture déterministe.
4. Une machine d'état cobrindo registro, rénovation, grâce/rédemption, gele et pierres tombales.
5. Événements canoniques consommés par la passerelle DNS/passerelle automatique.

## 1. Identifiants et hachage| Identifiant | Description | Dérivacao |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Identifiant du registre des suffixes de niveau supérieur (`.sora`, `.nexus`, `.dao`). Ajouté au catalogue de suffixes dans [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Atribuido por voto de gouvernance; armazenado em `SuffixPolicyV1`. |
| `SuffixSelector` | Forme canonique dans une chaîne avec suffixe (ASCII, minuscule). | Exemple : `.sora` -> `sora`. |
| `NameSelectorV1` | Sélectionnez le binaire pour le disque enregistré. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Le rotule et le NFC + la seconde minuscule Norm v1. |
| `NameHash` (`[u8;32]`) | C'est la première chose à faire pour les contrats, les événements et les caches. | `blake3(NameSelectorV1_bytes)`. |

Conditions requises pour le déterminisme :

- Les rotulos sont normalisés via Norm v1 (UTS-46 strict, STD3 ASCII, NFC). Comme les chaînes de l'utilisateur DEVEM sont normalisées avant le hachage.
- Rotulos reservados (de `SuffixPolicyV1.reserved_labels`) nunca entram no registro ; remplace les fonctions de gouvernance émettant des événements `ReservedNameAssigned`.

## 2. Structures Norito

### 2.1 NomEnregistrementV1| Champ | Type | Notes |
|-------|------|-------|
| `suffix_id` | `u16` | Référence `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Octets du sélecteur brut pour l'auditoire/le débogage. |
| `name_hash` | `[u8; 32]` | Chave para mapas/eventos. |
| `normalized_label` | `AsciiString` | Rotulo legivel por humanos (post Norm v1). |
| `display_label` | `AsciiString` | Boîtier fornecido pelo steward; cosmétique en option. |
| `owner` | `AccountId` | Controla renovacoes/transferencias. |
| `controllers` | `Vec<NameControllerV1>` | Références aux fichiers de contenu d'alvo, résolveurs ou métadonnées d'application. |
| `status` | `NameStatus` | Indicateur de cycle de vie (voir Secao 4). |
| `pricing_class` | `u8` | Indice nos niveaux de preco do sufixo (standard, premium, réservé). |
| `registered_at` | `Timestamp` | Horodatage du bloc d'activation initial. |
| `expires_at` | `Timestamp` | Fim do termo pago. |
| `grace_expires_at` | `Timestamp` | Fim da grace de auto-renovação (par défaut +30 jours). |
| `redemption_expires_at` | `Timestamp` | Fim da janela de redemption (par défaut +60 jours). |
| `auction` | `Option<NameAuctionStateV1>` | Présente quand les Pays-Bas rouvrent ou leiloes premium ativos. |
| `last_tx_hash` | `Hash` | Ponteiro determinista para a transacao que segerou esta versao. || `metadata` | `Metadata` | Arbitraire des métadonnées du registraire (enregistrements de texte, preuves). |

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
| `suffix_id` | `u16` | Chave primaria; estavel entre versos de politica. |
| `suffix` | `AsciiString` | par exemple, `sora`. |
| `steward` | `AccountId` | L'intendant n'a pas défini de charte de gouvernance. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Identifiant de l'activité de règlement par le responsable (par exemple `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | Coeficientes de preco por tiers et regras de duracao. |
| `min_term_years` | `u8` | Piso para o termo comprado indépendamment des remplacements de niveau. |
| `grace_period_days` | `u16` | Par défaut 30. |
| `redemption_period_days` | `u16` | Par défaut 60. |
| `max_term_years` | `u8` | Maximo de renovação anticipada. |
| `referral_cap_bps` | `u16` | <=1000 (10%) seconde ou charter. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Liste fournie par la gouvernance avec les instructions d'attribution. |
| `fee_split` | `SuffixFeeSplitV1` | Partes tesouraria / steward / référence (points de base). |
| `fund_splitter_account` | `AccountId` | Conta que mantem escrow + distribui fundos. |
| `policy_version` | `u16` | Incrémentez-le chaque fois. |
| `metadata` | `Metadata` | Notas estendidas (convention KPI, hashs de documents de conformité). |

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
```### 2.3 Registres de réception et de règlement

| Structure | Campos | Proposé |
|--------|--------|---------------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Registro deterministico de pagamentos roteados por epoca de règlement (semanal). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Émis cada vez qu'un pagamento e postado (registro, renovação, leilao). |

Tous les champs `TokenValue` utilisent le code canonique Norito avec le code de la personne déclarée non associée `SuffixPolicyV1`.

### 2.4 Événements à enregistrer

Les événements canoniques forment un journal de relecture pour l'automatisation DNS/passerelle et analyses.

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

Les événements doivent être examinés dans une reproduction de journal (par exemple, le domaine `RegistryEvents`) et reflètent nos flux de passerelle pour que les caches DNS soient invalides dans le SLA.

## 3. Disposition de l'armement et des indices| Chave | Description |
|-----|-------------|
| `Names::<name_hash>` | Carte principale de `name_hash` pour `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Indice secondaire pour l'interface utilisateur du portefeuille (page amigavel). |
| `NamesByLabel::<suffix_id, normalized_label>` | Détecter les conflits, habilita busca deterministica. |
| `SuffixPolicies::<suffix_id>` | Ultimo `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | Historique de `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Journal en annexe uniquement avec une séquence monotone. |

Tous ces éléments sont sérialisés en utilisant les tuplas Norito pour gérer le hachage déterministe entre les hôtes. L'actualisation de l'indice correspond à la forme atomique avec le registre principal.

## 4. Machine d'état pour le cycle de vie| État | Conditions d'entrée | Transicoes permisidas | Notes |
|-------|------------|----------------------|-------|
| Disponible | Dérivé lorsque `NameRecord` est ausente. | `PendingAuction` (premium), `Active` (norme d'enregistrement). | A busca de disponibilidade le apenas indices. |
| En attente d'enchères | Criado quando `PriceTierV1.auction_kind` != none. | `Active` (leilao liquida), `Tombstoned` (sem lances). | Les lignes émettent les éléments `AuctionOpened` et `AuctionSettled`. |
| Actif | Registro ou renovação bem-sucedida. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` guide la transition. |
| Période de grâce | Automatique lorsque `now > expires_at`. | `Active` (rénovation en dia), `Redemption`, `Tombstoned`. | Par défaut +30 jours ; ainda résout mas sinalizado. |
| Rédemption | `now > grace_expires_at` et `< redemption_expires_at`. | `Active` (rénovation tardive), `Tombstoned`. | Les commandants exigent des taxes de pénalité. |
| Congelé | Freeze de gouvernance ou tuteur. | `Active` (possibilité de correction), `Tombstoned`. | Nao peut transférer nem actualiser les contrôleurs. |
| Tombé | Renonce volontaire, résultant d'un litige permanent ou d'un rachat expiré. | `PendingAuction` (réouverture aux Pays-Bas) ou tombstoned permanent. | L'événement `NameTombstoned` doit inclure un motif. |En tant que transicos de l'état DEVEM émettent le correspondant `RegistryEventKind` pour gérer les caches en aval des coordonnées. Nomes tombstoned que entram em leiloes Dutch rouvre l'examen de la charge utile `AuctionKind::DutchReopen`.

## 5. Événements canoniques et synchronisation des passerelles

Les passerelles associent `RegistryEventV1` et synchronisent DNS/SoraFS entre autres :

1. Recherchez le dernier `NameRecordV1` référencé par la séquence d'événements.
2. Régénérer les modèles de résolveur (envoyer les préférences i105 + compressé (`sora`) comme deuxième option, enregistrements de texte).
3. Pinnear données de zone actualisées via le flux SoraDNS décrit dans [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Garanties de participation aux événements :

- Cada transacao que afeta um `NameRecordV1` *deve* anexar exatamente um evento com `version` estritamente croissante.
- Les événements `RevenueSharePosted` font référence aux liquidités émises par `RevenueShareRecordV1`.
- Les événements de gel/dégel/tombstone incluent les hachages des artefatos de gouvernance à l'intérieur de `metadata` pour la relecture de l'auditoire.

## 6. Exemples de charges utiles Norito

### 6.1 Exemple de NameRecord

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

### 6.2 Exemple de SuffixPolicy

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

## 7. Proximos passos- **SN-2b (API du registraire et hooks de gouvernance) :** exportez ces structures via Torii (liaisons Norito et JSON) et vérifiez l'admission aux articles de gouvernance.
- **SN-3 (moteur d'enchères et d'enregistrement) :** réutiliser `NameAuctionStateV1` pour implémenter la logique de validation/révélation et la réouverture néerlandaise.
- **SN-5 (Paiement et règlement) :** approuver `RevenueShareRecordV1` pour concilier les finances et l'automatisation des relations.

Les demandes ou sollicitations de changement doivent être enregistrées conjointement avec les mises à jour de la feuille de route SNS sur `roadmap.md` et reflétées sur `status.md` lorsqu'elles sont intégrées.