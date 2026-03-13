---
lang: fr
direction: ltr
source: docs/portal/docs/sns/registrar-api.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Fonte canonica
Cette page est en ligne `docs/source/sns/registrar_api.md` et elle sert maintenant à
copia canonica do portail. L'archive est permanente pour les flux de traduction.
:::

# API du registraire SNS et des hooks de gouvernance (SN-2b)

**Statut :** Redigido 2026-03-24 -- donc révision du Nexus Core  
**Lien vers la feuille de route :** SN-2b "API du registraire et hooks de gouvernance"  
**Pré-requis :** Définitions du schéma dans [`registry-schema.md`](./registry-schema.md)

Cette note spécifique aux points de terminaison Torii, services gRPC, DTO de demande/réponse et
artefatos de gouvernance nécessaires pour exploiter ou registraire du Sora Name Service (SNS).
Il s'agit d'un contrat autorisé pour les SDK, les portefeuilles et l'enregistrement automatique précis,
rénover ou gérer des noms SNS.

## 1. Transport et authentique| Requis | Détails |
|-----------|---------|
| Protocoles | REST sanglot `/v2/sns/*` et service grRPC `sns.v1.Registrar`. Nous utilisons Norito-JSON (`application/json`) et Norito-RPC binaire (`application/x-norito`). |
| Authentification | Jetons `Authorization: Bearer` ou certifiés mTLS émis par suffix steward. Les points de terminaison sensibles sont une gouvernance (gel/dégel, attributs réservés) exigée par `scope=sns.admin`. |
| Limites des taxons | Les bureaux d'enregistrement partagent les buckets `torii.preauth_scheme_limits` avec les hamadores JSON mais les limites de burst par suffixe : `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Télémétrie | Torii expose `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` pour les gestionnaires du registraire (filtre `scheme="norito_rpc"`) ; une API également incrémentée `sns_registrar_status_total{result, suffix_id}`. |

## 2. Visa général de DTO

Les champs de référence sont les structures canoniques définies dans [`registry-schema.md`](./registry-schema.md). Toutes les charges incluent `NameSelectorV1` + `SuffixId` pour éviter toute ambiguïté de rotation.

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
|----------|--------|---------|---------------|
| `/v2/sns/registrations` | POSTER | `RegisterNameRequestV1` | Registrar ou reouvrir un nom. Résoudre le niveau de prix, valider les preuves de paiement/gouvernance, émettre des événements de registre. |
| `/v2/sns/registrations/{selector}/renew` | POSTER | `RenewNameRequestV1` | Estende o termo. Application Janelas de Grace/Redemption Da Politica. |
| `/v2/sns/registrations/{selector}/transfer` | POSTER | `TransferNameRequestV1` | Transférer la propriété lorsque la gouvernance est approuvée pour les anexadas. |
| `/v2/sns/registrations/{selector}/controllers` | METTRE | `UpdateControllersRequestV1` | Remplacer le groupe de contrôleurs ; valida enderecos de conta assinados. |
| `/v2/sns/registrations/{selector}/freeze` | POSTER | `FreezeNameRequestV1` | Gel du tuteur/conseil. Demander un gardien de ticket et une référence au dossier de gouvernance. |
| `/v2/sns/registrations/{selector}/freeze` | SUPPRIMER | `GovernanceHookV1` | Dégelez l'apos remediacao ; garantie dérogation do conseil registrado. |
| `/v2/sns/reserved/{selector}` | POSTER | `ReservedAssignmentRequestV1` | Attribution de noms réservés à l'intendant/conseil. |
| `/v2/sns/policies/{suffix_id}` | OBTENIR | -- | Busca `SuffixPolicyV1` actuel (cacheavel). |
| `/v2/sns/registrations/{selector}` | OBTENIR | -- | Retorna `NameRecordV1` actuel + estado efetivo (Active, Grace, etc.). |

**Codification du sélecteur :** le segment `{selector}` correspond à I105, compressé ou hexadécimal canonique conforme à ADDR-5 ; Torii normalise via `NameSelectorV1`.**Modèle d'erreur :** tous les points de terminaison renvoient le nom Norito JSON avec `code`, `message`, `details`. Les codes incluent `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Helpers CLI (requis du manuel du registraire N0)

Les stewards de la version bêta peuvent maintenant fonctionner ou s'enregistrer via CLI sans monter JSON manuellement :

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id xor#sora \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` padrao et un compte de configuration de la CLI ; repita `--controller` pour ajouter des détails sur le contrôleur supplémentaire (padrao `[owner]`).
- Les drapeaux en ligne du paiement mapeiam directement pour `PaymentProofV1` ; passe `--payment-json PATH` lorsque je vois un reçu établi. Les métadonnées (`--metadata-json`) et les crochets de gouvernance (`--governance-json`) suivent le même chemin.

Helpers de leitura somente completam os ensaios:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Voir `crates/iroha_cli/src/commands/sns.rs` pour la mise en œuvre ; Les commandes réutilisent les DTO Norito décrites dans ce document pour que la CLI coïncide octet par octet avec les réponses de Torii.

Helpers adicionais cobrem renovacoes, transferencias e acoes de tuteur:

```bash
# Renew an expiring name
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id xor#sora \
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

`--governance-json` doit vérifier un enregistrement `GovernanceHookV1` valide (identifiant de proposition, hachages de vote, intendant/tuteur d'Assinaturas). Cette commande consiste simplement à sélectionner le point de terminaison `/v2/sns/registrations/{selector}/...` correspondant aux opérateurs de version bêta qui utilisent exactement la superficie Torii que les SDK remplacent.## 4. Servico gRPC

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

Format filaire : hachage de l'esquema Norito au rythme de la compilation enregistré
`fixtures/norito_rpc/schema_hashes.json` (ligne `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, etc.).

## 5. Crochets de gouvernance et de preuves

Aujourd'hui, il est important de modifier l'état en indiquant des preuves adéquates pour la relecture :

| Açao | Données de gouvernance requises |
|------|----------------------------------------------|
| Registro/renovacao padrao | Prova de pagamento referenciando uma instrucao de règlement ; nao exige que le conseil vote pour moins que le niveau exija aprovacao do steward. |
| Registre de niveau premium / atribuicao reservada | `GovernanceHookV1` identifiant de proposition de référence + accusé de réception du steward. |
| Transfert | Hash de vote du conseil + hash de sinal DAO ; autorisation du tuteur quando a transferencia e acionada por resolucao de disputa. |
| Geler/Dégeler | Assinature du gardien du ticket mais annulation du conseil (dégel). |

Torii vérifie en tant que fournisseur :

1. La proposition n'existe pas de grand livre de gouvernance (`/v2/governance/proposals/{id}`) ni de statut et `Approved`.
2. Les hachages correspondent aux artefatos de voto registrados.
3. Assinaturas steward/guardian referenciam as chaves publicas esperadas de `SuffixPolicyV1`.

Falhas renvoie le nom `sns_err_governance_missing`.

## 6. Exemples de flux de travail

### 6.1 Registre Padrao1. Le client consulte `/v2/sns/policies/{suffix_id}` pour obtenir des prix, grâce et niveaux disponibles.
2. Le client monte `RegisterNameRequestV1` :
   - `selector` dérivé du label I105 (préféré) ou compressé (segunda melhor opcao).
   - `term_years` dans les limites de la politique.
   - `payment` référence au transfert du répartiteur tesouraria/steward.
3. Torii valide :
   - Normalizacao de label + lista reservada.
   - Durée/prix brut vs `PriceTierV1`.
   - Montant de la Prova de pagamento >= preco calculé + frais.
4. Avec succès Torii :
   - Persister `NameRecordV1`.
   - Émite `RegistryEventV1::NameRegistered`.
   - Émite `RevenueAccrualEventV1`.
   - Retour au nouveau registre + événements.

### 6.2 Renovation durante grace

Les rénovations pendant la grâce incluent les exigences requises, mais la détection des pénalités :

- Torii comparé à `now` vs `grace_expires_at` et ajout des tableaux de surveillance de `SuffixPolicyV1`.
- Une preuve de paiement doit cobrir a sobretaxa. Falha => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` enregistre le nouveau `expires_at`.

### 6.3 Gel du tuteur et dérogation du conseil

1. Guardian envoie le ticket `FreezeNameRequestV1` en référence à l'identifiant de l'incident.
2. Torii déplacez l'enregistrement pour `NameStatus::Frozen`, émettez `NameFrozen`.
3. Après correction, le conseil émet une dérogation ; L'opérateur envoie DELETE `/v2/sns/registrations/{selector}/freeze` avec `GovernanceHookV1`.
4. Torii valide le remplacement, émet `NameUnfrozen`.## 7. Validation et codes d'erreur

| Codigo | Description | HTTP |
|--------|-----------|------|
| `sns_err_reserved` | Étiquette réservée ou bloquée. | 409 |
| `sns_err_policy_violation` | Termo, tier ou conjunto de contrôleurs viole la politique. | 422 |
| `sns_err_payment_mismatch` | Inadéquation de la valeur ou des actifs à la preuve du paiement. | 402 |
| `sns_err_governance_missing` | Artefatos de gouvernance requeridos ausentes/invalidos. | 403 |
| `sns_err_state_conflict` | L'opération n'est pas autorisée dans l'état actuel du cycle de vie. | 409 |

Tous les codes apparaissent via `X-Iroha-Error-Code` et les enveloppes Norito JSON/NRPC créées.

## 8. Notes de mise en œuvre

- Torii armazena leiloes pendentes em `NameRecordV1.auction` et rejeita tentativas de registro directementto enquanto estiver `PendingAuction`.
- Provas de pagamento réutilizam recibos do ledger Norito ; services de tesouraria pour l'assistance API (`/v2/finance/sns/payments`).
- Les SDK développent des points de terminaison et des aides fortement conçues pour que les portefeuilles présentent des motifs d'erreur clairs (`ERR_SNS_RESERVED`, etc.).

## 9. Proximos passos

- Connectez les gestionnaires du Torii au contrat d'enregistrement réel lorsque les leiloes SN-3 choisissent.
- Publier des guides spécifiques du SDK (Rust/JS/Swift) référençant cette API.
- Estender [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) avec des liens croisés pour les champs de preuves de gouvernance.