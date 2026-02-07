---
lang: he
direction: rtl
source: docs/portal/docs/sns/registrar-api.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::הערה מקור קנוניק
Cette page reflete `docs/source/sns/registrar_api.md` et sert desormais de
עותק canonique du portail. Le fichier source est conserve pour les flux de
מסירה.
:::

# API של הרשם SNS et hooks de governance (SN-2b)

**סטטוס:** Redige 2026-03-24 -- en revue par Nexus Core  
**מפת הדרכים של Lien:** SN-2b "Registrar API & Governance Hooks"  
**דרישה מוקדמת:** Definitions de schema dans [`registry-schema.md`](./registry-schema.md)

הערה Cette מציינת את נקודות הקצה Torii, שירותי gRPC, DTOs de Request/Reponse
et artefacts de governance necessaires pour operer le registrar du Sora שם
שירות (SNS). C'est le contrat de reference pour les SDKs, ארנקים et
אוטומציה qui doivent enregistrer, renouveler ou gerer des noms SNS.

## 1. תחבורה ואימות

| נחישות | פירוט |
|--------|--------|
| פרוטוקולים | REST sous `/v1/sns/*` et service gRPC `sns.v1.Registrar`. Les deux acceptent Norito-JSON (`application/json`) ו-Norito-RPC binaire (`application/x-norito`). |
| Auth | Jetons `Authorization: Bearer` או אישורי mTLS emis par suffix steward. Les endpoints sensibles a la governance (הקפאה/ביטול ההקפאה, שמירה על השפעות) דרושים `scope=sns.admin`. |
| מגבלות חיוב | Les registrars partagent les buckets `torii.preauth_scheme_limits` avec les appelants JSON plus des limites de rafale par סיומת: `sns.register`, `sns.renew`, `sns.controller`, Norito. |
| טלמטריה | Torii לחשוף `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` למטפלים של הרשם (מסנן `scheme="norito_rpc"`); l'API incremente aussi `sns_registrar_status_total{result, suffix_id}`. |

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

## 3. נקודות קצה REST

| נקודת קצה | מתודה | מטען | תיאור |
|--------|--------|--------|----------------|
| `/v1/sns/registrations` | פוסט | `RegisterNameRequestV1` | הרשום או rouvrir un nom. Resout le tier de prix, valide les preuves de paiement/gouvernance, emet des evenements de registre. |
| `/v1/sns/registrations/{selector}/renew` | פוסט | `RenewNameRequestV1` | הארך את הטרמה. Applique les fenetres de grace/גאולה depuis la politique. |
| `/v1/sns/registrations/{selector}/transfer` | פוסט | `TransferNameRequestV1` | Transfere la propriete une fois les approbations de gouvernance jointes. |
| `/v1/sns/registrations/{selector}/controllers` | PUT | `UpdateControllersRequestV1` | Remplace l'ensemble des controllers; valide les adresses de compte signees. |
| `/v1/sns/registrations/{selector}/freeze` | פוסט | `FreezeNameRequestV1` | הקפאת אפוטרופוס/מועצה. קבל כרטיס אפוטרופוס ויחידה הפניה au dossier de gouvernance. |
| `/v1/sns/registrations/{selector}/freeze` | מחק | `GovernanceHookV1` | ביטול הקפאה של תיקון אפרה; s'assure que l'override du Council est להירשם. |
| `/v1/sns/reserved/{selector}` | פוסט | `ReservedAssignmentRequestV1` | חיבה דה נומס מילואים פר דייל/מועצה. |
| `/v1/sns/policies/{suffix_id}` | קבל | -- | Recupere le `SuffixPolicyV1` courant (ניתן לאחסון במטמון). |
| `/v1/sns/registrations/{selector}` | קבל | -- | Retourne le `NameRecordV1` courant + etat effectif (אקטיב, גרייס וכו'). |**Encodage du selector:** קטע `{selector}` מקבל IH58, דחיסה, או hex canonique selon ADDR-5; Torii לנרמל דרך `NameSelectorV1`.

**דגם שגיאות:** tous les endpoints retournent Norito JSON avec `code`, `message`, `details`. הקודים כוללים `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Aides CLI (דרישת מדריך הרשם N0)

Les stewards de beta fermee peuvent desormais utiliser le registrar via la CLI sans fabriquer du JSON a la main:

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

- `--owner` prend par defaut le compte de configuration de la CLI; repetez `--controller` pour ajouter des comptes controller supplementaires (par defaut `[owner]`).
- Les flags inline de paiement mappt direction vers `PaymentProofV1`; passez `--payment-json PATH` quand vous avez deja un recu מבנה. Les metadonnees (`--metadata-json`) et les hooks de governance (`--governance-json`) תואם לסכימת הממים.

Les aides and lecture הם חזרות שלמות:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Voir `crates/iroha_cli/src/commands/sns.rs` pour l'implementation; les commandes reutilisent les DTOs Norito decrits dans ce document afin que la sortie CLI corresponde byte for byte aux reponses Torii.

עוזרים משלימים לחידושים, העברות ופעולות אפוטרופוס:

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
  --new-owner ih58... \
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

`--governance-json` תגדיר את הרישום `GovernanceHookV1` תקף (מזהה הצעה, גיבוב הצבעה, דייל/אפוטרופוס חתימות). Chaque commande reflee simplement l'endpoint `/v1/sns/registrations/{selector}/...` correspondant pour que les operators de beta puissent repeter exactement les משטחים Torii que les SDKs appelleront.

## 4. שירות gRPC

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

פורמט חוט: hash du schema Norito בזמני קומפילציה הרשום
`fixtures/norito_rpc/schema_hashes.json` (lignes `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1` וכו').

## 5. Hooks de governance et preuves

Chaque appel qui modifie l'etat doit joindre des preuves reutilisables pour la relecture:

| פעולה | Donnees de governance requises |
|--------|-------------------------------------|
| תקן רישום/חידוש | Preuve de paiement referencant une instruction de settlement; מועצת ההצבעה של aucun דרישה לסדרת ההסכמה. |
| רישום פרמיית שכבת/השפעה | `GovernanceHookV1` מזהה הצעה רפרנטית + אישור דייל. |
| העברה | חש דו קול מועצת + חש דו אות DAO; אפוטרופוס אישור quand le transfert est declenche par resolution de litige. |
| הקפאה/בטל הקפאה | Signature du ticket guardian plus override du Council (ביטול הקפאה). |

Torii בדוק את הקודמים והאימות:

1. L'identifiant de offer existe dans le Ledger de governance (`/v1/governance/proposals/{id}`) et le statut est `Approved`.
2. כתב Les hashes aux artefacts de vote enregistres.
3. Les חתימות דייל/אפוטרופוס התייחסות Les cles publiques attendues de `SuffixPolicyV1`.

Les controls en echec renvoient `sns_err_governance_missing`.

## 6. דוגמאות לזרימת עבודה### 6.1 תקן רישום

1. חקירת הלקוח `/v1/sns/policies/{suffix_id}` pour recuperer les prix, la grace et les tiers disponibles.
2. Le client construit `RegisterNameRequestV1`:
   - `selector` נגזר מהתווית IH58 (עדיף) או דחיסה (בחירה שנייה).
   - `term_years` dans les limites de la politique.
   - `payment` referencant le transfert du splitter tresorerie/דייל.
3. תקף Torii:
   - נורמליזציה תווית + רשימת שמירה.
   - טווח/מחיר ברוטו לעומת `PriceTierV1`.
   - Montant de preuve de paiement >= prix calcule + frais.
4. En success Torii:
   - Persiste `NameRecordV1`.
   - אמת `RegistryEventV1::NameRegistered`.
   - אמת `RevenueAccrualEventV1`.
   - Retourne le nouveau record + evenements.

### 6.2 תליון Renouvellement la grace

תליון Les renouvellements la grace incluent la requete standard plus la detection de penalite:

- Torii השווה בין `now` ל-`grace_expires_at` ו-ajoute les tables de surcharge de `SuffixPolicyV1`.
- La preuve de paiement doit couvrir la תשלום נוסף. Echec => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` הרשמה ל-Nouveau `expires_at`.

### 6.3 הקפאת האפוטרופוס וביטול המועצה

1. Guardian soumet `FreezeNameRequestV1` avec un ticket referencant l'id d'incident.
2. Torii deplace le record en `NameStatus::Frozen`, emet `NameFrozen`.
3. תיקון אפרה, le Council emet un override; l'operateur envoie DELETE `/v1/sns/registrations/{selector}/freeze` avec `GovernanceHookV1`.
4. Torii valide l'override, emet `NameUnfrozen`.

## 7. Validation et codes d'erreur

| קוד | תיאור | HTTP |
|------|-------------|------|
| `sns_err_reserved` | תווית reserve ou bloque. | 409 |
| `sns_err_policy_violation` | מונח, tier ou אנסמבל דה בקרי ויולה לפוליטיקה. | 422 |
| `sns_err_payment_mismatch` | חוסר התאמה דה valeur ou d'asset dans la preuve de paiement. | 402 |
| `sns_err_governance_missing` | חפצי שלטון דורשים נעדרים/פסולים. | 403 |
| `sns_err_state_conflict` | Operation non permise dans l'etat de cycle de vie actuel. | 409 |

ניתן לראות את הקודים באמצעות `X-Iroha-Error-Code` ו-des enveloppes Norito מבני JSON/NRPC.

## 8. הערות ליישום

- Torii stocke les auctions en attente sous `NameRecordV1.auction` et rejette les tentatives d'enregistrement Direct tant que `PendingAuction`.
- Les preuves de paiement reutilisent les recus du ledger Norito; les services de tresorerie fournissent des APIs helper (`/v1/finance/sns/payments`).
- Les SDKs devraient envelopper ces endpoints avec des helpers fortement types pour que les wallets puissent presenter des raisons d'erreur claires (`ERR_SNS_RESERVED`, וכו').

## 9. Prochaines etapes

- Relier les handlers Torii au contrat de registre reel une fois les auctions SN-3 disponibles.
- Publier des guides מפרט SDK (Rust/JS/Swift) מתייחס ל-API.
- Etendre [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) avec des liens croises vers les champs de preuve des hooks de governance.