---
lang: es
direction: ltr
source: docs/portal/docs/nexus/overview.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: descripción general del nexo
título: نظرة عامة على Sora Nexus
descripción: ملخص عالي المستوى لمعمارية Iroha 3 (Sora Nexus) مع مؤشرات إلى وثائق المستودع الأحادي المعيارية.
---

توسع Nexus (Iroha 3) Iroha 2 بتنفيذ متعدد المسارات، Y مساحات بيانات محددة بالحوكمة، Todos los archivos están disponibles en el SDK. تعكس هذه الصفحة الملخص الجديد `docs/source/nexus_overview.md` في المستودع الأحادي حتى يتمكن قراء البوابة من فهم كيفية ترابط أجزاء المعمارية بسرعة.

## خطوط الإصدارات

- **Iroha 2** - عمليات نشر مستضافة ذاتيا للكونسورتيوم أو الشبكات الخاصة.
- **Iroha 3 / Sora Nexus** - الشبكة العامة متعددة المسارات حيث يسجل المشغلون مساحات البيانات (DS) ويرثون أدوات مشتركة للحوكمة والتسوية والملاحظة.
- كلا الخطين يُبنيان من نفس مساحة العمل (IVM + cadena de herramientas Kotodama), que incluye SDK y ABI. وaccesorios Norito قابلة للنقل. La unidad de disco duro `iroha3-<version>-<os>.tar.zst` y la unidad Nexus راجع `docs/source/sora_nexus_operator_onboarding.md` لقائمة التحقق بملء الشاشة.

## اللبنات الأساسية| المكون | الملخص | روابط البوابة |
|-----------|---------|--------------|
| مساحة البيانات (DS) | نطاق تنفيذ/تخزين محدد بالحوكمة يمتلك مسارا واحدا أو أكثر، ويعلن مجموعات المدققين وفئة الخصوصية وسياسة الرسوم + DA. | راجع [Nexus spec](./nexus-spec) لمخطط البيان. |
| Carril | شريحة تنفيذ حتمية تصدر تعهدات يرتبها حلقة NPoS العالمية. Utilice el carril `default_public`, `public_custom`, `private_permissioned` y `hybrid_confidential`. | يلتقط [نموذج Lane](./nexus-lane-model) الهندسة وبوادئ التخزين والاحتفاظ. |
| خطة الانتقال | معرفات placeholder ومراحل توجيه وتعبئة بملفين تتبع كيف تتطور عمليات النشر أحادية المسار إلى Nexus. | توثق [ملاحظات الانتقال](./nexus-transition-notes) كل مرحلة ترحيل. |
| Directorio espacial | عقد سجل يخزن بيانات DS ونسخها. يقوم المشغلون بمطابقة إدخالات الكتالوج مع هذا الدليل قبل الانضمام. | متعقب فروقات البيان موجود تحت `docs/source/project_tracker/nexus_config_deltas/`. |
| كتالوج المسارات | قسم الإعداد `[nexus]` يربط معرّفات Lane بالأسماء المستعارة وسياسات التوجيه وعتبات DA. يقوم `irohad --sora --config … --trace-config` بطباعة الكتالوج المحسوم لأغراض التدقيق. | Utilice `docs/source/sora_nexus_operator_onboarding.md` para obtener más información. |
| موجه التسوية | La moneda XOR es una moneda de CBDC. | يوضح `docs/source/cbdc_lane_playbook.md` مقابض السياسة وبوابات القياس. || القياس/SLOs | Accesorios y accesorios `dashboards/grafana/nexus_*.json` Accesorios y accesorios DA y accesorios y accesorios الحوكمة. | يوضح [خطة معالجة القياس](./nexus-telemetry-remediation) اللوحات والتنبيهات وأدلة التدقيق. |

## لقطة الإطلاق

| المرحلة | التركيز | معايير الخروج |
|-------|-------|-----------------------|
| N0 - بيتا مغلقة | Registrador يديره المجلس (`.sora`), انضمام يدوي للمشغلين، كتالوج مسارات ثابت. | بيانات DS موقعة + عمليات تسليم حوكمة مجربة. |
| N1 - إطلاق عام | Este es el registrador `.nexus` y el registrador de archivos XOR. | اختبارات تزامن resolver/gateway, لوحات تسوية الفواتير، تمارين محاكاة للنزاعات. |
| N2 - توسع | `.dao`, y la API de configuración está configurada para funcionar correctamente. | مصنوعات امتثال بإصدارات، مجموعة أدوات هيئة السياسات متاحة, تقارير شفافية الخزانة. |
| Cámara NX-12/13/14 | يجب أن يصدر محرك الامتثال ولوحات القياس والوثائق معا قبل تجارب الشركاء. | نشر [Descripción general de Nexus](./nexus-overview) + [Operaciones de Nexus](./nexus-operations), توصيل اللوحات، دمج محرك السياسات. |

## مسؤوليات المشغلين1. **نظافة الإعداد** - أبقِ `config/config.toml` متزامنا مع كتالوج المسارات ومساحات البيانات المنشور؛ Utilice `--trace-config` para obtener más información.
2. **تتبع البيانات** - طابق إدخالات الكتالوج مع أحدث حزمة Space Directory قبل الانضمام أو ترقية العقد.
3. **تغطية القياس** - Utilice los controladores `nexus_lanes.json` e `nexus_settlement.json` y el SDK. Utilice PagerDuty y utilice aplicaciones de software y dispositivos de seguridad.
4. **الإبلاغ عن الحوادث** - اتبع مصفوفة الشدة في [operaciones Nexus](./nexus-operations) y قدّم تقارير RCA خلال خمسة أيام عمل.
5. **الجاهزية للحوكمة** - احضر تصويتات مجلس Nexus التي تؤثر على مساراتك وتدرّب على تعليمات الرجوع ربع سنويا (يتم تتبعها عبر `docs/source/project_tracker/nexus_config_deltas/`).

## انظر أيضا

- النظرة العامة الرسمية: `docs/source/nexus_overview.md`
- Contenido del sitio: [./nexus-spec](./nexus-spec)
- Enlaces: [./nexus-lane-model](./nexus-lane-model)
- خطة الانتقال: [./nexus-transition-notes](./nexus-transition-notes)
- خطة معالجة القياس: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- دليل العمليات: [./nexus-operaciones](./nexus-operations)
- Nombre del fabricante: `docs/source/sora_nexus_operator_onboarding.md`