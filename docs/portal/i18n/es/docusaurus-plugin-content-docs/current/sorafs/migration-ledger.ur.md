---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: SoraFS مائیگریشن لیجر
descripción: کینونیکل چینج لاگ۔
---

> [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md) سے ماخوذ۔

# SoraFS مائیگریشن لیجر

یہ لیجر SoraFS Arquitectura RFC میں ریکارڈ کردہ مائیگریشن چینج لاگ کی عکاسی کرتا ہے۔ اندراجات
سنگ میل کے حساب سے گروپ ہوتی ہیں اور ventana efectiva, متاثرہ ٹیمیں، اور مطلوبہ acciones درج
کرتی ہیں۔ پلان میں اپڈیٹس لازمی طور پر اس صفحے اور RFC
(`docs/source/sorafs_architecture_rfc.md`) دونوں میں تبدیلی کریں تاکہ consumidores intermedios
ہم آہنگ رہیں۔

| سنگ میل | موثر مدت | تبدیلی کا خلاصہ | متاثرہ ٹیمیں | ایکشن آئٹمز | حیثیت |
|---------|----------|-----------------|--------------|-------------|-------|
| M1 | ہفتے 7–12 | Accesorios deterministas de CI نافذ کرتا ہے؛ alias pruebas puesta en escena میں دستیاب ہیں؛ herramientas de indicadores de expectativas explícitas دیتا ہے۔ | Documentos, almacenamiento, gobernanza | Accesorios کو firmado رکھیں، registro de preparación میں alias رجسٹر کریں، listas de verificación de liberación کو `--car-digest/--root-cid` cumplimiento کے ساتھ اپڈیٹ کریں۔ | ⏳ زیر التوا |

گورننس plano de control کی minutos جو ان hitos کو حوالہ دیتی ہیں `docs/source/sorafs/` کے تحت
موجود ہیں۔ ٹیموں کو ہر قطار کے نیچے viñetas con fecha شامل کرنے چاہئیں جب نمایاں واقعات
ہوئیں (مثلا نئے registros de alias یا retrospectivas de incidentes de registro) تاکہ قابلِ آڈٹ
ریکارڈ فراہم ہو۔

## تازہ ترین اپڈیٹس- 2025-11-01 — `migration_roadmap.md` کو consejo de gobierno اور listas de operadores میں revisión کے
  لیے بھیجا گیا؛ اگلی sesión del consejo میں aprobación کا انتظار ہے (ref:
  `docs/source/sorafs/council_minutes_2025-10-29.md` seguimiento).
- 2025-11-02 — Registro de PIN Registro ISI اب `sorafs_manifest` ayudantes کے ذریعے fragmentador compartido/
  validación de políticas نافذ کرتا ہے، جس سے rutas en cadena Torii verifica کے ساتھ alineado رہتے ہیں۔
- 2026-02-13: Fases de implementación de anuncios del proveedor (R0–R3) لیجر میں شامل کی گئیں اور متعلقہ paneles
  اور guía del operador شائع کی گئی
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).