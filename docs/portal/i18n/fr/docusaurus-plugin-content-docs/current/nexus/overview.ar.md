---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/overview.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu du lien
titre : نظرة عامة على Sora Nexus
description: ملخص عالي المستوى لمعمارية Iroha 3 (Sora Nexus) مع مؤشرات إلى وثائق المستودع الأحادي المعيارية.
---

توسع Nexus (Iroha 3) Iroha 2 بتنفيذ متعدد المسارات، ومساحات محددة بالحوكمة، Vous avez besoin du SDK. تعكس هذه الصفحة الملخص الجديد `docs/source/nexus_overview.md` في المستودع الأحادي حتى يتمكن قراء البوابة من فهم كيفية ترابط أجزاء المعمارية بسرعة.

## خطوط الإصدارات

- **Iroha 2** - عمليات نشر مستضافة ذاتيا للكونسورتيوم أو الشبكات الخاصة.
- **Iroha 3 / Sora Nexus** - Système d'alimentation en eau potable (DS) et أدوات مشتركة للحوكمة والتسوية والملاحظة.
- Il s'agit d'une solution à base de données (IVM + chaîne d'outils Kotodama) pour le SDK et ABI. وluminaires Norito قابلة للنقل. يقوم المشغلون بتنزيل حزمة `iroha3-<version>-<os>.tar.zst` للانضمام إلى Nexus؛ راجع `docs/source/sora_nexus_operator_onboarding.md` لقائمة التحقق بملء الشاشة.

## اللبنات الأساسية| المكون | الملخص | روابط البوابة |
|-----------|---------|--------------|
| مساحة البيانات (DS) | نطاق تنفيذ/تخزين محدد بالحوكمة يمتلك مسارا واحدا أو أكثر، ويعلن مجموعات المدققين وفئة الخصوصية وسياسة الرسوم + DA. | راجع [Nexus spec](./nexus-spec) لمخطط البيان. |
| Voie | شريحة تنفيذ حتمية تصدر تعهدات يرتبها حلقة NPoS العالمية. تشمل فئات Lane `default_public` et `public_custom` et `private_permissioned` et `hybrid_confidential`. | يلتقط [نموذج Lane](./nexus-lane-model) الهندسة وبوادئ التخزين والاحتفاظ. |
| خطة الانتقال | معرفات placeholder ومراحل توجيه وتعبئة بملفين تتبع كيف تتطور عمليات النشر أحادية المسار إلى Nexus. | توثق [ملاحظات الانتقال](./nexus-transition-notes) كل مرحلة ترحيل. |
| Répertoire spatial | Il s'agit de la DS ونسخها. يقوم المشغلون بمطابقة إدخالات الكتالوج مع هذا الدليل قبل الانضمام. | متعقب فروقات البيان موجود تحت `docs/source/project_tracker/nexus_config_deltas/`. |
| كتالوج المسارات | قسم الإعداد `[nexus]` يربط معرّفات Lane بالأسماء المستعارة وسياسات التوجيه وعتبات DA. يقوم `irohad --sora --config … --trace-config` بطباعة الكتالوج المحسوم لأغراض التدقيق. | استخدم `docs/source/sora_nexus_operator_onboarding.md` لشرح سطر الأوامر. |
| موجه التسوية | La société XOR est devenue une CBDC proche de la société. | يوضح `docs/source/cbdc_lane_playbook.md` مقابض السياسة وبوابات القياس. || القياس/SLO | لوحات وأجهزة إنذار تحت `dashboards/grafana/nexus_*.json` تلتقط ارتفاع المسارات وتراكم DA وزمن تسوية الصفقات وعمق طابور الحوكمة. | يوضح [خطة معالجة القياس](./nexus-telemetry-remediation) اللوحات والتنبيهات وأدلة التدقيق. |

## لقطة الإطلاق

| المرحلة | التركيز | معايير الخروج |
|-------|-------|--------------------|
| N0 - بيتا مغلقة | Le registraire est يديره المجلس (`.sora`), انضمام يدوي للمشغلين, كتالوج مسارات ثابت. | بيانات DS موقعة + عمليات تسليم حوكمة مجربة. |
| N1 - إطلاق عام | Il s'agit d'un registraire `.nexus` pour XOR. | Il s'agit d'un résolveur/passerelle, ou d'une passerelle. |
| N2 - توسع | Il s'agit d'une API `.dao` qui vous permet d'utiliser l'API pour créer des liens. | مصنوعات امتثال بإصدارات، مجموعة أدوات هيئة السياسات متاحة، تقارير شفافية الخزانة. |
| Par NX-13/12/14 | يجب أن يصدر محرك الامتثال ولوحات القياس والوثائق معا قبل تجارب الشركاء. | نشر [Présentation Nexus](./nexus-overview) + [Opérations Nexus](./nexus-operations) ، توصيل اللوحات، دمج محرك السياسات. |

## مسؤوليات المشغلين1. **نظافة الإعداد** - أبقِ `config/config.toml` متزامنا مع كتالوج المسارات ومساحات البيانات المنشور؛ وأرشِف مخرجات `--trace-config` مع كل تذكرة إصدار.
2. **تتبع البيانات** - طابق إدخالات الكتالوج مع أحدث حزمة Space Directory قبل الانضمام أو ترقية العقد.
3. **Détails ** - Outils SDK `nexus_lanes.json` et `nexus_settlement.json` et SDK pour les utilisateurs Les fonctionnalités de PagerDuty sont également disponibles pour les utilisateurs de PagerDuty.
4. **الإبلاغ عن الحوادث** - اتبع مصفوفة الشدة في [Opérations Nexus](./nexus-operations) et تقارير RCA خلال خمسة أيام عمل.
5. **جاهزية للحوكمة** - احضر تصويتات مجلس Nexus التي تؤثر على مساراتك وتدرّب على تعليمات الرجوع ربع سنويا (يتم تتبعها عبر `docs/source/project_tracker/nexus_config_deltas/`).

## انظر أيضا

- Nom de l'utilisateur : `docs/source/nexus_overview.md`
- Nom du produit : [./nexus-spec](./nexus-spec)
- Nom du produit : [./nexus-lane-model](./nexus-lane-model)
- Fichier de référence : [./nexus-transition-notes](./nexus-transition-notes)
- خطة معالجة القياس : [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- دليل العمليات : [./nexus-opérations](./nexus-operations)
- دليل انضمام المشغلين : `docs/source/sora_nexus_operator_onboarding.md`