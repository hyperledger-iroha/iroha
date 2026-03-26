---
lang: fr
direction: ltr
source: docs/portal/docs/sns/registrar-api.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sns/registrar_api.md` کی عکاسی کرتا ہے اور اب پورٹل
کی کینونیکل کاپی ہے۔ سورس فائل ترجمہ PRs کے لئے برقرار رہتی ہے۔
:::

# API SNS Registrar et hooks (SN-2b)

**حالت:** 2026-03-24 - Nexus Core ریویو کے تحت  
**روڈمیپ لنک :** SN-2b "API du registraire et hooks de gouvernance"  
**پیشگی شرائط:** اسکیمہ تعریفیں [`registry-schema.md`](./registry-schema.md) میں۔

Avec les points de terminaison Torii, les services gRPC, les DTO de requête/réponse et la gouvernance
artefacts et le registraire du Sora Name Service (SNS) sont disponibles.
درکار ہیں۔ SDK, portefeuilles et automatisation pour les applications SNS
رجسٹر، renouveler et gérer کرنا چاہتے ہیں۔

## 1. ٹرانسپورٹ اور توثیق| شرط | تفصیل |
|-----|-------|
| پروٹوکولز | REST `/v1/sns/*` est un service gRPC `sns.v1.Registrar`۔ Fichier Norito-JSON (`application/json`) et Norito-RPC (`application/x-norito`) pour le téléchargement |
| Authentification | Jetons `Authorization: Bearer` et certificats mTLS et gestionnaire de suffixe et gestionnaire de suffixe points de terminaison sensibles à la gouvernance (gel/dégel, affectations réservées) par `scope=sns.admin` pour |
| ریٹ حدود | Les bureaux d'enregistrement `torii.preauth_scheme_limits` regroupent les appelants JSON et les majuscules éclatées : `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`۔ |
| ٹیلیمیٹری | Gestionnaires d'enregistrement Torii pour `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` pour le filtre (filtre `scheme="norito_rpc"`) API pour `sns_registrar_status_total{result, suffix_id}` pour le client |

## 2. DTO خلاصہ

فیلڈز [`registry-schema.md`](./registry-schema.md) میں متعین structures canoniques et se référer à کرتی ہیں۔ Charges utiles `NameSelectorV1` + `SuffixId` pour le routage et le routage

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

## 3. Points de terminaison REST| Point de terminaison | طریقہ | Charge utile | تفصیل |
|--------------|-------|---------|-------|
| `/v1/sns/names` | POSTER | `RegisterNameRequestV1` | نام رجسٹر یا دوبارہ کھولنا۔ niveau de tarification et les preuves de paiement/gouvernance et les événements de registre émettent des preuves |
| `/v1/sns/names/{namespace}/{literal}/renew` | POSTER | `RenewNameRequestV1` | مدت بڑھاتا ہے۔ پالیسی سے Grace/Redemption Windows نافذ کرتا ہے۔ |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POSTER | `TransferNameRequestV1` | حکمرانی approbations لگنے کے بعد propriété منتقل کرتا ہے۔ |
| `/v1/sns/names/{namespace}/{literal}/controllers` | METTRE | `UpdateControllersRequestV1` | contrôleurs کا سیٹ بدلتا ہے؛ adresses de compte signées کی توثیق کرتا ہے۔ |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POSTER | `FreezeNameRequestV1` | gel du tuteur/du conseil۔ ticket de gardien et dossier de gouvernance کا حوالہ درکار۔ |
| `/v1/sns/names/{namespace}/{literal}/freeze` | SUPPRIMER | `GovernanceHookV1` | remédiation کے بعد dégeler؛ dérogation du conseil |
| `/v1/sns/reserved/{selector}` | POSTER | `ReservedAssignmentRequestV1` | noms réservés کی intendant/conseil کی طرف سے affectation۔ |
| `/v1/sns/policies/{suffix_id}` | OBTENIR | -- | `SuffixPolicyV1` موجودہ حاصل کرتا ہے (mise en cache)۔ |
| `/v1/sns/names/{namespace}/{literal}` | OBTENIR | -- | موجودہ `NameRecordV1` + موثر حالت (Active, Grace وغیرہ) et کرتا ہے۔ |

**Encodage du sélecteur :** Segment de chemin `{selector}` i105, compressé (`sora`) et hexadécimal canonique ADDR-5 pour le segment de chemin d'accès. Torii `NameSelectorV1` pour normaliser les choses**Modèle d'erreur :** Les points de terminaison Norito JSON `code`, `message`, `details` sont affichés. Codes `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing` en anglais

### 3.1 Assistants CLI (N0 دستی registrar ضرورت)

Les stewards bêta fermés de la CLI et du registraire sont également compatibles avec JSON :

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

- `--owner` pour un compte de configuration CLI Les comptes du contrôleur sont `--controller` et `[owner]` par défaut.
- Indicateurs de paiement en ligne pour la carte `PaymentProofV1` et la carte جب reçu structuré ہو تو `--payment-json PATH` دیں۔ Métadonnées (`--metadata-json`) et hooks de gouvernance (`--governance-json`) pour plus de détails

Répétitions des assistants en lecture seule:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Implémentation selon `crates/iroha_cli/src/commands/sns.rs` commandes Commandes de sortie Norito DTO Réponses de sortie CLI Torii ساتھ octet pour octet میل کھائے۔

Renouvellements d'aides supplémentaires, transferts et actions du tuteur et autres aides :

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
  --new-owner soraカタカナ... \
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

`--governance-json` میں درست `GovernanceHookV1` ریکارڈ ہونا چاہیے (identifiant de proposition, hachages de vote, signatures d'intendant/tuteur)۔ Le point de terminaison `/v1/sns/names/{namespace}/{literal}/...` est utilisé pour les opérateurs bêta et les surfaces Torii répètent. Les SDK sont également disponibles

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
```Format Wire : hachage de schéma Norito au moment de la compilation
`fixtures/norito_rpc/schema_hashes.json` میں درج ہے (lignes `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, et).

## 5. La gouvernance s'accroche aux preuves

ہر appel en mutation کو replay کے لئے موزوں preuve منسلک کرنا ہوتا ہے :

| Actions | Données de gouvernance requises |
|--------|--------------------------|
| Standard enregistrer/renouveller | Instruction de règlement et référence à une preuve de paiement vote du conseil کی ضرورت نہیں جب تک niveau کو approbation du délégué syndical |
| Registre de niveau Premium / affectation réservée | `GovernanceHookV1` et identifiant de proposition + accusé de réception du steward se référer à کرتا ہے۔ |
| Transfert | Hachage du vote du Conseil + hachage du signal DAO؛ autorisation du tuteur جب résolution des litiges de transfert سے déclencheur ہو۔ |
| Geler/Dégeler | Signature du ticket du gardien کے ساتھ dérogation au conseil (dégel)۔ |

Torii preuves en anglais:

1. Grand livre de gouvernance d'identifiant de proposition (`/v1/governance/proposals/{id}`) میں موجود ہے اور statut `Approved` ہے۔
2. hashs ریکارڈ شدہ votes artefacts سے match کرتے ہیں۔
3. signatures de l'intendant/tuteur `SuffixPolicyV1` pour les clés publiques et se référer à la déclaration

Échec des contrôles `sns_err_governance_missing` واپس کرتے ہیں۔

## 6. Exemples de workflow

### 6.1 Inscription standard1. Client `/v1/sns/policies/{suffix_id}` pour une requête sur les tarifs et les niveaux de grâce
2. Client `RegisterNameRequestV1` utilisé :
   - `selector` pour i105, deuxième meilleure étiquette compressée (`sora`) et dérivée
   - `term_years` پالیسی حدود میں۔
   - Transfert répartiteur de trésorerie/intendant `payment` et voir ici
3. Torii valide la question :
   - Normalisation des étiquettes + liste réservée۔
   - Durée/prix brut vs `PriceTierV1`.
   - Montant du justificatif de paiement >= prix calculé + frais۔
4. کامیابی pour Torii :
   - `NameRecordV1` محفوظ کرتا ہے۔
   - `RegistryEventV1::NameRegistered` émet کرتا ہے۔
   - `RevenueAccrualEventV1` émet کرتا ہے۔
   - نیا record + événements واپس کرتا ہے۔

### 6.2 Renouvellement pendant la grâce

Renouvellements de grâce comme demande standard et détection de pénalité comme :

- Torii `now` vs `grace_expires_at` et `SuffixPolicyV1` dans les tableaux de suppléments
- Preuve de paiement et couverture supplémentaire Échec => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` et `expires_at` ریکارڈ کرتا ہے۔

### 6.3 Gel des gardiens et dérogation au conseil1. Guardian `FreezeNameRequestV1` soumettre un ticket pour un identifiant d'incident et un ticket pour un ticket
2. L'enregistrement Torii et `NameStatus::Frozen` émettent un message d'erreur.
3. Assainissement et dérogation du conseil opérateur DELETE `/v1/sns/names/{namespace}/{literal}/freeze` et `GovernanceHookV1` sont en cours de réalisation
4. Torii override validate کرتا ہے، `NameUnfrozen` émet کرتا ہے۔

## 7. Validation et codes d'erreur

| Codes | Descriptif | HTTP |
|------|-------------|------|
| `sns_err_reserved` | Libellé réservé یا bloqué ہے۔ | 409 |
| `sns_err_policy_violation` | Terme, contrôleurs de niveau pour les contrôleurs de niveau | 422 |
| `sns_err_payment_mismatch` | Preuve de paiement ou inadéquation des actifs | 402 |
| `sns_err_governance_missing` | Artefacts de gouvernance requis غائب/invalid ہیں۔ | 403 |
| `sns_err_state_conflict` | État du cycle de vie et fonctionnement autorisé | 409 |

Codes de code `X-Iroha-Error-Code` et enveloppes structurées Norito JSON/NRPC en surface et en surface

## 8. Notes de mise en œuvre

- Torii enchères en attente pour `NameRecordV1.auction` pour les tentatives d'enregistrement direct et pour `PendingAuction` pour les tentatives d'enregistrement direct et pour rejeter les enchères
- Preuves de paiement Norito Reçus du grand livre دوبارہ استعمال کرتے ہیں؛ API d'assistance aux services de trésorerie (`/v1/finance/sns/payments`)
- Les SDK, les points de terminaison et les assistants fortement typés, ainsi que les enveloppes, les portefeuilles et les raisons d'erreur (`ERR_SNS_RESERVED`, etc.)## 9. Prochaines étapes

- Enchères SN-3 pour les gestionnaires Torii et le contrat de registre avec fil de fer
- Les guides spécifiques au SDK (Rust/JS/Swift) sont également disponibles sur l'API et se réfèrent à eux.
- [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) pour les champs de preuves de crochet de gouvernance et les liens croisés pour étendre les liens