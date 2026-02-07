---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan-registre-pin
title: خطة تنفيذ Pin Registry في SoraFS
sidebar_label : Dans le registre des broches
description: خطة تنفيذ SF-4 التي تغطي آلة الحالات للـ Registry et Torii والتولنغ والرصد.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/pin_registry_plan.md`. حافظ على النسختين متزامنتين ما دامت الوثائق القديمة نشطة.
:::

# خطة تنفيذ Pin Registry pour SoraFS (SF-4)

Le SF-4 est un registre de broches et un manifeste.
Il s'agit d'une broche et d'une API pour Torii et d'une API.
Il s'agit d'un lien vers une chaîne de production en chaîne.
وخدمات المضيف، والـ luminaires, والمتطلبات التشغيلية.

## النطاق

1. ** Registre des registres ** : Norito pour les manifestes et les alias et les noms de domaine
   وعصور الاحتفاظ وبيانات الحوكمة الوصفية.
2. **Caractéristiques** : Le CRUD est connecté à la broche (`ReplicationOrder`, `Precommit`,
   `Completion`, expulsion).
3. **Applications** : Utilisez gRPC/REST pour le registre et les SDK Torii.
   وتشمل الترقيم والاتستاشن.
4. ** Calendriers et calendriers ** : مساعدات CLI et اختبار ووثائق تحافظ على تزامن
   manifeste des alias et des enveloppes الخاصة بالحوكمة.
5. **التليمتري وعمليات التشغيل**: مقاييس وتنبيهات وrunbooks لصحة registre.

## نموذج البيانات

### السجلات الاساسية (Norito)

| البنية | الوصف | الحقول |
|--------|-------|--------|
| `PinRecordV1` | مدخل manifeste كنسي. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | ربط alias -> CID الخاص بالـ manifeste. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | تعليمات للمزوّدين لتثبيت manifeste. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | اقرار المزوّد. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | لقطة سياسة الحوكمة. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Lien vers `crates/sorafs_manifest/src/pin_registry.rs` pour Norito dans Rust
ومساعدات التحقق التي تدعم هذه السجلات. التحقق يعكس outillage الخاص بالـ manifeste
(recherche du registre des chunkers et du blocage des politiques de broches) pour la recherche Torii et CLI
Il s'agit d'invariants.

المهام:
- Remplacez les Norito par `crates/sorafs_manifest/src/pin_registry.rs`.
- Mise à jour (Rust + SDKs اخرى) par Norito.
- تحديث الوثائق (`sorafs_architecture_rfc.md`) بعد تثبيت المخططات.

## تنفيذ العقد| المهمة | المالك/المالكون | الملاحظات |
|--------|--------|---------------|
| Il s'agit d'un registre (sled/sqlite/off-chain) et d'un contrat intelligent. | Équipe Core Infra / Smart Contract | Le hachage est également une méthode de hachage. |
| Noms d'utilisateur : `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infrastructure de base | استخدام `ManifestValidator` من خطة التحقق. Utilisez l'alias `RegisterPinManifest` (DTO pour Torii) pour obtenir `bind_alias`. |
| انتقالات الحالة: فرض التعاقب (manifeste A -> B) et عصور الاحتفاظ، تفرد alias. | Conseil de gouvernance / Core Infra | تفرد alias وحدود الاحتفاظ وفحوصات اعتماد/سحب السلف موجودة في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ كشف التعاقب متعدد القفزات ودفاتر التكرار ما زالت مفتوحة. |
| Paramètres de configuration : Utiliser `ManifestPolicyV1` dans config/حالة الحوكمة؛ السماح بالتحديث عبر احداث الحوكمة. | Conseil de gouvernance | توفير CLI لتحديثات السياسة. |
| Utiliser le modèle : Norito pour (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observabilité | تعريف مخطط الاحداث + التسجيل. |

الاختبارات:
- اختبارات وحدة لكل نقطة دخول (ايجابي + رفض).
- اختبارات خصائص لسلسلة التعاقب (بدون دورات، عصور متصاعدة).
- Fuzz للتحقق عبر توليد manifeste عشوائية (مقيدة).

## واجهة الخدمة (تكامل Torii/SDK)

| المكون | المهمة | المالك/المالكون |
|--------|--------|--------|
| خدمة Torii | كشف `/v1/sorafs/pin` (soumettre) ، `/v1/sorafs/pin/{cid}` (recherche) ، `/v1/sorafs/aliases` (liste/liaison) ، `/v1/sorafs/replication` (commandes/reçus). توفير ترقيم + ترشيح. | Mise en réseau TL / Core Infra |
| الاتستاشن | تضمين ارتفاع/هاش registre في الاستجابات؛ Utilisez Norito pour les SDK. | Infrastructure de base |
| CLI | Utilisez `sorafs_manifest_stub` et CLI pour `sorafs_pin` avec `pin submit`, `alias bind`, `order issue`, `registry export`. | GT Outillage |
| SDK | Reliures disponibles (Rust/Go/TS) pour Norito؛ اضافة اختبارات تكامل. | Équipes SDK |

العمليات:
- اضافة طبقة cache/ETag pour GET.
- La limitation de débit / authentification est utilisée pour Torii.

## Calendriers et CI

- Luminaires : `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` يخزن لقطات موقعة لـ manifeste/alias/ordre يعاد توليدها عبر `cargo run -p iroha_core --example gen_pin_snapshot`.
- خطوة CI: `ci/check_sorafs_fixtures.sh` تعيد توليد اللقطة وتفشل عند وجود اختلافات، لتحافظ على تماهي luminaires الخاصة بـ CI.
- اختبارات التكامل (`crates/iroha_core/tests/pin_registry.rs`) تغطي المسار السعيد مع رفض alias المكرر، وحمايات اعتماد/احتفاظ alias, وhandles غير متطابقة لـ chunker, والتحقق من عدد النسخ، وفشل حمايات التعاقب (مؤشرات مجهولة/موافَق عليها مسبقا/مسحوبة/ذاتية الاشارة)؛ راجع حالات `register_manifest_rejects_*` لتفاصيل التغطية.
- اختبارات الوحدة تغطي الان التحقق من alias وحمايات الاحتفاظ وفحوصات الخلف في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ كشف التعاقب متعدد القفزات عند وصول آلة الحالات.
- JSON est utilisé pour la lecture en ligne et la lecture en ligne.

## التليمتري والرصد

Nom (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- تليمتري المزود الحالي (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) تبقى ضمن النطاق للوحـات de bout en bout.السجلات:
- تيار احداث Norito منظم لتدقيقات الحوكمة (موقع؟).

التنبيهات:
- اوامر تكرار معلقة تتجاوز SLA.
- انتهاء صلاحية alias اقل من العتبة.
- مخالفات الاحتفاظ (manifeste لم يجدد قبل الانتهاء).

لوحات المعلومات:
- Avec Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` pour les manifestes, les alias, le backlog, le SLA, la latence مقابل slack, ومعدلات الاوامر الفاشلة للمراجعة اثناء النوبة.

## Runbooks

- تحديث `docs/source/sorafs/migration_ledger.md` لتضمين تحديثات حالة registre.
- Numéro de téléphone : `docs/source/sorafs/runbooks/pin_registry_ops.md` (منشور حاليا) يغطي المقاييس والتنبيه والنشر والنسخ الاحتياطي واستعادة الخدمة.
- دليل الحوكمة: وصف معلمات السياسة وسير عمل الاعتماد ومعالجة النزاعات.
- L'API est utilisée pour la version ultérieure (et Docusaurus).

## الاعتماديات والتسلسل

1. Utilisez votre méthode de validation (ManifestValidator).
2. Utilisez Norito + قيم السياسة الافتراضية.
3. تنفيذ العقد + الخدمة وربط التليمتري.
4. اعادة توليد luminaires et اختبارات التكامل.
5. تحديث الوثائق/runbooks وووضع علامة اكتمال على عناصر خارطة الطريق.

Il s'agit d'un SF-4 qui est en train de fonctionner.
واجهة REST توفر الان نقاط نهاية قائمة مع اتستاشن:

- `GET /v1/sorafs/pin` et `GET /v1/sorafs/pin/{digest}` تعيدان manifestes مع
  ربط alias واوامر التكرار وكائن اتستاشن مشتق من هاش اخر كتلة.
- `GET /v1/sorafs/aliases` et `GET /v1/sorafs/replication` sont un alias
  النشط وتراكم اوامر التكرار بترقيم ثابت ومرشحات حالة.

Utilisez la CLI pour les fonctions (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) حتى يتمكن المشغلون من اتمتة تدقيقات registre بدون لمس
L'API est également disponible.