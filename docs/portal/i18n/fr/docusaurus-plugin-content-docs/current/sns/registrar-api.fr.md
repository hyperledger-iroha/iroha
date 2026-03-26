---
lang: fr
direction: ltr
source: docs/portal/docs/sns/registrar-api.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Source canonique
Cette page reflète `docs/source/sns/registrar_api.md` et sert désormais de
copie canonique du portail. Le fichier source est conservé pour les flux de
traduction.
:::

# API du registraire SNS et hooks de gouvernance (SN-2b)

**Statut :** Redige 2026-03-24 -- en revue par Nexus Core  
**Feuille de route des liens :** SN-2b "API du registraire et hooks de gouvernance"  
**Prérequis :** Définitions de schéma dans [`registry-schema.md`](./registry-schema.md)

Cette note spécifie les points de terminaison Torii, services gRPC, DTO de requête/réponse
et artefacts de gouvernance nécessaires pour operer le registrar du Sora Name
Service (SNS). C'est le contrat de référence pour les SDK, wallets et
automatisation qui doit enregistrer, renouveler ou gerer des noms SNS.

## 1. Transports et authentification| Exigence | Détail |
|--------------|--------|
| Protocoles | REST sous `/v1/sns/*` et service gRPC `sns.v1.Registrar`. Les deux acceptent Norito-JSON (`application/json`) et Norito-RPC binaire (`application/x-norito`). |
| Authentification | Jetons `Authorization: Bearer` ou certificats mTLS emis par suffixe steward. Les points de terminaison sensibles à la gouvernance (gel/dégel, affectations réservées) exigent `scope=sns.admin`. |
| Limites de débit | Les registrars partagent les buckets `torii.preauth_scheme_limits` avec les appelants JSON plus des limites de rafale par suffixe : `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Télémétrie | Torii expose `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` pour les gestionnaires du registraire (filtre `scheme="norito_rpc"`) ; l'API incrémente également `sns_registrar_status_total{result, suffix_id}`. |

## 2. Ouverture du DTO

Les champs référencent les structs canoniques définis dans [`registry-schema.md`](./registry-schema.md). Tous les frais utiles intègrent `NameSelectorV1` + `SuffixId` pour éviter un routage ambigu.

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

## 3. Points de terminaison REST| Point de terminaison | Méthode | Charge utile | Descriptif |
|--------------|---------|---------|-------------|
| `/v1/sns/names` | POSTER | `RegisterNameRequestV1` | Enregistrer ou rouvrir un nom. Resout le niveau de prix, valide les preuves de paiement/gouvernance, et des événements de registre. |
| `/v1/sns/names/{namespace}/{literal}/renew` | POSTER | `RenewNameRequestV1` | Prolongez le terme. Appliquer les fenêtres de grâce/rédemption depuis la politique. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POSTER | `TransferNameRequestV1` | Transférer la propriété une fois les approbations de gouvernance conjointe. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | METTRE | `UpdateControllersRequestV1` | Remplacer l'ensemble des contrôleurs; valider les adresses de compte signées. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POSTER | `FreezeNameRequestV1` | Geler le gardien/conseil. Exige un ticket gardien et une référence au dossier de gouvernance. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | SUPPRIMER | `GovernanceHookV1` | Dégeler après la remédiation ; s'assure que l'override du conseil est enregistré. |
| `/v1/sns/reserved/{selector}` | POSTER | `ReservedAssignmentRequestV1` | Affectation de noms réserves par steward/council. |
| `/v1/sns/policies/{suffix_id}` | OBTENIR | -- | Récupérez le courant `SuffixPolicyV1` (mis en cache). |
| `/v1/sns/names/{namespace}/{literal}` | OBTENIR | -- | Retourne le `NameRecordV1` courant + état effectif (Active, Grace, etc.). |

**Encodage du sélecteur :** le segment `{selector}` accepte i105, compressé, ou hex canonique selon ADDR-5; Torii le normalise via `NameSelectorV1`.**Modèle d'erreurs :** tous les points de terminaison retournent Norito JSON avec `code`, `message`, `details`. Les codes incluent `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Aides CLI (exigence du registraire manuel N0)

Les stewards de beta fermee peuvent désormais utiliser le registrar via la CLI sans fabriquer du JSON à la main :

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

- `--owner` prend par défaut le compte de configuration de la CLI ; répétez `--controller` pour ajouter des comptes contrôleur supplémentaires (par défaut `[owner]`).
- Les flags inline de paiement mappent directement vers `PaymentProofV1`; passez `--payment-json PATH` lorsque vous avez déjà une structure de récupération. Les métadonnées (`--metadata-json`) et les hooks de gouvernance (`--governance-json`) suivent le schéma meme.

Les aides en lecture complètent seules les répétitions :

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Voir `crates/iroha_cli/src/commands/sns.rs` pour l'implémentation ; les commandes réutilisent les DTOs Norito décrites dans ce document afin que la sortie CLI corresponde octet pour octet aux réponses Torii.

Des aides supplémentaires couvrent les renouvellements, les transferts et les actions tutélaires :

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
  --new-owner <katakana-i105-account-id> \
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
````--governance-json` doit contenir un enregistrement `GovernanceHookV1` valide (identifiant de proposition, hachages de vote, commissaire/tuteur des signatures). Chaque commande reflète simplement l'endpoint `/v1/sns/names/{namespace}/{literal}/...` correspondant pour que les opérateurs de bêta puissent répéter exactement les surfaces Torii que les SDK appelleront.

## 4. Service gRPC

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

Wire-format: hash du schéma Norito en temps de compilation enregistré dans
`fixtures/norito_rpc/schema_hashes.json` (lignes `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, etc.).

## 5. Hooks de gouvernance et preuves

Chaque appel qui modifie l'état doit joindre des preuves réutilisables pour la relecture :

| Actions | Données de gouvernance requises |
|--------|-------------------------------|
| Norme d'enregistrement/renouvellement | Preuve de paiement référençant une instruction de règlement ; aucun vote du conseil requis sauf si le tier exige l'approbation steward. |
| Enregistrement de niveau premium / affectation réservée | `GovernanceHookV1` ID de proposition de référent + accusé de réception du steward. |
| Transfert | Hash du vote conseil + hash du signal DAO ; clearing tuteur quand le transfert est déclenché par résolution de litige. |
| Geler/Dégeler | Signature du gardien du ticket plus dérogation du conseil (dégel). |

Torii vérifier les preuves en vérifiant :1. L'identifiant de proposition existe dans le grand livre de gouvernance (`/v1/governance/proposals/{id}`) et le statut est `Approved`.
2. Les hachages correspondant aux artefacts de vote enregistrés.
3. Les signatures steward/guardian référencent les clés publiques attendues de `SuffixPolicyV1`.

Les contrôles en echec renvoient `sns_err_governance_missing`.

## 6. Exemples de workflow

### 6.1 Norme d'enregistrement

1. Le client interroge `/v1/sns/policies/{suffix_id}` pour récupérer les prix, la grâce et les niveaux disponibles.
2. Le client a construit `RegisterNameRequestV1` :
   - `selector` dérive du label i105 (préférer) ou compresse (deuxième choix).
   - `term_years` dans les limites de la politique.
   - `payment` référencant le transfert du splitter trésorerie/steward.
3. Torii valide :
   - Normalisation du label + liste réservée.
   - Durée/prix brut vs `PriceTierV1`.
   - Montant de preuve de paiement >= prix calculé + frais.
4. En succès Torii :
   - Persister `NameRecordV1`.
   -Emet `RegistryEventV1::NameRegistered`.
   -Emet `RevenueAccrualEventV1`.
   - Retourne le nouveau disque + soirées.

### 6.2 Renouvellement pendant la grâce

Les renouvellements pendant la grâce incluent la demande standard plus la détection de pénalité :- Torii compare `now` vs `grace_expires_at` et ajoute les tables de supplément de `SuffixPolicyV1`.
- La preuve de paiement doit couvrir le supplément. Échec => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` enregistre le nouveau `expires_at`.

### 6.3 Freeze Guardian et override du conseil

1. Guardian soumet `FreezeNameRequestV1` avec un ticket référençant l'id d'incident.
2. Torii remplace l'enregistrement en `NameStatus::Frozen`, emet `NameFrozen`.
3. Après la remédiation, le conseil emet un override ; l'opérateur envoie DELETE `/v1/sns/names/{namespace}/{literal}/freeze` avec `GovernanceHookV1`.
4. Torii valide l'override, emet `NameUnfrozen`.

## 7. Validation et codes d'erreur

| Codes | Descriptif | HTTP |
|------|-------------|------|
| `sns_err_reserved` | Étiquette réserver ou bloquer. | 409 |
| `sns_err_policy_violation` | Terme, niveau ou ensemble de contrôleurs violent la politique. | 422 |
| `sns_err_payment_mismatch` | Mismatch de valeur ou d'actif dans la preuve de paiement. | 402 |
| `sns_err_governance_missing` | Artefacts de gouvernance requis absents/invalides. | 403 |
| `sns_err_state_conflict` | Opération non permise dans l'état de cycle de vie actuel. | 409 |

Tous les codes apparaissent via `X-Iroha-Error-Code` et des enveloppes Norito JSON/NRPC structurées.

## 8. Notes d'implémentation- Torii stocke les enchères en attente sous `NameRecordV1.auction` et rejette les tentatives d'enregistrement direct tant que `PendingAuction`.
- Les preuves de paiement réutilisant les recus du grand livre Norito ; les services de tresorerie fournis des APIs helper (`/v1/finance/sns/payments`).
- Les SDK devraient envelopper ces endpoints avec des helpers fortement types pour que les wallets puissent présenter des raisons d'erreur claires (`ERR_SNS_RESERVED`, etc.).

## 9. Prochaines étapes

- Relier les handlers Torii au contrat de registre réel une fois les enchères SN-3 disponibles.
- Publier des guides SDK spécifiques (Rust/JS/Swift) référençant cette API.
- Etendre [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) avec des liens croisés vers les champs de preuve des hooks de gouvernance.