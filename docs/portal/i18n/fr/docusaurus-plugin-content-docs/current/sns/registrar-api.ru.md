---
lang: fr
direction: ltr
source: docs/portal/docs/sns/registrar-api.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Cette page renvoie `docs/source/sns/registrar_api.md` et vous permet de vous connecter
Portail de copie canonique. Il s'agit d'un rendez-vous pour les relations publiques.
:::

# API d'enregistrement SNS et d'installation de téléphones (SN-2b)

**État :** Publié le 2026-03-24 — pour la mise à jour Nexus Core  
**Feuille de route :** SN-2b "API du registraire et hooks de gouvernance"  
**Procédures :** Schémas de maintenance dans [`registry-schema.md`](./registry-schema.md)

Cette demande concerne l'entreprise Torii, les services gRPC, DTO associés et
Création d'objets d'art, non disponibles pour les ordinateurs d'enregistrement Sora Name Service
(SNS). Ce contrat d'autorité pour le SDK, les contrats et l'automatisation, sont
Vous ne pouvez pas vous inscrire, diffuser ou configurer des éléments SNS.

## 1. Transport et authentification| Trebovanie | Détails |
|------------|--------|
| Protocoles | REST pod `/v1/sns/*` et service gRPC `sns.v1.Registrar`. Utilisez Norito-JSON (`application/json`) et Norito-RPC (`application/x-norito`). |
| Authentification | `Authorization: Bearer` sont des certificats ou des certificats mTLS, vous pouvez utiliser le suffix steward. Les tâches de mise en œuvre des postes de travail (gel/dégel, affectations réservées) concernent `scope=sns.admin`. |
| Organisation | Le registraire fournit les buckets `torii.preauth_scheme_limits` avec JSON et les limites d'utilisation du suffixe : `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Télémétrie | Torii publie `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` pour le bureau d'enregistrement (filtre selon `scheme="norito_rpc"`) ; L'API utilise `sns_registrar_status_total{result, suffix_id}`. |

## 2. Обзор DTO

Pour déterminer la structure canonique, utilisez [`registry-schema.md`](./registry-schema.md). Les charges utiles incluent `NameSelectorV1` + `SuffixId`, ce qui permet une navigation non disponible.

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

## 3. REST эндпоинты| Réponse | Méthode | Charge utile | Description |
|--------------|-------|---------|--------------|
| `/v1/sns/names` | POSTER | `RegisterNameRequestV1` | Enregistrez-vous ou ouvrez-moi. Разрешает ценовой tier, проверяет доказательства платежа/управления, испускает события реестра. |
| `/v1/sns/names/{namespace}/{literal}/renew` | POSTER | `RenewNameRequestV1` | Продлевает срок. La grâce/rédemption occupe une place importante dans la politique. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POSTER | `TransferNameRequestV1` | Avant de procéder à la mise en service du système, veuillez procéder à l'installation. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | METTRE | `UpdateControllersRequestV1` | Заменяет набор contrôleurs; проверяет подписанные адреса аккаунтов. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POSTER | `FreezeNameRequestV1` | Geler le gardien/conseil. Требует tuteur ticket и ссылку на gouvernance dossier. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | SUPPRIMER | `GovernanceHookV1` | Dégeler après l'utilisation ; убеждается, что le conseil outrepasse зафиксирован. |
| `/v1/sns/reserved/{selector}` | POSTER | `ReservedAssignmentRequestV1` | Назначение réservé имен intendant/conseil. |
| `/v1/sns/policies/{suffix_id}` | OBTENIR | -- | Получает текущую `SuffixPolicyV1` (кэшируемо). |
| `/v1/sns/names/{namespace}/{literal}` | OBTENIR | -- | Возвращает текущий `NameRecordV1` + эффективное состояние (Active, Grace, и т. д.). |

**Sélecteur de sélection :** le segment `{selector}` est composé d'i105, compressé (`sora`) ou hexadécimal canonique par ADDR-5 ; Torii est normalisé par rapport à `NameSelectorV1`.**Modèle d'ordinateur :** nos entreprises utilisent Norito JSON avec `code`, `message`, `details`. Les codes incluent `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 CLI помощники (требование ручного регистратора N0)

Steward doit utiliser le registraire à partir de la CLI sans utiliser de code JSON :

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

- `--owner` pour utiliser la CLI de configuration de compte ; Sélectionnez `--controller` pour ajouter des comptes de contrôleur supplémentaires (en utilisant `[owner]`).
- La plaque de signalisation en ligne est compatible avec `PaymentProofV1` ; Avant d'utiliser `--payment-json PATH`, sinon vous avez une structure structurelle. Les métadonnées (`--metadata-json`) et les hooks de gouvernance (`--governance-json`) sont à votre disposition.

Lecture seule помощники дополняют репетиции:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

См. `crates/iroha_cli/src/commands/sns.rs` pour la réalisation ; Les commandes qui utilisent généralement Norito DTO dans ce document, puis la CLI sont compatibles avec les réponses Torii du bateau.

Les personnes supplémentaires offrant des services, des parents et des tuteurs :

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
  --new-owner <i105-account-id> \
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

`--governance-json` doit être corrigé en indiquant `GovernanceHookV1` (identifiant de proposition, hachages de vote, administrateur/tuteur). Cette commande va bientôt ouvrir le connecteur `/v1/sns/names/{namespace}/{literal}/...` que les opérateurs peuvent répéter à leur guise. Torii, vous pouvez utiliser le SDK.

## 4. Service gRPC```text
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

Format de fil : les schémas Norito pour la compilation d'étapes
`fixtures/norito_rpc/schema_hashes.json` (pièces `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, et. д.).

## 5. Хуки управления и доказательства

Si vous avez besoin d'un document approprié pour votre travail :

| Réception | Требуемые данные управления |
|--------------|-----------------------------|
| Enregistrement/Produits standard | Доказательство платежа со ссылкой на règlement инструкцию; голосование conseil не нужно, если tier не требует одобрения steward. |
| Inscription niveau premium / mission réservée | `GovernanceHookV1` correspond à l'identifiant de la proposition + accusé de réception du steward. |
| Précédente | Хеш голосования conseil + хеш сигнала DAO; autorisation du tuteur, когда передача инициирована разрешением спора. |
| Geler/Dégeler | Подпись ticket de gardien плюс dérogation du conseil (dégel). |

Torii a vérifié le document, a vérifié :

1. L'identifiant de la proposition correspond à la configuration du grand livre (`/v1/governance/proposals/{id}`) et au statut `Approved`.
2. Il s'agit d'un produit compatible avec les objets d'art en question.
3. Le steward/tuteur demande une clé publique à partir du numéro `SuffixPolicyV1`.

Les tests récents concernent `sns_err_governance_missing`.

## 6. Exemples de processus

### 6.1 Enregistrement standard1. Le client choisit `/v1/sns/policies/{suffix_id}` pour pouvoir accéder aux niveaux de scène, de grâce et de livraison.
2. Adresse client `RegisterNameRequestV1` :
   - `selector` peut être utilisé avant l'étiquette i105 ou avant l'étiquette compressée (`sora`).
   - `term_years` dans les politiques antérieures.
   - `payment` ссылается на перевод splitter trésor/intendant.
3. Torii prouve :
   - Нормализацию метки + список réservé.
   - Durée/prix brut vs `PriceTierV1`.
   - Сумма доказательства платежа >= рассчитанной цены + комиссии.
4. Dans le cas de Torii :
   - Сохраняет `NameRecordV1`.
   - Produit `RegistryEventV1::NameRegistered`.
   - Produit `RevenueAccrualEventV1`.
   - Возвращает новую запись + события.

### 6.2 Déroulement de la période de grâce

Les fonctionnalités de Grace incluent des fonctionnalités standard et une détection de zone :

- Torii remplace `now` par `grace_expires_at` et ajoute des tableaux de supplément à `SuffixPolicyV1`.
- Доказательство платежа должно покрывать supplément. Ошибка => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` indique le nouveau `expires_at`.

### 6.3 Gel du tuteur et dérogation du conseil

1. Guardian utilise le ticket `FreezeNameRequestV1`, en s'appuyant sur l'identité de l'incident.
2. Torii doit être installé dans `NameStatus::Frozen` pour obtenir `NameFrozen`.
3. Après le conseil d'administration, la dérogation est prise; L'opérateur utilise SUPPRIMER `/v1/sns/names/{namespace}/{literal}/freeze` avec `GovernanceHookV1`.
4. Torii vérifie la dérogation, en utilisant `NameUnfrozen`.## 7. Validation et codes d'accès

| Code | Description | HTTP |
|-----|----------|------|
| `sns_err_reserved` | Метка зарезервирована или заблокирована. | 409 |
| `sns_err_policy_violation` | Les contrôleurs de niveau ou de niveau sont très politiques. | 422 |
| `sns_err_payment_mismatch` | Assurez-vous de choisir ou d'activer la plaque de documentation. | 402 |
| `sns_err_governance_missing` | Отсутствуют/некорректны требуемые артефакты управления. | 403 |
| `sns_err_state_conflict` | L'utilisation n'est pas nécessaire dans le cadre de l'installation technique. | 409 |

Tous les codes sont testés à partir de `X-Iroha-Error-Code` et les conversions structurelles Norito JSON/NRPC.

## 8. Introduction à la réalisation

- Torii présente des enchères en attente dans `NameRecordV1.auction` et ouvre les enregistrements de pop-up, par `PendingAuction`.
- Доказательства платежа переиспользуют Norito reçus du grand livre ; Les services de trésorerie proposent l'API d'assistance (`/v1/finance/sns/payments`).
- Le SDK permet de trouver ces informations sur les types de fonctionnalités qui peuvent être utilisées pour les programmes de démarrage (`ERR_SNS_RESERVED`, et т. д.).

## 9. Les petits chats

- Connectez les appareils Torii au contrat réel après la vente aux enchères SN-3.
- Déployez le SDK-руководства (Rust/JS/Swift), en utilisant cette API.
- Connectez [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) aux crochets de gouvernance des documents.