---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : opérations de connexion
titre : دليل تشغيل Nexus
description: ملخص عملي لخطوات تشغيل مشغل Nexus, يعكس `docs/source/nexus_operations.md`.
---

استخدم هذه الصفحة كمرجع سريع موازي لـ `docs/source/nexus_operations.md`. فهي تلخص قائمة التشغيل، ونقاط إدارة التغيير، ومتطلبات تغطية القياس التي يجب على مشغلي Nexus اتباعها.

## قائمة دورة الحياة

| المرحلة | الإجراءات | الأدلة |
|-------|--------|--------------|
| ما قبل الإقلاع | تحقق من بصمات/تواقيع الإصدار، أكد `profile = "iroha3"`, وجهز قوالب الإعداد. | Utilisez `scripts/select_release_profile.py` pour la somme de contrôle et utilisez la somme de contrôle. |
| مواءمة الكتالوج | حدّث كتالوج `[nexus]`, سياسة التوجيه، وعتبات DA وفق بيان المجلس، ثم التقط `--trace-config`. | مخرجات `irohad --sora --config ... --trace-config` محفوظة مع تذكرة intégration. |
| فحص الدخان والتحويل | شغّل `irohad --sora --config ... --trace-config`, نفّذ فحص الدخان في CLI (`FindNetworkStatus`), تحقق من صادرات القياس واطلب القبول. | سجل فحص الدخان + تأكيد Alertmanager. |
| حالة مستقرة | Il existe également des configs/runbooks et des configs/runbooks. | محاضر مراجعة ربع سنوية، لقطات لوحات، أرقام تذاكر التدوير. |

L'intégration (استبدال المفاتيح، قوالب التوجيه، خطوات ملف الإصدار) est `docs/source/sora_nexus_operator_onboarding.md`.

## إدارة التغيير1. **تحديثات الإصدار** - تتبع الإعلانات في `status.md`/`roadmap.md`؛ Il s'agit de l'intégration et des relations publiques.
2. **Voie d'accès aux voies** - Accès à l'annuaire spatial et à l'annuaire `docs/source/project_tracker/nexus_config_deltas/`.
3. **تغييرات الإعداد** - كل تغيير في `config/config.toml` يحتاج تذكرة تشير إلى voie/espace de données. احفظ نسخة منقحة من الإعداد الفعلي عند انضمام أو ترقية العقد.
4. **تمارين الرجوع** - درّب ربع سنوي على إجراءات الإيقاف/الاستعادة/فحص الدخان؛ دوّن النتائج تحت `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **موافقات الامتثال** - يجب أن تحصل lanes الخاصة/CBDC على موافقة امتثال قبل تعديل سياسة DA أو مفاتيح تنقيح القياس (انظر `docs/source/cbdc_lane_playbook.md`).

## القياس et SLO

- Paramètres : `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, et les fichiers du SDK (comme `android_operator_console.json`).
- Fonctions : `dashboards/alerts/nexus_audit_rules.yml` et Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- المقاييس التي يجب مراقبتها :
  - `nexus_lane_height{lane_id}` - تنبيه عند عدم التقدم لثلاث خانات.
  - `nexus_da_backlog_chunks{lane_id}` - تنبيه عند تجاوز العتبات لكل voie (الافتراضي 64 publiques / 8 privées).
  - `nexus_settlement_latency_seconds{lane_id}` - Fonctionne avec P99 900 ms (public) et 1 200 ms (privé).
  - `torii_request_failures_total{scheme="norito_rpc"}` - تنبيه إذا تجاوز معدل الخطأ خلال 5 دقائق 2%.
  - `telemetry_redaction_override_total` - 2 septembre تأكد من وجود تذاكر امتثال للتجاوزات.
- نفذ قائمة معالجة القياس في [خطة معالجة قياس Nexus](./nexus-telemetry-remediation) على الأقل فصليا وأرفق النموذج المكتمل بملاحظات مراجعة التشغيل.

## مصفوفة الحوادث| الشدة | التعريف | الاستجابة |
|--------------|------------|--------------|
| 1 septembre | Il s'agit d'un espace de données qui dure environ 15 jours et qui contient des informations sur l'espace de données. | نادِ Nexus Primary + Release Engineering + Compliance, جمّد القبول، اجمع الأدلة، انشر تواصل <=60 دقيقة، RCA <=5 أيام عمل. |
| 2 septembre | La voie SLA est limitée à 30 jours pour le déploiement. | نادِ Nexus Primary + SRE, <=4 ساعات، سجّل المتابعات خلال يومي عمل. |
| 3 septembre | انحراف غير معطل (docs, تنبيهات). | سجّل في المتتبع، وجدول إصلاح داخل السبرنت. |

Vous pouvez utiliser les identifiants de la voie/espace de données et les identifiants de la voie et de l'espace de données. والمقاييس/السجلات الداعمة، ومهام المتابعة والمالكين.

## أرشيف الأدلة

- خزّن الحزم/البيانات/صادرات القياس تحت `artifacts/nexus/<lane>/<date>/`.
- احتفظ بالتهيئات المنقحة + مخرجات `--trace-config` pour إصدار.
- أرفق محاضر المجلس + القرارات الموقعة عند تطبيق تغييرات الإعداد أو البيانات.
- احتفظ بلقطات Prometheus الأسبوعية ذات الصلة بمقاييس Nexus لمدة 12 شهرا.
- دوّن تعديلات الدليل في `docs/source/project_tracker/nexus_config_deltas/README.md` حتى يعرف المدققون متى تغيرت المسؤوليات.

## مواد ذات صلة- Nom du produit : [Présentation Nexus](./nexus-overview)
- Nom : [Spécification Nexus](./nexus-spec)
- Voie هندسة : [Modèle de voie Nexus](./nexus-lane-model)
- Notes de transition Nexus](./nexus-transition-notes)
- intégration المشغلين : [intégration de l'opérateur Sora Nexus](./nexus-operator-onboarding)
- معالجة القياس : [Plan de remédiation de télémétrie Nexus](./nexus-telemetry-remediation)