---
lang: fr
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : torii-app-api-parité
titre : تدقيق تكافؤ واجهة تطبيق Torii
description : La version de TORII-APP-1 est une version SDK et une version plus récente.
---

الحالة: مكتمل 2026-03-21  
Nom : Plateforme Torii, responsable du programme SDK  
Lien vers la version : TORII-APP-1 — Mise à jour `app_api`

تعكس هذه الصفحة تدقيق `TORII-APP-1` الداخلي (`docs/source/torii/app_api_parity_audit.md`) حتى يتمكن القراء خارج المستودع الاحادي من معرفة اي اسطح `/v1/*` موصولة ومختبرة وموثقة. يتتبع التدقيق المسارات المعاد تصديرها عبر `Torii::add_app_api_routes` et `add_contracts_and_vk_routes` et `add_connect_routes`.

## النطاق والمنهج

يفحص التدقيق اعادة التصدير العامة في `crates/iroha_torii/src/lib.rs:256-522` وبناة المسارات المحمية بالميزات. ولكل سطح `/v1/*` في خارطة الطريق تحققنامن:

- تنفيذ المعالج وتعريفات DTO pour `crates/iroha_torii/src/routing.rs`.
- Utilisez les fichiers `app_api` et `connect`.
- اختبارات التكامل/الوحدة الموجودة والفريق المسؤول عن التغطية طويلة الاجل.

قوائم أصول/معاملات الحساب وقوائم حاملي الأصول معاملات استعلام `asset_id` اختيارية للتصفية المسبقة، بالإضافة إلى حدود الترقيم/الضغط العكسي الحالية.

## المصادقة والتوقيع القياسي- نقاط النهاية GET/POST الموجهة للتطبيقات تقبل رؤوس طلب قياسية اختيارية (`X-Iroha-Account`, `X-Iroha-Signature`) مبنية par `METHOD\n/path\nsorted_query\nsha256(body)`؛ Torii est un `QueryRequestWithAuthority` qui est l'exécuteur `/query`.
- Utiliser le SDK pour les applications :
  - JS/TS : `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` ou `canonicalRequest.js`.
  -Swift : `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  -Android (Kotlin/Java) : `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- امثلة:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "<i105-account-id>", method: "get", path: "/v1/accounts/<i105-account-id>/assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/<i105-account-id>/assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "<i105-account-id>",
                                                  method: "get",
                                                  path: "/v1/accounts/<i105-account-id>/assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("<i105-account-id>", "get", "/v1/accounts/<i105-account-id>/assets", "limit=5", ByteArray(0), signer)
```

## جرد نقاط النهاية

### اذونات الحساب (`/v1/accounts/{id}/permissions`) — مغطى
- Nom : `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO : `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Nom de la personne : `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Titres : `crates/iroha_torii/tests/accounts_endpoints.rs:126` et `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Nom : Plateforme Torii.
- Fichiers : Fichiers JSON Norito ou `items`/`total`, qui sont également disponibles. Fonctionnalités du SDK.

### تقييم OPRF للاسماء المستعارة (`POST /v1/aliases/voprf/evaluate`) — مغطى
- Nom : `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO : `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Nom de la personne : `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Éléments : éléments en ligne pour le SDK (`crates/iroha_torii/src/lib.rs:9945-9986`) pour le SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Nom : Plateforme Torii.
- ملاحظات: واجهة الاستجابة تفرض hex محدد وهوية backend؛ Utilisez le SDK pour DTO.### احداث preuve عبر SSE (`GET /v1/events/sse`) — مغطى
- Nom : `handle_v1_events_sse` ou numéro de téléphone (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO : `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) pour la preuve.
- Nom de la personne : `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Fonctions : SSE est résistant à l'épreuve (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) et fumée لSSE في خط الانابيب
  (`integration_tests/tests/events/sse_smoke.rs`).
- Fonctionnalité : Plateforme Torii (runtime) et GT de tests d'intégration (fixtures).
- ملاحظات: تم التحقق من مسارات فلتر preuve طرفا لطرف؛ والتوثيق موجود في `docs/source/zk_app_api.md`.

### دورة حياة العقود (`/v1/contracts/*`) — مغطى
- Titres : `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO : `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Nom de la personne : `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Fonctions : routeur/intégration `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Sujet : Smart Contract WG sur la plateforme Torii.
- ملاحظات: نقاط النهاية تضع المعاملات الموقعة في قائمة انتظار وتعيد استخدام مقاييس التليمترية المشتركة (`handle_transaction_with_metrics`).### دورة حياة مفاتيح التحقق (`/v1/zk/vk/*`) — مغطى
- Modèles : `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) et `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO : `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Nom de la personne : `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Titres : `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Nom : Groupe de travail ZK pour la plate-forme Torii.
- Éléments : Utiliser les DTO avec les SDK Norito Il s'agit de la limitation de débit `limits.rs`.

### Nexus Connect (`/v1/connect/*`) — Fonctionnalité (fonctionnalité `connect`)
- Modèles : `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO : `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Nom de la personne : `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Fonctions : `crates/iroha_torii/tests/connect_gating.rs` (fonctionnalité de déclenchement, prise en main WS) et
  تغطية مصفوفة ميزات الموجه (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Nom : Nexus Connect WG.
- ملاحظات : يتم تتبع مفاتيح rate limit عبر `limits::rate_limit_key`؛ وتغذي عدادات التليمترية مقاييس `connect.*`.### تليمترية مرحلات Kaigi — مغطى
- Modèles : `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO : `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Nom de la personne : `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Titres : `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- ملاحظات: يعيد بث SSE استخدام القناة العامة للبث مع فرض بوابة ملف تليمترية؛ وتوثق مخططات الاستجابة في `docs/source/torii/kaigi_telemetry_api.md`.

## ملخص تغطية الاختبارات

- اختبارات smoke للراوتر (`crates/iroha_torii/tests/router_feature_matrix.rs`) تضمن ان تركيبات الميزات تسجل كل مسار وان توليد OpenAPI يبقى متزامنا.
- تغطي الحزم الخاصة بنقاط النهاية استعلامات الحسابات ودورة حياة العقود ومفاتيح التحقق ZK وفلاتر preuve SSE Connectez-vous à Nexus.
- Utilisez le SDK (JavaScript, Swift, Python) pour Alias ​​VOPRF et SSE ولا يلزم عمل اضافي.

## الحفاظ على تحديث هذه المرآة

حدّث هذه الصفحة وتدقيق المصدر (`docs/source/torii/app_api_parity_audit.md`) عند تغير سلوك واجهة تطبيق Torii حتى يبقى Le SDK est également disponible pour vous.