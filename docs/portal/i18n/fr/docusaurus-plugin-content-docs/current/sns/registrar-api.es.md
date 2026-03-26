---
lang: fr
direction: ltr
source: docs/portal/docs/sns/registrar-api.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Fuente canonica
Cette page reflète `docs/source/sns/registrar_api.md` et maintenant, monsieur comme la
copie canonique du portail. Le fichier source est conservé pour les flux de
traduction.
:::

# API de l'enregistreur SNS et des hooks de gouvernance (SN-2b)

**État :** Borrador 2026-03-24 -- basse révision de Nexus Core  
**Enlace del roadmap:** SN-2b "API du registraire et hooks de gouvernance"  
**Prérequis :** Définitions du schéma dans [`registry-schema.md`](./registry-schema.md)

Cette note spécifique aux points de terminaison Torii, services gRPC, DTO de sollicitude
réponse et artefacts de gouvernement nécessaires pour faire fonctionner le registrateur du
Service de noms Sora (SNS). Il s'agit du contrat autorisé pour les SDK, les portefeuilles et
automatización qui necesitan registrar, renovar o gestionar nombres SNS.

## 1. Transport et authentification| Requis | Détails |
|-----------|---------|
| Protocoles | REST sous `/v1/sns/*` et service gRPC `sns.v1.Registrar`. Nous acceptons le binaire Norito-JSON (`application/json`) et Norito-RPC (`application/x-norito`). |
| Authentification | Jetons `Authorization: Bearer` ou certifiés mTLS émis par suffix steward. Les points de terminaison sensibles à la gestion (gel/dégel, attributions réservées) nécessitent `scope=sns.admin`. |
| Limites de la tâche | Les bureaux d'enregistrement partagent les buckets `torii.preauth_scheme_limits` avec les appelants JSON avec des limites de rafaga par suffixe : `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Télémétrie | Torii expose `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` pour les gestionnaires du registrateur (filtrer `scheme="norito_rpc"`) ; L'API incrémente également `sns_registrar_status_total{result, suffix_id}`. |

## 2. Résumé du DTO

Les champs référencent les structures canoniques définies dans [`registry-schema.md`](./registry-schema.md). Toutes les charges incluent `NameSelectorV1` + `SuffixId` pour éviter toute ambiguïté sur la route.

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

## 3. Points de terminaison REST| Point de terminaison | Méthode | Charge utile | Description |
|--------------|--------|---------|-------------|
| `/v1/sns/names` | POSTER | `RegisterNameRequestV1` | Registrar ou reouvrir un numéro. Résoudre le niveau de prix, valider les essais de paiement/gouvernement, émettre des événements de registre. |
| `/v1/sns/names/{namespace}/{literal}/renew` | POSTER | `RenewNameRequestV1` | Étendre le terminus. Appliquez les ventanas de gracia/redencion segun la politique. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POSTER | `TransferNameRequestV1` | Transférer la propriété una vez adjuntas les aprobaciones de gobernanza. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | METTRE | `UpdateControllersRequestV1` | Remplacez l'ensemble des contrôleurs ; valida direcciones de cuenta firmadas. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POSTER | `FreezeNameRequestV1` | Gel du tuteur/conseil. Exiger un ticket de tuteur et une référence au dossier de gouvernement. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | SUPPRIMER | `GovernanceHookV1` | Dégeler la remédiation ; asegura outrepasser le conseil enregistré. |
| `/v1/sns/reserved/{selector}` | POSTER | `ReservedAssignmentRequestV1` | Assignation de nombres réservés au délégué/conseil. |
| `/v1/sns/policies/{suffix_id}` | OBTENIR | -- | Obtenez `SuffixPolicyV1` réel (mis en cache). |
| `/v1/sns/names/{namespace}/{literal}` | OBTENIR | -- | Devuelve `NameRecordV1` actuel + état effectif (Active, Grace, etc.). |

**Codification du sélecteur :** le segment `{selector}` accepte i105, compressé ou hexadécimal canonique selon ADDR-5 ; Torii pour normaliser via `NameSelectorV1`.**Modèle d'erreur :** tous les points de terminaison ont été développés avec Norito JSON avec `code`, `message`, `details`. Les codes incluent `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Helpers CLI (requisito de registrador manuel N0)

Les stewards de la version bêta certifiée peuvent utiliser le registrateur via la CLI sans utiliser JSON à la main :

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

- `--owner` indique par défaut le compte de configuration de la CLI ; Répétez `--controller` pour ajouter des comptes supplémentaires au contrôleur (par défaut `[owner]`).
- Les drapeaux en ligne de page mappée directement sur `PaymentProofV1` ; usa `--payment-json PATH` quand vous avez un recibo structuré. Les métadonnées (`--metadata-json`) et les crochets de gouvernance (`--governance-json`) suivent le même client.

Helpers de solo lectura completan los ensayos :

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Ver `crates/iroha_cli/src/commands/sns.rs` pour la mise en œuvre ; Les commandes réutilisent les DTO Norito décrits dans ce document pour que la sortie de la CLI coïncide octet par octet avec les réponses de Torii.

Aides supplémentaires pour les rénovations, les transferts et les actions de tuteur :

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
````--governance-json` doit contenir un registre valide `GovernanceHookV1` (identifiant de proposition, hachages de vote, sociétés de steward/tuteur). Il suffit de commander pour refléter le point de terminaison `/v1/sns/names/{namespace}/{literal}/...` correspondant afin que les opérateurs bêta testent exactement la surface Torii qui appelle les SDK.

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

Format filaire : hachage de l'esquema Norito dans le temps de compilation enregistré
`fixtures/norito_rpc/schema_hashes.json` (filas `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, etc.).

## 5. Crochets de gouvernance et de preuve

Chaque fois que vous êtes en état, vous devez ajouter des preuves appropriées pour la relecture :

| Accion | Données de gouvernance requises |
|--------|-------------------------------|
| Norme d'enregistrement/rénovation | Prueba de pago que referencia unea instruccion de règlement ; il n'est pas nécessaire de voter au conseil pour que le niveau requiera l'approbation du steward. |
| Registro premium / asignación reservada | `GovernanceHookV1` référence à l'identifiant de la proposition + accusé de réception du steward. |
| Transfert | Hash de vote du conseil + hachage du sénal DAO ; autorisation du tuteur lorsque le transfert est activé pour la résolution du litige. |
| Geler/Dégeler | Firma de ticket tuteur mas annuler del conseil (dégeler). |

Torii vérifier les essais effectués :1. La proposition existe dans le grand livre de l'État (`/v1/governance/proposals/{id}`) et le statut est `Approved`.
2. Les hachages coïncident avec les artefacts de vote enregistrés.
3. Firmas de steward/guardian referencian las claves publicas esperadas de `SuffixPolicyV1`.

Fallos a développé `sns_err_governance_missing`.

## 6. Exemples de flujo

### 6.1 Registre standard

1. Le client consulte `/v1/sns/policies/{suffix_id}` pour obtenir des prix, gracia et niveaux disponibles.
2. Le client arma `RegisterNameRequestV1` :
   - `selector` dérivé de l'étiquette i105 (préféré) ou compressé (seconde meilleure option).
   - `term_years` dans les limites de la politique.
   - `payment` qui fait référence au transfert du répartiteur tesoreria/steward.
3. Torii valide :
   - Normalisation du label + liste réservée.
   - Durée/prix brut vs `PriceTierV1`.
   - Montant du proof de pago >= precio calculé + frais.
4. En quittant Torii :
   - Persister `NameRecordV1`.
   - Émite `RegistryEventV1::NameRegistered`.
   - Émite `RevenueAccrualEventV1`.
   - Créez le nouveau registre + événements.

### 6.2 Rénovation durante gracia

Les rénovations gracieuses incluent la demande standard avec détection de pénalité :

- Torii compare `now` vs `grace_expires_at` et regroupe les tableaux de chargement à partir de `SuffixPolicyV1`.
- La prueba de pago doit curir el recargo. Fallo => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` enregistre le nouveau `expires_at`.### 6.3 Gelamiento du gardien et remplacement du conseil

1. Guardian envoie `FreezeNameRequestV1` avec un ticket faisant référence à l'identifiant de l'incident.
2. Torii doit s'inscrire à `NameStatus::Frozen`, émettre `NameFrozen`.
3. Après correction, le conseil émet une dérogation ; l’opérateur envoie DELETE `/v1/sns/names/{namespace}/{literal}/freeze` avec `GovernanceHookV1`.
4. Torii valide le remplacement, émet `NameUnfrozen`.

## 7. Validation et codes d'erreur

| Codigo | Description | HTTP |
|--------|-------------|------|
| `sns_err_reserved` | Étiquette réservée ou bloquée. | 409 |
| `sns_err_policy_violation` | Terme, niveau ou ensemble de contrôleurs viola la politique. | 422 |
| `sns_err_payment_mismatch` | Inadéquation de la valeur ou de l'actif dans l'essai de paiement. | 402 |
| `sns_err_governance_missing` | Artefactos de gobernanza requeridos ausentes/invalidos. | 403 |
| `sns_err_state_conflict` | L’opération n’est pas autorisée dans l’état actuel du cycle de vie. | 409 |

Tous les codes sont vendus via `X-Iroha-Error-Code` et les enveloppes Norito JSON/NRPC structurées.

## 8. Notes de mise en œuvre

- Torii garde les subastas pendientes en `NameRecordV1.auction` et demande l'intention de s'inscrire directement ici en `PendingAuction`.
- Les essais de paiement réutilisent les recettes du grand livre Norito ; services d'assistance API éprouvés par tesoreria (`/v1/finance/sns/payments`).
- Les SDK doivent inclure ces points de terminaison avec des aides adaptées pour que les portefeuilles doivent avoir des raisons d'erreur (`ERR_SNS_RESERVED`, etc.).## 9. Proximos pasos

- Connectez les gestionnaires de Torii au contrat d'enregistrement réel une fois que vous lisez les sous-titres SN-3.
- Publier des guides spécifiques du SDK (Rust/JS/Swift) qui font référence à cette API.
- Extender [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) avec des liens croisés aux champs de preuves de crochet de gouvernance.