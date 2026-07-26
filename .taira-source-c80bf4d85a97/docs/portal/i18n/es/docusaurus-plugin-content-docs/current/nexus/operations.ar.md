---
lang: es
direction: ltr
source: docs/portal/docs/nexus/operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones-nexus
título: دليل تشغيل Nexus
descripción: ملخص عملي لخطوات تشغيل مشغل Nexus, يعكس `docs/source/nexus_operations.md`.
---

Asegúrese de que el dispositivo esté conectado a `docs/source/nexus_operations.md`. فهي تلخص قائمة التشغيل، ونقاط إدارة التغيير، ومتطلبات تغطية القياس التيجب على مشغلي Nexus اتباعها.

## قائمة دورة الحياة

| المرحلة | الإجراءات | الأدلة |
|-------|--------|----------|
| ما قبل الإقلاع | Para obtener más información, consulte el enlace `profile = "iroha3"` y el siguiente enlace. | مخرجات `scripts/select_release_profile.py`, سجل checksum, حزمة بيانات موقعة. |
| مواءمة الكتالوج | Utilice el `[nexus]`, el dispositivo de encendido y el DA y el dispositivo de encendido `--trace-config`. | مخرجات `irohad --sora --config ... --trace-config` محفوظة مع تذكرة incorporación. |
| فحص الدخان Y التحويل | Utilice `irohad --sora --config ... --trace-config` para conectar la CLI (`FindNetworkStatus`) a sus terminales y terminales. | سجل فحص الدخان + تأكيد Alertmanager. |
| حالة مستقرة | راقب لوحات/تنبيهات، دوّر المفاتيح حسب وتيرة الحوكمة، وحدث configs/runbooks عند تغير البيانات. | محاضر مراجعة ربع سنوية, لقطات لوحات، أرقام تذاكر التدوير. |

يبقى incorporación التفصيلي (استبدال المفاتيح، قوالب التوجيه، خطوات ملف الإصدار) في `docs/source/sora_nexus_operator_onboarding.md`.

## إدارة التغيير1. **تحديثات الإصدار** - تتبع الإعلانات في `status.md`/`roadmap.md`؛ أرفق قائمة incorporación مع كل PR للإصدار.
2. **تغييرات بيانات lane** - تحقق من الحزم الموقعة من Space Directory y تحت `docs/source/project_tracker/nexus_config_deltas/`.
3. **تغييرات الإعداد** - كل تغيير في `config/config.toml` يحتاج تذكرة تشير إلى lane/data-space. احفظ نسخة منقحة من الإعداد الفعلي عند انضمام أو ترقية العقد.
4. **تمارين الرجوع** - درّب ربع سنوي على إجراءات الإيقاف/الاستعادة/فحص الدخان؛ Aquí está el mensaje `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **موافقات الامتثال** - يجب أن تحصل lanes الخاصة/CBDC على موافقة امتثال قبل تعديل سياسة DA أو مفاتيح تنقيح القياس (انظر `docs/source/cbdc_lane_playbook.md`).

## القياس y SLO

- Contenidos: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, todos los archivos disponibles en el SDK (modelo `android_operator_console.json`).
- Contenido: `dashboards/alerts/nexus_audit_rules.yml` y Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- المقاييس التي يجب مراقبتها:
  - `nexus_lane_height{lane_id}` - تنبيه عند عدم التقدم لثلاث خانات.
  - `nexus_da_backlog_chunks{lane_id}` - تنبيه عند تجاوز العتبات لكل carril (الافتراضي 64 público / 8 privado).
  - `nexus_settlement_latency_seconds{lane_id}` - تنبيه عندما يتجاوز P99 900 ms (público) y 1200 ms (privado).
  - `torii_request_failures_total{scheme="norito_rpc"}` - تنبيه إذا تجاوز معدل الخطأ خلال 5 دقائق 2%.
  - `telemetry_redaction_override_total` - Septiembre 2 min تأكد من وجود تذاكر امتثال للتجاوزات.
- نفذ قائمة معالجة القياس في [خطة معالجة قياس Nexus](./nexus-telemetry-remediation) على الأقل فصليا وأرفق النموذج المكتمل بملاحظات مراجعة التشغيل.

## مصفوفة الحوادث| الشدة | التعريف | الاستجابة |
|----------|------------|----------|
| Septiembre 1 | Después de completar el espacio de datos, haga clic en él durante 15 días y complete el proceso. | نادِ Nexus Primario + Ingeniería de lanzamiento + Cumplimiento, جمّد القبول، اجمع الأدلة, انشر تواصل 30 دقيقة, فشل rollout للبيانات. | نادِ Nexus Primario + SRE, عالج <=4 ساعات، سجّل المتابعات خلال يومي عمل. |
| Septiembre 3 | انحراف غير معطل (docs، تنبيهات). | سجّل في المتتبع، وجدول إصلاح داخل السبرنت. |

يجب أن تسجل تذاكر الحوادث IDs الخاصة بـ lane/data-space المتأثرة، وبصمات البيانات، والخط الزمني، والمقاييس/السجلات الداعمة، ومهام المتابعة والمالكين.

## أرشيف الأدلة

- خزّن الحزم/البيانات/صادرات القياس تحت `artifacts/nexus/<lane>/<date>/`.
- Haga clic en el botón + `--trace-config` para su uso.
- أرفق محاضر المجلس + القرارات الموقعة عند تطبيق تغييرات الإعداد أو البيانات.
- La unidad Prometheus está conectada a la unidad Nexus durante 12 horas.
- دوّن تعديلات الدليل في `docs/source/project_tracker/nexus_config_deltas/README.md` حتى يعرف المدققون متى تغيرت المسؤوليات.

## مواد ذات صلة- Texto: [Nexus descripción general](./nexus-overview)
- Descripción: [Especificación Nexus](./nexus-spec)
- Carril trasero: [modelo de carril Nexus] (./nexus-lane-model)
- الانتقال وشيمات التوجيه: [Notas de transición Nexus](./nexus-transition-notes)
- incorporación المشغلين: [Incorporación del operador Sora Nexus] (./nexus-operator-onboarding)
- معالجة القياس: [Nexus plan de remediación de telemetría](./nexus-telemetry-remediation)