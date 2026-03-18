---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-plan-registro
título: خطة تنفيذ Registro de PIN según SoraFS
sidebar_label: Registro de PIN
descripción: خطة تنفيذ SF-4 التي تغطي آلة الحالات للـ registro y واجهة Torii والتولنغ والرصد.
---

:::nota المصدر المعتمد
Utilice el código `docs/source/sorafs/pin_registry_plan.md`. حافظ على النسختين متزامنتين ما دامت الوثائق القديمة نشطة.
:::

# خطة تنفيذ Registro de PIN con SoraFS (SF-4)

يقدّم SF-4 عقد Registro de PIN y manifiesto de manifiesto
Utilice el pin, la API y el Torii y las aplicaciones y los datos.
يوسّع هذا المستند خطة التحقق بمهام تنفيذية ملموسة تغطي المنطق on-chain,
وخدمات المضيف، والـ accesorios, والمتطلبات التشغيلية.

## النطاق

1. **آلة حالات registro**: سجلات Norito للـ manifiestos y alias وسلاسل الخلفاء
   وعصور الاحتفاظ وبيانات الحوكمة الوصفية.
2. **تنفيذ العقد**: عمليات CRUD حتمية لدورة حياة pin (`ReplicationOrder`, `Precommit`,
   `Completion`, desalojo).
3. **واجهة الخدمة**: Utilice gRPC/REST para el registro de archivos Torii y SDK.
   وتشمل الترقيم والاتستاشن.
4. **التولنغ y accesorios**: مساعدات CLI ومتجهات اختبار ووثائق تحافظ على تزامن
   manifiestos وaliases وsobres الخاصة بالحوكمة.
5. **التليمتري وعمليات التشغيل**: Registro de aplicaciones y runbooks.

## نموذج البيانات

### السجلات الاساسية (Norito)| البنية | الوصف | الحقول |
|--------|-------|--------|
| `PinRecordV1` | مدخل manifiesto كنسي. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | ربط alias -> CID الخاص بالـ manifiesto. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | تعليمات للمزوّدين لتثبيت manifiesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | اقرار المزوّد. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | لقطة سياسة الحوكمة. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Nombre del producto: `crates/sorafs_manifest/src/pin_registry.rs` Norito en Rust
ومساعدات التحقق التي تدعم هذه السجلات. التحقق يعكس herramientas الخاص بالـ manifiesto
(búsqueda en registro fragmentador y activación de política de pines) en el enlace Torii y CLI
تقاسم نفس invariantes.

المهام:
- Utilice Norito y `crates/sorafs_manifest/src/pin_registry.rs`.
- توليد الشفرة (Rust + SDKs اخرى) باستخدام ماكرو Norito.
- تحديث الوثائق (`sorafs_architecture_rfc.md`) بعد تثبيت المخططات.

## تنفيذ العقد| المهمة | المالك/المالكون | الملاحظات |
|--------|------------------|-----------|
| Registro de registro (sled/sqlite/off-chain) y contrato inteligente. | Equipo central de infraestructura/contrato inteligente | توفير hash حتمي وتجنب الفاصلة العائمة. |
| Nombre del producto: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infraestructura básica | Utilice `ManifestValidator` para conectarlo. alias يمر الان عبر `RegisterPinManifest` (DTO من Torii) بينما يبقى `bind_alias` المخصص مخططا لتحديثات لاحقة. |
| انتقالات الحالة: فرض التعاقب (manifiesto A -> B), عصور الاحتفاظ, تفرد alias. | Consejo de Gobernanza / Core Infraestructura | Nombre de alias y nombre de usuario y nombre de usuario de `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ كشف التعاقب متعدد القفزات ودفاتر التكرار ما زالت مفتوحة. |
| Fuentes de configuración: Descargue `ManifestPolicyV1` desde config/حالة الحوكمة؛ السماح بالتحديث عبر احداث الحوكمة. | Consejo de Gobierno | Utilice CLI para acceder a la configuración. |
| Nombre del producto: Nombre del producto Norito (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observabilidad | تعريف مخطط الاحداث + التسجيل. |

الاختبارات:
- اختبارات وحدة لكل نقطة دخول (ايجابي + رفض).
- اختبارات خصائص لسلسلة التعاقب (بدون دورات، عصور متصاعدة).
- Fuzz للتحقق عبر توليد manifiesta عشوائية (مقيدة).

## واجهة الخدمة (تكامل Torii/SDK)| المكون | المهمة | المالك/المالكون |
|--------|--------|------------------|
| Módulo Torii | Aquí `/v1/sorafs/pin` (enviar), `/v1/sorafs/pin/{cid}` (búsqueda), `/v1/sorafs/aliases` (listar/enlazar), `/v1/sorafs/replication` (pedidos/recibos). توفير ترقيم + ترشيح. | Redes TL / Core Infraestructura |
| الاتستاشن | تضمين ارتفاع/هاش registro في الاستجابات؛ Utilice Norito para instalar SDK. | Infraestructura básica |
| CLI | Utilice `sorafs_manifest_stub` y CLI para seleccionar `sorafs_pin` o `pin submit`, `alias bind`, `order issue`, `registry export`. | Grupo de Trabajo sobre Herramientas |
| SDK | Enlaces de última generación (Rust/Go/TS) desde Norito؛ اضافة اختبارات تكامل. | Equipos SDK |

العمليات:
- اضافة طبقة cache/ETag لنقاط نهاية GET.
- توفير limitación de velocidad/autenticación بما يتوافق مع سياسات Torii.

## Calendario y CI- Accesorios adicionales: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` يخزن لقطات موقعة لـ manifest/alias/order يعاد توليدها عبر `cargo run -p iroha_core --example gen_pin_snapshot`.
- خطوة CI: `ci/check_sorafs_fixtures.sh` تعيد توليد اللقطة وتفشل عند وجود اختلافات، لتحافظ على تماهي accesorios الخاصة بـ CI.
- اختبارات التكامل (`crates/iroha_core/tests/pin_registry.rs`) تغطي المسار السعيد مع رفض alias المكرر، وحمايات اعتماد/احتفاظ alias, وhandles غير متطابقة لـ chunker, والتحقق من عدد النسخ، وفشل حمايات التعاقب (مؤشرات مجهولة/موافَق عليها مسبقا/مسحوبة/ذاتية الاشارة)؛ راجع حالات `register_manifest_rejects_*` لتفاصيل التغطية.
- اختبارات الوحدة تغطي الان التحقق من alias وحمايات الاحتفاظ وفحوصات الخلف في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ كشف التعاقب متعدد القفزات عند وصول آلة الحالات.
- JSON muestra el contenido del archivo JSON.

## التليمتري والرصد

المقاييس (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- تليمتري المزود الحالي (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) تبقى ضمن النطاق للوحـات de un extremo a otro.

Artículos:
- تيار احداث Norito منظم لتدقيقات الحوكمة (موقع؟).

Artículos:
- اوامر تكرار معلقة تتجاوز SLA.
- انتهاء صلاحية alias اقل من العتبة.
- مخالفات الاحتفاظ (manifiesto لم يجدد قبل الانتهاء).

Otros artículos:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` muestra manifiestos, alias, retrasos, SLA, latencia y holgura. ومعدلات الاوامر الفاشلة للمراجعة اثناء النوبة.

## Runbooks y otros- تحديث `docs/source/sorafs/migration_ledger.md` لتضمين تحديثات حالة registro.
- دليل المشغل: `docs/source/sorafs/runbooks/pin_registry_ops.md` (منشور حاليا) يغطي المقاييس والتنبيه والنشر والنسخ الاحتياطي واستعادة الخدمة.
- دليل الحوكمة: وصف معلمات السياسة وسير عمل الاعتماد ومعالجة النزاعات.
- صفحات مرجع API لكل نقطة نهاية (وثائق Docusaurus).

## الاعتماديات والتسلسل

1. اكمال مهام خطة التحقق (دمج ManifestValidator).
2. Pulse el botón Norito + قيم السياسة الافتراضية.
3. تنفيذ العقد + الخدمة وربط التليمتري.
4. اعادة توليد accesorios y تشغيل اختبارات التكامل.
5. تحديث الوثائق/runbooks y ووضع علامة اكتمال على عناصر خارطة الطريق.

يجب ان تشير كل قائمة تحقق ضمن SF-4 الى هذه الخطة عند تسجيل التقدم.
Y REST توفر الان نقاط نهاية قائمة مع اتستاشن:

- `GET /v1/sorafs/pin` y `GET /v1/sorafs/pin/{digest}` se manifiestan aquí
  ربط alias واوامر التكرار وكائن اتستاشن مشتق من هاش اخر كتلة.
- `GET /v1/sorafs/aliases` y `GET /v1/sorafs/replication` تكشفان كتالوج alias
  النشط وتراكم اوامر التكرار بترقيم ثابت ومرشحات حالة.

Utilice CLI para acceder a (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) حتى يتمكن المشغلون من اتمتة تدقيقات registro بدون لمس
واجهات API منخفضة المستوى.