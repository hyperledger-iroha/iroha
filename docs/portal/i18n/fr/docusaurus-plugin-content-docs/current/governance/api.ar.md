---
lang: fr
direction: ltr
source: docs/portal/docs/governance/api.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

الحالة: مسودة/تصور لمرافقة مهام تنفيذ الحوكمة. قد تتغير الصيغ اثناء التنفيذ. الحتمية وسياسة RBAC قيود معيارية؛ يمكن لتوريي توقيع/ارسال المعاملات عندما يتم توفير `authority` و`private_key`, والا يبني العملاء المعاملة Il s'agit de `/transaction`.نظرة عامة
- Vous utilisez JSON. لمسارات انتاج المعاملات، تتضمن الردود `tx_instructions` - مصفوفة من تعليمة هيكلية واحدة او اكثر:
  - `wire_id` : معرّف السجل لنوع التعليمة
  - `payload_hex` : version Norito (hex)
- اذا تم توفير `authority` و`private_key` (او `private_key` pour les bulletins de vote des DTO) et Torii بالتوقيع والارسال ويعيد ايضا `tx_instructions`.
- Il s'agit d'une autorité SignedTransaction et d'une autorité chain_id pour le POST `/transaction`.
- Kit SDK :
- Python (`iroha_python`) : `ToriiClient.get_governance_proposal_typed` ou `GovernanceProposalResult` (état/type) et `ToriiClient.get_governance_referendum_typed` ou `GovernanceReferendumResult`. و`ToriiClient.get_governance_tally_typed` à `GovernanceTally`, و`ToriiClient.get_governance_locks_typed` à `GovernanceLocksResult`, و`ToriiClient.get_governance_unlock_stats_typed` à `GovernanceUnlockStats`, و`ToriiClient.list_governance_instances_typed` يعيد `GovernanceInstancesPage`, فارضا وصولا بنمط tapé عبر سطح الحوكمة مع امثلة استخدام في README.
- Fichier Python (`iroha_torii_client`) : `ToriiClient.finalize_referendum` et `ToriiClient.enact_proposal` et `GovernanceInstructionDraft` typé (voir `tx_instructions`). (Torii) ، لتجنب تحليل JSON اليدوي عند تركيب سكربتات Finalize/Enact.- JavaScript (`@iroha/iroha-js`) : `ToriiClient` pour les aides saisies pour le déverrouillage et les déverrouillages. `listGovernanceInstances(namespace, options)` est un conseil municipal (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) pour Node.js. Utilisez le VRF pour `/v1/gov/instances/{ns}` et le VRF pour les autres appareils.

نقاط النهاية

- POSTE `/v1/gov/proposals/deploy-contract`
  - Texte (JSON) :
    {
      "espace de noms": "applications",
      "contract_id": "mon.contrat.v1",
      "code_hash": "blake2b32 :..." | "...64hex",
      "abi_hash": "blake2b32:..." | "...64hex",
      "abi_version": "1",
      "fenêtre": { "inférieur": 12345, "supérieur": 12400 },
      "autorité": "i105…?",
      "private_key": "...?"
    }
  - الرد (JSON) :
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Fonction : Utiliser la fonction `abi_hash` et `abi_version` et la version `abi_hash`. Pour `abi_version = "v1"`, la version actuelle est `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.API العقود (déployer)
- POSTE `/v1/contracts/deploy`
  - Contenu : { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - Nom : `code_hash` pour IVM et `abi_hash` pour `abi_version`, pour `RegisterSmartContractCode` (manifeste) et `RegisterSmartContractBytes` (pour `.to`) ou `authority`.
  - الرد : { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - ذي صلة:
    - GET `/v1/contracts/code/{code_hash}` -> يعيد manifeste المخزن
    - OBTENIR `/v1/contracts/code-bytes/{code_hash}` -> ou `{ code_b64 }`
- POSTE `/v1/contracts/instance`
  - Contenu : { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Élément : ينشر البايتات المقدمة ويُفعل فورا ربط `(namespace, contract_id)` ou `ActivateContractInstance`.
  - Définition : { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }خدمة الاسماء المستعارة
- POSTE `/v1/aliases/voprf/evaluate`
  - Titre : { "blinded_element_hex": "..." }
  - Contenu : { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` يعكس تنفيذ المقيم. Nom de l'utilisateur : `blake2b512-mock`.
  - ملاحظات: مقيم mock حتمي يطبق Blake2b512 مع فصل مجال `iroha.alias.voprf.mock.v1`. Vous devez utiliser le VOPRF pour Iroha.
  - Type : HTTP `400` est utilisé en hexadécimal. Utilisez Torii pour Norito `ValidationFail::QueryFailed::Conversion` pour votre décodeur.
- POSTE `/v1/aliases/resolve`
  - Nom : { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Nom : { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Types : vous pouvez utiliser le pont ISO (`[iso_bridge.account_aliases]` et `iroha_config`). يقوم Torii بتطبيع الاسماء عبر ازالة الفراغات وتحويلها الى احرف كبيرة قبل البحث. يعيد 404 عندما يكون الاسم غير موجود و503 عندما يكون runtime الخاص بـ ISO bridge معطلا.
- POSTE `/v1/aliases/resolve_index`
  - Nom : { "index": 0 }
  - Contenu : { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - ملاحظات : مؤشرات الاسماء تعين بشكل حتمي حسب ترتيب التكوين (basé sur 0). يمكن للعملاء تخزين الردود hors ligne لبناء مسارات تدقيق لاحداث attestation الخاصة بالاسماء.حد حجم الكود
- Fichier de référence : `max_contract_code_bytes` (JSON u64)
  - يتحكم في الحد الاقصى المسموح (بالبايت) لتخزين كود العقود على السلسلة.
  - Capacité : 16 Mo. Le `RegisterSmartContractBytes` est un invariant.
  - يمكن للمشغلين التعديل عبر `SetParameter(Custom)` et `id = "max_contract_code_bytes"` وحمولة رقمية.

- POSTE `/v1/gov/ballots/zk`
  - Nom : { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - الرد : { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - ملاحظات :
    - عندما تتضمن المدخلات العامة للدائرة `owner` و`amount` و`duration_blocks`, وتتتحقق البرهان من VK Utilisez le verrou et le verrou pour `election_id` avec `owner`. تبقى الاتجاهات مخفية (`unknown`)؛ ويتم تحديث montant/expiration فقط. Les éléments monotones : montant et expiration sont tous deux (effets max(amount, prev.amount) et max(expiry, prev.expiry)).
    - اعادات التصويت ZK التي تحاول تقليل montant et expiration يتم رفضها من جهة الخادم مع تشخيصات `BallotRejected`.
    - يجب على تنفيذ العقد استدعاء `ZK_VOTE_VERIFY_BALLOT` pour `SubmitBallot` ; ويفرض المضيفون loquet لمرة واحدة.- POSTE `/v1/gov/ballots/plain`
  - الطلب : { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Oui|Non|S'abstenir" }
  - الرد : { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - ملاحظات: اعادات التصويت تمتد فقط - لا يمكن لvote جديد تقليل montant et او expiration لقفل موجود. يجب ان يساوي `owner` سلطة المعاملة. Il s'agit d'un `conviction_step_blocks`.- POSTE `/v1/gov/finalize`
  - Nom : { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - Définition : { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - الاثر على السلسلة (الهيكل الحالي): تنفيذ اقتراح déployer معتمد يدرج `ContractManifest` ادنى بمفتاح `code_hash` مع `abi_hash` المتوقع ويضع الاقتراح بحالة Adoptée. Il s'agit du manifeste correspondant à `code_hash` et à `abi_hash`.
  - ملاحظات :
    - لانتخابات ZK, يجب على مسارات العقد استدعاء `ZK_VOTE_VERIFY_TALLY` قبل تنفيذ `FinalizeElection` ; ويفرض المضيفون loquet لمرة واحدة. يرفض `FinalizeReferendum` الاستفتاءات ZK حتى يتم انهاء tally للانتخابات.
    - الاغلاق التلقائي عند `h_end` يصدر Approuvé/Rejeté فقط للاستفتاءات Plain؛ تبقى استفتاءات ZK Closed حتى يتم ارسال tally منته ويجري تنفيذ `FinalizeReferendum`.
    - فحوصات participation تستخدم approuver+rejeter فقط؛ s'abstenir en cas de participation.- POSTE `/v1/gov/enact`
  - Nom : { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - Définition : { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - ملاحظات: يقدم Torii المعاملة الموقعة عندما توفر `authority`/`private_key` ; والا يعيد هيكلا لتوقيع العملاء وارساله. الـ preimage اختيارية وحاليا معلوماتية.

- OBTENIR `/v1/gov/proposals/{id}`
  - Nom `{id}` : Nom hexadécimal (64 caractères)
  - الرد : { "found": bool, "proposal": { ... } ? }

- OBTENIR `/v1/gov/locks/{rid}`
  - Nom `{rid}` : chaîne de caractères
  - Définition : { "found": bool, "referendum_id": "rid", "locks": { ... } ? }

- OBTENIR `/v1/gov/council/current`
  - الرد : { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - ملاحظات: يعيد Council المحفوظ اذا كان موجودا؛ والا يشتق بديلا حتميا باستخدام اصل participation المضبوط والعتبات (يعكس مواصفات VRF حتى تثبت ادلة VRF الحية على السلسلة).- POST `/v1/gov/council/derive-vrf` (fonctionnalité : gov_vrf)
  - Nom : { "committee_size": 21, "epoch": 123 ? , "candidats": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Fonction : Vous pouvez utiliser VRF pour créer un système de hachage `chain_id` et `epoch` et hash للبلوك؛ يرتب حسب bytes الخرج desc مع كاسرات تعادل؛ ويعيد اعلى `committee_size` من الاعضاء. لا يتم الحفظ.
  - الرد : { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "vérifié": K }
  - Valeurs : Normal = pk pour G1, preuve pour G2 (96 octets). Petit = pk pour G2, preuve pour G1 (48 octets). Les informations fournies par `chain_id`.

### Paramètres de connexion (iroha_config `gov.*`)

يتم ضبط Council الاحتياطي الذي يستخدمه Torii عندما لا يوجد roster محفوظ عبر `iroha_config` :

```toml
[gov]
  vk_ballot.backend = "halo2/ipa"
  vk_ballot.name    = "ballot_v1"
  vk_tally.backend  = "halo2/ipa"
  vk_tally.name     = "tally_v1"
  plain_voting_enabled = false
  conviction_step_blocks = 100
  max_conviction = 6
  approval_q_num = 1
  approval_q_den = 2
  min_turnout = 0
  parliament_committee_size = 21
  parliament_term_blocks = 43200
  parliament_min_stake = 1
  parliament_eligibility_asset_id = "SORA#stake"
```

بدائل البيئة المكافئة:

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_PARLIAMENT_COMMITTEE_SIZE=21
GOV_PARLIAMENT_TERM_BLOCKS=43200
GOV_PARLIAMENT_MIN_STAKE=1
GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID=SORA#stake
GOV_ALIAS_TEU_MINIMUM=0
GOV_ALIAS_FRONTIER_TELEMETRY=true
```

`parliament_committee_size` يحد عدد اعضاء fallback المعادين عندما لا يوجد Council محفوظ، و`parliament_term_blocks` يحدد طول الحقبة المستخدمة لاشتقاق seed (`epoch = floor(height / term_blocks)`) et `parliament_min_stake` pour la mise en jeu (بوحدات صغرى) et `parliament_eligibility_asset_id` يحدد اي رصيد اصل يتم مسحه عند بناء مجموعة المرشحين.

VK للحوكمة بلا bypass: تحقق من bulletins de vote يتطلب دائما مفتاح تحقق `Active` by inline, ولا يجب ان تعتمد البيئات على تبديلات اختبارية لتخطي التحقق.RBAC
- التنفيذ على السلسلة يتطلب صلاحيات:
  - Propositions : `CanProposeContractDeployment{ contract_id }`
  - Bulletins de vote : `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgation : `CanEnactGovernance`
  - Gestion communale (مستقبلا) : `CanManageParliament`

Espaces de noms محمية
- Le module `gov_protected_namespaces` (tableau JSON avec chaînes) permet de déclencher le déploiement d'espaces de noms.
- Vous pouvez utiliser les métadonnées pour déployer les espaces de noms :
  - `gov_namespace` : espace de noms (pour "apps")
  - `gov_contract_id` : espace de noms utilisé
- `gov_manifest_approvers` : tableau JSON contenant les identifiants de compte. عندما يعلن manifeste لمسار ما quorum اكبر من واحد، يتطلب admission سلطة المعاملة بالاضافة الى الحسابات المدرجة لتلبية quorum الخاص بالmanifeste.
- تكشف التليمترية عدادات admission عبر `governance_manifest_admission_total{result}` لتمييز القبولات الناجحة عن مسارات `missing_manifest` و`non_validator_authority` و`quorum_rejected` و`protected_namespace_rejected` و`runtime_hook_rejected`.
- حتى يتمكن المشغلون من تدقيق الموافقات المفقودة.
- Liste autorisée des espaces de noms pour les manifestes. L'espace de noms `gov_namespace` est également appelé `gov_contract_id` et l'espace de noms est `protected_namespaces` pour le manifeste. Vous pouvez utiliser `RegisterSmartContractCode` pour les métadonnées de votre ordinateur.
- يفرض admission وجود اقتراح حوكمة Promulgué للتركيبة `(namespace, contract_id, code_hash, abi_hash)`؛ Il s'agit également de NotPerowed.Runtime de Hooks
- يمكن لـ manifeste le système d'exécution du runtime `hooks.runtime_upgrade` (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- حقول crochet :
  - `allow` (bool, version `true`) : le moteur d'exécution `false` est utilisé pour le runtime.
  - `require_metadata` (bool, الافتراضي `false`) : les métadonnées sont utilisées par `metadata_key`.
  - `metadata_key` (string) : les métadonnées utilisées sont le hook. La version `gov_upgrade_id` est une métadonnée ajoutée à la liste d'autorisation.
  - `allowed_ids` (tableau et chaînes) : liste autorisée pour les métadonnées (par trim). يرفض عندما لا تكون القيمة المقدمة مدرجة.
- عندما يكون hook موجودا، يفرض admission في الطابور سياسة metadata قبل دخول المعاملة للطابور. métadonnées liées à la liste d'autorisation et à la liste d'autorisation NotPerowed.
- تتابع التليمترية النتائج عبر `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Les métadonnées `gov_upgrade_id=<value>` (pour le manifeste) sont également associées au crochet. بواسطة quorum الخاص بالmanifest.

Point de terminaison
- POST `/v1/gov/protected-namespaces` - يطبق `gov_protected_namespaces` مباشرة على العقدة.
  - الطلب : { "namespaces": ["apps", "system"] }
  - الرد : { "ok": vrai, "appliqué": 1 }
  - ملاحظات : مخصص للادارة/الاختبار؛ Le jeton API est ici. للانتاج، يفضل ارسال معاملة موقعة مع `SetParameter(Custom)`.مساعدات CLI
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - يجلب مثيلات العقود للnamespace ويتحقق من :
    - Torii est un bytecode pour `code_hash`, et digest Blake2b-32 est `code_hash`.
    - Le manifeste est `/v1/contracts/code/{code_hash}` et `code_hash` et `abi_hash`.
    - يوجد اقتراح حوكمة a été promulguée par `(namespace, contract_id, code_hash, abi_hash)` pour le hachage de l'ID de proposition dans le cadre de la procédure de hachage.
  - JSON utilise `results[]` pour les problèmes (problèmes, manifeste/code/proposition) pour votre projet. تعطيله (`--no-summary`).
  - Vous pouvez utiliser des espaces de noms pour déployer des espaces de noms.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - JSON utilise les métadonnées pour déployer des espaces de noms pour le quorum `gov_manifest_approvers` للmanifeste.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — تلميحات القفل مطلوبة عندما يكون `min_bond_amount > 0`, وأي مجموعة مقدمة يجب أن تضمن `owner` et `amount` et `duration_blocks`.
  - Valide les identifiants de compte canoniques, canonise les indices d'annulation de 32 octets et fusionne les indices dans `public_inputs_json` (avec `--public <path>` pour des remplacements supplémentaires).
  - L'annulateur est dérivé de l'engagement de preuve (entrée publique) plus `domain_tag`, `chain_id` et `election_id` ; `--nullifier` est validé par rapport à l'épreuve à la livraison.- الملخص في سطر واحد يعرض الان `fingerprint=<hex>` حتميا مشتقا من `CastZkBallot` المشفر مع astuces المفككة (`owner`, `amount`, `duration_blocks`, `direction` ici).
  - ردود CLI تضع تعليقات على `tx_instructions[]` مع `payload_fingerprint_hex` بالاضافة الى حقول مفكوكة كي تتحقق الادوات Il s'agit d'un produit que vous pouvez utiliser comme Norito.
  - Astuces pour le verrouillage des bulletins de vote ZK pour les bulletins de vote `LockCreated`/`LockExtended` نفسها.
-`iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Les alias `--lock-amount`/`--lock-duration-blocks` sont des indicateurs de sécurité pour ZK.
  - ناتج الملخص يعكس `vote --mode zk` pour les empreintes digitales et les bulletins de vote (`owner`, `amount`, `duration_blocks`, `direction`) ، لتاكيد سريع قبل توقيع الهيكل.قائمة المثيلات
- GET `/v1/gov/instances/{ns}` - يسرد مثيلات العقود النشطة لnamespace.
  - Paramètres de requête :
    - `contains` : remplacer la sous-chaîne par `contract_id` (sensible à la casse)
    - `hash_prefix` : remplacer par hexadécimal par `code_hash_hex` (minuscule)
    - `offset` (0 par défaut), `limit` (100 par défaut, 10_000 maximum)
    - `order` : correspond à `cid_asc` (par défaut), `cid_desc`, `hash_asc`, `hash_desc`.
  - Contenu : { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK d'assistance : `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) et `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

مسح déverrouille (المشغل/التدقيق)
- OBTENIR `/v1/gov/unlocks/stats`
  - Couleur : { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - ملاحظات: `last_sweep_height` يعكس اخر ارتفاع بلوك تم فيه مسح verrous منتهية وتخزينها. `expired_locks_now` est utilisé pour verrouiller le verrou `expiry_height <= height_current`.
- POSTE `/v1/gov/ballots/zk-v1`
  - الطلب (DTO version v1) :
    {
      "autorité": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "enveloppe_b64": "AAECAwQ=",
      "root_hint": "0x...64hex ?",
      "propriétaire": "i105…?",
      "nullifier": "blake2b32:...64hex?"
    }
  - الرد : { "ok": true, "accepted": true, "tx_instructions": [{...}] }- POST `/v1/gov/ballots/zk-v1/ballot-proof` (fonctionnalité : `zk-ballot`)
  - JSON `BallotProof` est remplacé par `CastZkBallot`.
  - الطلب:
    {
      "autorité": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "vote": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 pour ZK1 et H2*
        "root_hint": null, // chaîne hexadécimale facultative de 32 octets (racine d'éligibilité)
        "owner": null, // AccountId اختياري عندما تلتزم الدائرة par propriétaire
        "nullifier": null // chaîne hexadécimale facultative de 32 octets (indice d'annulation)
      }
    }
  - الرد :
    {
      "ok" : vrai,
      "accepté": vrai,
      "reason": "construire le squelette de la transaction",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - ملاحظات :
    - يقوم الخادم بربط `root_hint`/`owner`/`nullifier` الاختيارية من bulletin de vote pour `public_inputs_json` pour `CastZkBallot`.
    - Il y a beaucoup d'octets dans l'enveloppe en base64 pour la charge utile.
    - تتغير `reason` à `submitted transaction` pour le bulletin de vote Torii.
    - Le point de terminaison est associé à la fonctionnalité `zk-ballot`.مسار التحقق من CastZkBallot
- `CastZkBallot` est compatible avec la base64 et les paramètres de base64 (`BallotRejected` ou `invalid or empty proof`).
- المضيف يحل مفتاح التحقق للvote et référendum (`vk_ballot`) et افتراضات الحوكمة ويتطلب ان يكون السجل موجودا و`Active` et octets en ligne.
- octets de hachage pour `hash_vk` ; اي عدم تطابق للالتزام يوقف التنفيذ قبل التحقق للحماية من ادخالات سجل معبث بها (`BallotRejected` مع `verifying key commitment mismatch`).
- bytes الخاصة بالبرهان ترسل الى backend المسجل عبر `zk::verify_backend` ; غير الصالحة كـ `BallotRejected` مع `invalid proof` وتفشل التعليمة بشكل حتمي.
- La preuve doit exposer un engagement de vote et une racine d'éligibilité en tant qu'apports publics ; la racine doit correspondre au `eligible_root` de l’élection et l’annulateur dérivé doit correspondre à tout indice fourni.
- البراهين الناجحة تصدر `BallotAccepted` ; nullifiers المكررة او جذور اهلية قديمة او تراجعات lock تستمر في انتاج اسباب الرفض المذكورة سابقا في هذا المستند.

## سوء سلوك المدققين والتوافق المشترك

### سير عمل coupant et emprisonnantيصدر الاجماع `Evidence` مرمزا بـ Norito عندما ينتهك مدقق البروتوكول. تصل كل حمولة الى `EvidenceStore` في الذاكرة, وان لم تكن معروفة يتم تجسيدها في خريطة `consensus_evidence` المدعومة بـ WSV. يتم رفض السجلات الاقدم من `sumeragi.npos.reconfig.evidence_horizon_blocks` (الافتراضي `7200` بلوك) كي يبقى الارشيف محدودا، لكن الرفض يسجل للمشغلين. Les preuves dans l'horizon respectent également `sumeragi.npos.reconfig.activation_lag_blocks` (par défaut `1`) et le délai de barre oblique `sumeragi.npos.reconfig.slashing_delay_blocks` (par défaut `259200`) ; la gouvernance peut annuler les pénalités avec `CancelConsensusEvidencePenalty` avant que la réduction ne s'applique.

الانتهاكات المعترف بها تقابل واحدا لواحد مع `EvidenceKind` ; المميزات ثابتة ويفرضها نموذج البيانات:

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidQc,
    EvidenceKind::InvalidProposal,
    EvidenceKind::Censorship,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

- **DoublePrepare/DoubleCommit** - Les hachages sont également disponibles pour le tuple `(phase,height,view,epoch)`.
- **InvalidQc** - Il s'agit d'un certificat de validation qui n'est pas compatible avec le bitmap.
- **InvalidProposal** - Il s'agit d'une chaîne verrouillée.
- **Censure** : les reçus de soumission signés montrent une transaction qui n'a jamais été proposée/engagée.

يمكن للمشغلين والادوات فحص واعادة بث الحمولات عبر:

- Torii : `GET /v1/sumeragi/evidence` et `GET /v1/sumeragi/evidence/count`.
- CLI : `iroha ops sumeragi evidence list`, `... count`, و`... submit --evidence-hex <payload>`.

يجب على الحوكمة اعتبار bytes الخاصة بالevidence دليلا قانونيا:1. **جمع الحمولة** قبل ان تتقادم. Les octets Norito contiennent des métadonnées pour la hauteur/la vue.
2. **تجهيز العقوبة** عبر تضمين الحمولة في referendum او تعليمة sudo (مثل `Unregister::peer`). تعيد عملية التنفيذ التحقق من الحمولة؛ preuves المشوهة او القديمة ترفض حتميا.
3. **جدولة طوبولوجيا المتابعة** حتى لا يتمكن المدقق المخالف من العودة فورا. Les utilisateurs `SetParameter(Sumeragi::NextMode)` et `SetParameter(Sumeragi::ModeActivationHeight)` sont inscrits sur la liste.
4. **تدقيق النتائج** عبر `/v1/sumeragi/evidence` و`/v1/sumeragi/status` لضمان ان عداد تقدم وان الحوكمة طبقت الازالة.

### تسلسل الاجماع المشترك

يضمن الاجماع المشترك ان تقوم مجموعة المدققين الخارجة بانهاء بلوك الحد قبل ان تبدا المجموعة الجديدة بالاقتراح. Le runtime est utilisé pour les tâches suivantes :

- يجب ان يتم الالتزام بـ `SumeragiParameter::NextMode` و`SumeragiParameter::ModeActivationHeight` في **نفس البلوك**. يجب ان تكون `mode_activation_height` اكبر تماما من ارتفاع البلوك الذي حمل التحديث، بما يوفر على الاقل بلوكا واحدا من lag.
- `sumeragi.npos.reconfig.activation_lag_blocks` (`1`) pour les transferts de garde et le décalage :
- `sumeragi.npos.reconfig.slashing_delay_blocks` (`259200` par défaut) retarde la réduction du consensus afin que la gouvernance puisse annuler les pénalités avant qu'elles ne s'appliquent.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```- Le runtime et les éléments CLI mis en scène sont `/v1/sumeragi/params` et `iroha --output-format text ops sumeragi params` pour mettre en place un processus de mise en scène. وقوائم المدققين.
- يجب ان تقوم اتمتة الحوكمة دائما بما يلي:
  1. انهاء قرار الازالة (او الاستعادة) المدعوم بـ preuve.
  2. Utilisez le bouton `mode_activation_height = h_current + activation_lag_blocks`.
  3. مراقبة `/v1/sumeragi/status` حتى يتبدل `effective_consensus_mode` عند الارتفاع المتوقع.

Il s'agit d'un problème de décalage et de réduction du décalage, ainsi que du transfert. يتم رفض تلك المعاملات وتترك الشبكة على الوضع السابق.

## اسطح التليمترية

- La clé Prometheus est disponible :
  - `governance_proposals_status{status}` (jauge) يتتبع تعداد المقترحات حسب الحالة.
  - `governance_protected_namespace_total{outcome}` (compteur) est disponible pour l'admission aux espaces de noms.
  - `governance_manifest_activations_total{event}` (compteur) contient le manifeste (`event="manifest_inserted"`) et l'espace de noms (`event="instance_bound"`).
- `/status` pour les espaces de noms `governance` pour les espaces de noms Il s'agit du manifeste (espace de noms, identifiant du contrat, code/hachage ABI, hauteur du bloc, horodatage d'activation). Il s'agit de textes législatifs, de manifestes et d'espaces de noms.
- Pour Grafana (`docs/source/grafana_governance_constraints.json`) et Runbook pour `telemetry.md`, vous avez besoin de plus d'informations. Les espaces de noms manifestes et les espaces de noms sont associés à l'environnement d'exécution.