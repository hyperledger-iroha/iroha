---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mercado-capacidad-de-almacenamiento
título: سوق سعة التخزين في SoraFS
sidebar_label: سوق السعة
descripción: خطة SF-2c لسوق السعة، أوامر النسخ المتماثل، القياس عن بعد، وخطافات الحوكمة.
---

:::nota المصدر المعتمد
Aquí está el mensaje `docs/source/sorafs/storage_capacity_marketplace.md`. حافظوا على تزامن النسختين ما دامت الوثائق القديمة نشطة.
:::

# سوق سعة التخزين في SoraFS (módulo SF-2c)

يقدّم بند خارطة الطريق SF-2c سوقا محكوما حيث يعلن مزودو التخزين عن سعة ملتزم بها، ويتلقون أوامر النسخ المتماثل، ويكسبون رسوما تتناسب مع التوافر المُسلَّم. يحدد هذا المستند نطاق المخرجات المطلوبة للإصدار الأول ويقسمها إلى مسارات قابلة للتنفيذ.

## الأهداف

- التعبير عن التزامات سعة المزودين (إجمالي البايتات، حدود لكل lane, تاريخ الانتهاء) بصيغة قابلة للتحقق يمكن Utilice el software SoraNet y Torii.
- توزيع pines عبر المزودين وفق السعة المعلنة و estaca وقيود السياسة مع الحفاظ على سلوك حتمي.
- قياس تسليم التخزين (نجاح النسخ المتماثل، uptime, أدلة السلامة) y تصدير التليمترية لتوزيع الرسوم.
- توفير عمليات الإلغاء والنزاع حتى يمكن معاقبة المزودين غير الأمناء أو إزالتهم.

## مفاهيم النطاق| المفهوم | الوصف | المخرج الأولي |
|---------|-------------|---------------------|
| `CapacityDeclarationV1` | El dispositivo Norito está conectado a un fragmentador GiB, conectado a un carril, desconectado del carril. apostar, وتاريخ الانتهاء. | المخطط + المدقق في `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Para obtener información sobre el manifiesto CID y el SLA, consulte el formulario de solicitud del CID. | Utilice Norito para obtener un contrato inteligente Torii +. |
| `CapacityLedger` | سجل on-chain/off-chain يتتبع إعلانات السعة النشطة، أوامر النسخ المتماثل، مقاييس الأداء، وتراكم الرسوم. | Hay contratos inteligentes, trozos fuera de la cadena y instantáneas. |
| `MarketplacePolicy` | سياسة حوكمة تحدد الحد الأدنى لـ estaca ومتطلبات التدقيق ومنحنيات العقوبة. | config struct es `sorafs_manifest` + وثيقة حوكمة. |

### المخططات المنفذة (الحالة)

## تفصيل العمل

### 1. طبقة المخططات والسجل| المهمة | Propietario(s) | ملاحظات |
|------|----------|-------|
| Utilice `CapacityDeclarationV1`, `ReplicationOrderV1` y `CapacityTelemetryV1`. | Equipo de Almacenamiento / Gobernanza | Fuente Norito؛ وأدرج النسخ الدلالية ومراجع القدرات. |
| Aquí hay un analizador + validador como `sorafs_manifest`. | Equipo de almacenamiento | فرض IDs أحادية، حدود السعة، ومتطلبات participación. |
| Para crear un fragmentador de metadatos, utilice `min_capacity_gib`. | Grupo de Trabajo sobre Herramientas | يساعد العملاء على فرض الحد الأدنى من متطلبات العتاد لكل ملف تعريف. |
| صياغة وثيقة `MarketplacePolicy` التي تلتقط ضوابط القبول وجدول العقوبات. | Consejo de Gobierno | انشرها في docs بجانب valores predeterminados de la política. |

#### تعريفات المخطط (منفذة)- يلتقط `CapacityDeclarationV1` التزامات سعة موقعة لكل مزود، بما في ذلك maneja el fragmentador القياسية, مراجع capacidades, حدود اختيارية لكل lane, تلميحات التسعير, نوافذ الصلاحية, وmetadatos. تضمن عملية التحقق participación غير صفري، maneja قياسية، alias مزالة التكرار، حدود carril ضمن الإجمالي المعلن، ومحاسبة GiB أحادية.【crates/sorafs_manifest/src/capacity.rs:28】
- يربط `ReplicationOrderV1` manifiesta بتعيينات صادرة عن الحوكمة مع أهداف التكرار، عتبات SLA, وضمانات لكل task؛ Los validadores de تفرض manejan el fragmentador القياسية، مزودين فريدين، وقيود قبل أن يقوم Torii أو السجل بابتلاع الأمر.【crates/sorafs_manifest/src/capacity.rs:301】
- يعبر `CapacityTelemetryV1` عن instantáneas الحقبة (GiB المعلنة مقابل المستخدمة، عدادات النسخ، نسب uptime/PoR) التي تغذي توزيع الرسوم. La configuración de la configuración de la red es 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- Ayudantes de ayuda (`CapacityMetadataEntry` y `PricingScheduleV1` y carril/asignación/SLA) تحقق مفاتيح حتمي وتقارير أخطاء يمكن Para CI y herramientas posteriores están disponibles.【crates/sorafs_manifest/src/capacity.rs:230】
- يعرض `PinProviderRegistry` Instantánea على السلسلة عبر `/v2/sorafs/capacity/state`, جامعاً إعلانات المزودين y خلف Norito JSON حتمي.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】- تغطي اختبارات التحقق فرض maneja القياسية, كشف التكرار، حدود carril, حمايات تعيين النسخ المتماثل، وفحوص نطاق Las aplicaciones de CI.【crates/sorafs_manifest/src/capacity.rs:792】
- Contenido del paquete: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` Especificaciones del producto Compatible con cargas útiles Norito, blobs base64 y archivos JSON يتمكن المشغلون من تجهيز accesorios لـ `/v2/sorafs/capacity/declare` و`/v2/sorafs/capacity/telemetry` وأوامر النسخ المتماثل مع تحقق Más `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. تكامل طبقة التحكم| المهمة | Propietario(s) | ملاحظات |
|------|----------|-------|
| Utilice los archivos Torii, `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry` y `/v2/sorafs/capacity/orders` con los archivos Norito JSON. | Torii Equipo | محاكاة منطق التحقق؛ Utilice ayudantes JSON Norito. |
| Instale instantáneas en `CapacityDeclarationV1`, metadatos en el orquestador y puerta de enlace. | Equipo de trabajo de herramientas / orquestador | تمديد `provider_metadata` بمراجع السعة حتى يحترم puntuación متعدد المصادر حدود carril. |
| تغذية أوامر النسخ المتماثل إلى عملاء orquestador/gateway لقيادة asignaciones y conmutación por error. | Equipo de Networking TL / Gateway | يستهلك Generador de marcadores أوامر النسخ الموقعة من الحوكمة. |
| Utilice CLI: Seleccione `sorafs_cli` o `capacity declare`, `capacity telemetry` y `capacity orders import`. | Grupo de Trabajo sobre Herramientas | توفير JSON حتمي + مخرجات marcador. |

### 3. سياسة السوق والحوكمة| المهمة | Propietario(s) | ملاحظات |
|------|----------|-------|
| إقرار `MarketplacePolicy` (الحد الأدنى لـ estaca, مضاعفات العقوبة, وتواتر التدقيق). | Consejo de Gobierno | نشرها في docs وتسجيل سجل المراجعات. |
| إضافة خطافات الحوكمة حتى يستطيع Parlamento اعتماد وتجديد وإلغاء declaraciones. | Consejo de Gobernanza / Equipo de Contratos Inteligentes | استخدام Eventos Norito + ingestión de manifiesto. |
| تنفيذ جدول العقوبات (تخفيض الرسوم، cortando للبوند) المرتبط بانتهاكات SLA المقاسة عن بعد. | Consejo de Gobierno / Tesorería | Utilice el dispositivo de encendido `DealEngine`. |
| توثيق عملية النزاع ومصفوفة التصعيد. | Documentos / Gobernanza | Aquí hay un runbook de disputas + ayudantes de CLI. |

### 4. الميترينغ وتوزيع الرسوم

| المهمة | Propietario(s) | ملاحظات |
|------|----------|-------|
| Puede ingerir archivos desde Torii o `CapacityTelemetryV1`. | Torii Equipo | Incluye GiB-hora, PoR y tiempo de actividad. |
| Utilice el software `sorafs_node` para obtener más información sobre SLA. | Equipo de almacenamiento | المواءمة مع أوامر النسخ المتماثل وhandles chunker. |
| خط أنابيب التسوية: تحويل التليمترية + بيانات النسخ إلى pagos مقومة بـ XOR, y ملخصات جاهزة للحوكمة، وتسجيل حالة libro mayor. | Equipo de Tesorería / Almacenamiento | التوصيل إلى Exportaciones de Deal Engine/Tesorería. |
| تصدير paneles/alertas لصحة الميترينغ (ingestión de trabajos pendientes, تليمترية قديمة). | Observabilidad | Utilice el cable Grafana para SF-6/SF-7. |- يعرض Torii الآن `/v2/sorafs/capacity/telemetry` و `/v2/sorafs/capacity/state` (JSON + Norito) بحيث يمكن للمشغلين إرسال snapshots تليمترية لكل حقبة ويمكن للمراجعين استرجاع libro mayor الحتمي للتدقيق أو تغليف الأدلة.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- El dispositivo `PinProviderRegistry` incluye el punto final del dispositivo. Utilice la CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) para crear un hash o un alias.
- تنتج instantáneas الميترينغ إدخالات `CapacityTelemetrySnapshot` المثبتة على instantánea `metering`, y تغذي Prometheus exportaciones لوحة Grafana Adaptador de corriente para `docs/source/grafana_sorafs_metering.json`, adaptador de corriente GiB-hora y nano-SORA 【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Para suavizar la medición, obtener instantáneas `smoothed_gib_hours` e `smoothed_por_success_bps` para obtener información sobre EMA. Obtenga más información sobre los pagos.【crates/sorafs_node/src/metering.rs:401】

### 5. معالجة النزاعات والإلغاء| المهمة | Propietario(s) | ملاحظات |
|------|----------|-------|
| تعريف حمولة `CapacityDisputeV1` (صاحب الشكوى، evidencia, المزود المستهدف). | Consejo de Gobierno | مخطط Norito + مدقق. |
| دعم CLI لتقديم disputas y والرد عليها (مع مرفقات evidencia). | Grupo de Trabajo sobre Herramientas | ضمان hash حتمي لحزمة evidencia. |
| إضافة فحوص تلقائية لتكرار خروقات SLA (تصعيد تلقائي إلى disputa). | Observabilidad | عتبات تنبيه وخطافات حوكمة. |
| توثيق libro de jugadas الإلغاء (فترة سماح، إخلاء بيانات fijado). | Equipo de Documentos/Almacenamiento | الربط إلى وثيقة السياسة وoperator runbook. |

## متطلبات الاختبار y CI

- اختبارات وحدات لكل مدققات المخطط الجديدة (`sorafs_manifest`).
- اختبارات تكامل تحاكي: إعلان → أمر نسخ → medición → pago.
- flujo de trabajo de CI لإعادة توليد إعلانات/تليمترية السعة وضمان تزامن التواقيع (توسيع `ci/check_sorafs_fixtures.sh`).
- اختبارات تحميل لواجهة API de registro (entre 10k y 100k).

## التليمترية ولوحات المعلومات

- Panel de control de aplicaciones:
  - السعة المعلنة مقابل المستخدمة لكل مزود.
  - acumulación de pedidos أوامر النسخ ومتوسط ​​تأخير التعيين.
  - امتثال SLA (tiempo de actividad, معدل نجاح PoR).
  - تراكم الرسوم والغرامات لكل حقبة.
- Alertas:
  - مزود أقل من الحد الأدنى للسعة الملتزم بها.
  - أمر نسخ متعثر لأكثر من SLA.
  - إخفاقات خط أنابيب الميترينغ.

## مخرجات التوثيق- دليل المشغل لإعلان السعة وتجديد الالتزامات ومراقبة الاستخدام.
- دليل الحوكمة للموافقة على الإعلانات وإصدار الأوامر ومعالجة النزاعات.
- مرجع API لنقاط نهاية السعة وتنسيق أوامر النسخ المتماثل.
- أسئلة شائعة للـ mercado موجهة للمطورين.

## قائمة تحقق جاهزية GA

بند خارطة الطريق **SF-2c** يبوّب إطلاق الإنتاج على أدلة ملموسة عبر المحاسبة ومعالجة النزاعات والالتحاق. استخدموا الأدوات أدناه لإبقاء معايير القبول متزامنة مع التنفيذ.

### Contabilidad nocturna y conciliación XOR
- La instantánea del libro mayor XOR y el libro mayor XOR son:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  ينهي المساعد التنفيذ بكود غير صفري عند وجود تسويات أو غرامات مفقودة/زائدة، ويصدر ملخصا بصيغة Archivo de texto Prometheus.
- Nombre `SoraFSCapacityReconciliationMismatch` (tipo `dashboards/alerts/sorafs_capacity_rules.yml`)
  يطلق عندما تشير مقاييس reconciliación إلى فجوات؛ لوحات المعلومات موجودة تحت
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- Copia de JSON y hashes mediante `docs/examples/sorafs_capacity_marketplace_validation/`
  Paquetes de gobernanza de بجانب.

### Disputa y evidencia de reducción
- قدّم disputas عبر `sorafs_manifest_stub capacity dispute` (اختبارات:
  `cargo test -p sorafs_car --test capacity_cli`) حتى تبقى cargas útiles قياسية.
- شغّل `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` وحزم العقوبات
  (`record_capacity_telemetry_penalises_persistent_under_delivery`) لإثبات أن disputas y barras diagonales تُعاد حتميا.
- اتبع `docs/source/sorafs/dispute_revocation_runbook.md` لالتقاط الأدلة والتصعيد؛ اربط موافقات huelga في تقرير التحقق.### Pruebas de humo de incorporación y salida de proveedores
- أعد توليد artefactos للإعلان/التليمترية باستخدام `sorafs_manifest_stub capacity ...` y تشغيل اختبارات CLI قبل الإرسال (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Utilice Torii (`/v2/sorafs/capacity/declare`) o `/v2/sorafs/capacity/state` o Grafana. Utilice el dispositivo `docs/source/sorafs/capacity_onboarding_runbook.md`.
- أرشِف artefactos الموقعة ومخرجات reconciliación داخل `docs/examples/sorafs_capacity_marketplace_validation/`.

## الاعتماديات والتسلسل

1. إكمال SF-2b (política de admisión) - يعتمد Marketplace على مزودين مدققين.
2. Pulse el botón + el botón (هذا المستند) para seleccionar Torii.
3. Tubería de medición إكمال قبل تفعيل المدفوعات.
4. الخطوة الأخيرة: تفعيل توزيع الرسوم المحكوم من الحوكمة بعد التحقق من بيانات medición y puesta en escena.

يجب تتبع التقدم في hoja de ruta مع الإحالات إلى هذا المستند. حدّث hoja de ruta بمجرد وصول كل قسم رئيسي (المخططات، plano de control, التكامل، medición, معالجة النزاعات) إلى حالة característica completa.