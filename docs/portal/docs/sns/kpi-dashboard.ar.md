---
lang: ar
direction: rtl
source: docs/portal/docs/sns/kpi-dashboard.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3649db9b00f9be968cfeb98bc34bbc797aaf22d7ac3936698b4f562094911073
source_last_modified: "2025-11-15T05:28:18.938087+00:00"
translation_last_reviewed: 2026-01-01
---

# لوحة مؤشرات KPI لخدمة أسماء سورا

تمنح لوحة مؤشرات KPI الأمناء والحراس والمنظمين مكانا واحدا لمراجعة إشارات الاعتماد والأخطاء والإيرادات قبل إيقاع الملحق الشهري (SN-8a). يوجد تعريف Grafana في المستودع عند `dashboards/grafana/sns_suffix_analytics.json`، وتعكس البوابة اللوحات نفسها عبر iframe مضمّن بحيث تطابق التجربة نسخة Grafana الداخلية.

## المرشحات ومصادر البيانات

- **مرشح اللاحقة** – يقود استعلامات `sns_registrar_status_total{suffix}` حتى يمكن فحص `.sora` و`.nexus` و`.dao` بشكل مستقل.
- **مرشح الإصدار المجمع** – يقيّد مقاييس `sns_bulk_release_payment_*` حتى يتمكن فريق المالية من مطابقة بيان registrar محدد.
- **المقاييس** – يجلب من Torii (`sns_registrar_status_total`, `torii_request_duration_seconds`)، وCLI الخاص بالـguardian (`guardian_freeze_active`)، و`sns_governance_activation_total`، ومقاييس مساعد bulk-onboarding.

## اللوحات

1. **التسجيلات (آخر 24h)** – عدد أحداث registrar الناجحة لللاحقة المحددة.
2. **تفعيلات الحوكمة (30d)** – حركات الميثاق/الملحق المسجلة بواسطة CLI.
3. **معدل تدفق registrar** – معدل الإجراءات الناجحة للـregistrar لكل لاحقة.
4. **أنماط أخطاء registrar** – معدل 5 دقائق لعدادات `sns_registrar_status_total` الموسومة بالخطأ.
5. **نوافذ تجميد guardian** – محددات مباشرة حيث يُبلغ `guardian_freeze_active` عن تذكرة تجميد مفتوحة.
6. **وحدات الدفع الصافية حسب الأصل** – الإجماليات التي يبلغ عنها `sns_bulk_release_payment_net_units` لكل أصل.
7. **طلبات مجمعة لكل لاحقة** – أحجام الـmanifest لكل معرف لاحقة.
8. **الوحدات الصافية لكل طلب** – حساب بنمط ARPU مشتق من مقاييس الإصدار.

## قائمة مراجعة KPI الشهرية

يقود مسؤول المالية مراجعة دورية في أول ثلاثاء من كل شهر:

1. افتح صفحة البوابة **Analytics → SNS KPI** (أو لوحة Grafana `sns-kpis`).
2. التقط تصدير PDF/CSV لجداول معدل تدفق registrar والإيرادات.
3. قارن اللواحق لاكتشاف خروقات SLA (ارتفاعات معدل الخطأ، محددات مجمدة >72 h، فروق ARPU >10%).
4. سجّل الملخصات وبنود العمل في إدخال الملحق المناسب تحت `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
5. أرفق عناصر لوحة البيانات المصدّرة بعملية commit الخاصة بالملحق واربطها في جدول أعمال المجلس.

إذا كشفت المراجعة عن خروقات SLA، افتح حادث PagerDuty للمالك المتأثر (registrar duty manager، guardian on-call، أو steward program lead) وتابع المعالجة في سجل الملحق.
