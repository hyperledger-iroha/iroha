---
lang: fr
direction: ltr
source: docs/portal/docs/sns/registrar-api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 14373f09e9a691a910fbde08548c5fdfe03581049c85af425c27c94fc4fcafd8
source_last_modified: "2025-11-10T20:06:34.010395+00:00"
translation_last_reviewed: 2026-01-01
---

:::note Source canonique
Cette page reflete `docs/source/sns/registrar_api.md` et sert desormais de
copie canonique du portail. Le fichier source est conserve pour les flux de
traduction.
:::

# API du registrar SNS et hooks de gouvernance (SN-2b)

**Statut:** Redige 2026-03-24 -- en revue par Nexus Core  
**Lien roadmap:** SN-2b "Registrar API & governance hooks"  
**Prerequis:** Definitions de schema dans [`registry-schema.md`](./registry-schema.md)

Cette note specifie les endpoints Torii, services gRPC, DTOs de requete/reponse
et artefacts de gouvernance necessaires pour operer le registrar du Sora Name
Service (SNS). C'est le contrat de reference pour les SDKs, wallets et
automatisation qui doivent enregistrer, renouveler ou gerer des noms SNS.

## 1. Transport et authentification

| Exigence | Detail |
|----------|--------|
| Protocoles | REST sous `/v1/sns/*` et service gRPC `sns.v1.Registrar`. Les deux acceptent Norito-JSON (`application/json`) et Norito-RPC binaire (`application/x-norito`). |
| Auth | Jetons `Authorization: Bearer` ou certificats mTLS emis par suffix steward. Les endpoints sensibles a la gouvernance (freeze/unfreeze, affectations reservees) exigent `scope=sns.admin`. |
| Limites de debit | Les registrars partagent les buckets `torii.preauth_scheme_limits` avec les appelants JSON plus des limites de rafale par suffixe: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Telemetrie | Torii expose `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` pour les handlers du registrar (filtrer `scheme="norito_rpc"`); l'API incremente aussi `sns_registrar_status_total{result, suffix_id}`. |

## 2. Apercu des DTO

Les champs referencent les structs canoniques definis dans [`registry-schema.md`](./registry-schema.md). Toutes les charges utiles integrent `NameSelectorV1` + `SuffixId` pour eviter un routage ambigu.

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

## 3. Endpoints REST

| Endpoint | Methode | Payload | Description |
|----------|---------|---------|-------------|
| `/v1/sns/names` | POST | `RegisterNameRequestV1` | Enregistrer ou rouvrir un nom. Resout le tier de prix, valide les preuves de paiement/gouvernance, emet des evenements de registre. |
| `/v1/sns/names/{namespace}/{literal}/renew` | POST | `RenewNameRequestV1` | Prolonge le terme. Applique les fenetres de grace/redemption depuis la politique. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POST | `TransferNameRequestV1` | Transfere la propriete une fois les approbations de gouvernance jointes. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | PUT | `UpdateControllersRequestV1` | Remplace l'ensemble des controllers; valide les adresses de compte signees. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POST | `FreezeNameRequestV1` | Freeze guardian/council. Exige un ticket guardian et une reference au dossier de gouvernance. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | DELETE | `GovernanceHookV1` | Unfreeze apres remediation; s'assure que l'override du council est enregistre. |
| `/v1/sns/reserved/{selector}` | POST | `ReservedAssignmentRequestV1` | Affectation de noms reserves par steward/council. |
| `/v1/sns/policies/{suffix_id}` | GET | -- | Recupere le `SuffixPolicyV1` courant (cacheable). |
| `/v1/sns/names/{namespace}/{literal}` | GET | -- | Retourne le `NameRecordV1` courant + etat effectif (Active, Grace, etc.). |

**Encodage du selector:** le segment `{selector}` accepte I105, compresse, ou hex canonique selon ADDR-5; Torii le normalise via `NameSelectorV1`.

**Modele d'erreurs:** tous les endpoints retournent Norito JSON avec `code`, `message`, `details`. Les codes incluent `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Aides CLI (exigence du registrar manuel N0)

Les stewards de beta fermee peuvent desormais utiliser le registrar via la CLI sans fabriquer du JSON a la main:

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

- `--owner` prend par defaut le compte de configuration de la CLI; repetez `--controller` pour ajouter des comptes controller supplementaires (par defaut `[owner]`).
- Les flags inline de paiement mappent directement vers `PaymentProofV1`; passez `--payment-json PATH` quand vous avez deja un recu structure. Les metadonnees (`--metadata-json`) et les hooks de gouvernance (`--governance-json`) suivent le meme schema.

Les aides en lecture seule completent les repetitions:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Voir `crates/iroha_cli/src/commands/sns.rs` pour l'implementation; les commandes reutilisent les DTOs Norito decrits dans ce document afin que la sortie CLI corresponde byte for byte aux reponses Torii.

Des aides supplementaires couvrent les renouvellements, transferts et actions guardian:

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

`--governance-json` doit contenir un enregistrement `GovernanceHookV1` valide (proposal id, vote hashes, signatures steward/guardian). Chaque commande reflete simplement l'endpoint `/v1/sns/names/{namespace}/{literal}/...` correspondant pour que les operateurs de beta puissent repeter exactement les surfaces Torii que les SDKs appelleront.

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

Wire-format: hash du schema Norito en temps de compilation enregistre dans
`fixtures/norito_rpc/schema_hashes.json` (lignes `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, etc.).

## 5. Hooks de gouvernance et preuves

Chaque appel qui modifie l'etat doit joindre des preuves reutilisables pour la relecture:

| Action | Donnees de gouvernance requises |
|--------|-------------------------------|
| Enregistrement/renouvellement standard | Preuve de paiement referencant une instruction de settlement; aucun vote council requis sauf si le tier exige l'approbation steward. |
| Enregistrement de tier premium / affectation reservee | `GovernanceHookV1` referencant proposal id + steward acknowledgement. |
| Transfert | Hash du vote council + hash du signal DAO; clearance guardian quand le transfert est declenche par resolution de litige. |
| Freeze/Unfreeze | Signature du ticket guardian plus override du council (unfreeze). |

Torii verifie les preuves en verifiant:

1. L'identifiant de proposal existe dans le ledger de gouvernance (`/v1/governance/proposals/{id}`) et le statut est `Approved`.
2. Les hashes correspondent aux artefacts de vote enregistres.
3. Les signatures steward/guardian referencent les cles publiques attendues de `SuffixPolicyV1`.

Les controles en echec renvoient `sns_err_governance_missing`.

## 6. Exemples de workflow

### 6.1 Enregistrement standard

1. Le client interroge `/v1/sns/policies/{suffix_id}` pour recuperer les prix, la grace et les tiers disponibles.
2. Le client construit `RegisterNameRequestV1`:
   - `selector` derive du label I105 (prefere) ou compresse (second choix).
   - `term_years` dans les limites de la politique.
   - `payment` referencant le transfert du splitter tresorerie/steward.
3. Torii valide:
   - Normalisation du label + liste reservee.
   - Term/gross price vs `PriceTierV1`.
   - Montant de preuve de paiement >= prix calcule + frais.
4. En succes Torii:
   - Persiste `NameRecordV1`.
   - Emet `RegistryEventV1::NameRegistered`.
   - Emet `RevenueAccrualEventV1`.
   - Retourne le nouveau record + evenements.

### 6.2 Renouvellement pendant la grace

Les renouvellements pendant la grace incluent la requete standard plus la detection de penalite:

- Torii compare `now` vs `grace_expires_at` et ajoute les tables de surcharge de `SuffixPolicyV1`.
- La preuve de paiement doit couvrir la surcharge. Echec => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` enregistre le nouveau `expires_at`.

### 6.3 Freeze guardian et override du council

1. Guardian soumet `FreezeNameRequestV1` avec un ticket referencant l'id d'incident.
2. Torii deplace le record en `NameStatus::Frozen`, emet `NameFrozen`.
3. Apres remediation, le council emet un override; l'operateur envoie DELETE `/v1/sns/names/{namespace}/{literal}/freeze` avec `GovernanceHookV1`.
4. Torii valide l'override, emet `NameUnfrozen`.

## 7. Validation et codes d'erreur

| Code | Description | HTTP |
|------|-------------|------|
| `sns_err_reserved` | Label reserve ou bloque. | 409 |
| `sns_err_policy_violation` | Term, tier ou ensemble de controllers viole la politique. | 422 |
| `sns_err_payment_mismatch` | Mismatch de valeur ou d'asset dans la preuve de paiement. | 402 |
| `sns_err_governance_missing` | Artefacts de gouvernance requis absents/invalides. | 403 |
| `sns_err_state_conflict` | Operation non permise dans l'etat de cycle de vie actuel. | 409 |

Tous les codes apparaissent via `X-Iroha-Error-Code` et des enveloppes Norito JSON/NRPC structurees.

## 8. Notes d'implementation

- Torii stocke les auctions en attente sous `NameRecordV1.auction` et rejette les tentatives d'enregistrement direct tant que `PendingAuction`.
- Les preuves de paiement reutilisent les recus du ledger Norito; les services de tresorerie fournissent des APIs helper (`/v1/finance/sns/payments`).
- Les SDKs devraient envelopper ces endpoints avec des helpers fortement types pour que les wallets puissent presenter des raisons d'erreur claires (`ERR_SNS_RESERVED`, etc.).

## 9. Prochaines etapes

- Relier les handlers Torii au contrat de registre reel une fois les auctions SN-3 disponibles.
- Publier des guides SDK specifiques (Rust/JS/Swift) referencant cette API.
- Etendre [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) avec des liens croises vers les champs de preuve des hooks de gouvernance.
