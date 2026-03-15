---
lang: ar
direction: rtl
source: docs/connect_config.md
status: complete
translator: manual
source_hash: 15799a5698133ba1d6c5510d71d17fd60934519df890f83cb53b49d56980dc5b
source_last_modified: "2025-11-05T17:16:36.700630+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/connect_config.md (Torii Connect Configuration) -->

## إعداد Torii Connect

يقدّم Iroha Torii نقاط نهاية WebSocket اختيارية بأسلوب WalletConnect بالإضافة
إلى relay بسيط داخل العقدة عندما يكون feature Cargo المسمى `connect`
مفعَّلًا (القيمة الافتراضية). يتم التحكّم في سلوك runtime عبر الإعدادات:

- اضبط `connect.enabled=false` لتعطيل جميع مسارات Connect
  (`/v2/connect/*`).
- اتركها `true` (القيمة الافتراضية) لتمكين نقاط نهاية جلسات WebSocket
  ومسار `/v2/connect/status`.

تجاوزات متغيرات البيئة (إعداد المستخدم → الإعداد الفعلي):

- `CONNECT_ENABLED` (نوع bool؛ افتراضيًا `true`)
- `CONNECT_WS_MAX_SESSIONS` (`usize`; افتراضيًا `10000`)
- `CONNECT_WS_PER_IP_MAX_SESSIONS` (`usize`; افتراضيًا `10`)
- `CONNECT_WS_RATE_PER_IP_PER_MIN` (`u32`; افتراضيًا `120`)
- `CONNECT_FRAME_MAX_BYTES` (`usize`; افتراضيًا `64000`)
- `CONNECT_SESSION_BUFFER_MAX_BYTES` (`usize`; افتراضيًا `262144`)
- `CONNECT_PING_INTERVAL_MS` (مدة؛ افتراضيًا `30000`)
- `CONNECT_PING_MISS_TOLERANCE` (`u32`; افتراضيًا `3`)
- `CONNECT_PING_MIN_INTERVAL_MS` (مدة؛ افتراضيًا `15000`)
- `CONNECT_DEDUPE_CAP` (`usize`; افتراضيًا `8192`)
- `CONNECT_RELAY_ENABLED` (نوع bool؛ افتراضيًا `true`)
- `CONNECT_RELAY_STRATEGY` (string؛ افتراضيًا `"broadcast"`)
- `CONNECT_P2P_TTL_HOPS` (`u8`; افتراضيًا `0`)

ملاحظات:

- تستخدم `CONNECT_SESSION_TTL_MS` و `CONNECT_DEDUPE_TTL_MS` قيم مدة (duration
  literals) في إعدادات المستخدم، وتُربَطان بالحقول الفعلية `session_ttl`
  و`dedupe_ttl`.
- `CONNECT_WS_PER_IP_MAX_SESSIONS=0` يعطّل حد الجلسات لكل عنوان IP.
- `CONNECT_WS_RATE_PER_IP_PER_MIN=0` يعطّل محدد معدل المصافحة لكل عنوان IP.
- يقوم نظام heartbeat بتقييد الفترة الزمنية المكوّنة إلى الحد الأدنى
  المناسب للمتصفحات (`ping_min_interval_ms`)، ويتحمّل الخادم قيمة
  `ping_miss_tolerance` من رسائل pong المفقودة على التوالي قبل إغلاق
  WebSocket وزيادة المقياس `connect.ping_miss_total`.
- عند تعطيل Connect في runtime (`connect.enabled=false`)، لا يتم تسجيل
  مسارات الـ WebSocket ومسار الحالة، وتعيد الطلبات إلى
  `/v2/connect/ws` و`/v2/connect/status` استجابة 404.
- يتطلّب الخادم `sid` يقدّمه العميل في `​/v2/connect/session` (بصيغة
  base64url أو hex بطول 32 بايت). لم يَعُد الخادم يولّد `sid` احتياطيًا.

انظر أيضًا:
`crates/iroha_config/src/parameters/{user,actual}.rs` وقيم الافتراضات في
`crates/iroha_config/src/parameters/defaults.rs` (الوحدة `connect`).

</div>
