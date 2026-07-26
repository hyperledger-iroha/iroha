---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: SoraFS مائیگریشن لیجر
description: ہر مائیگریشن سنگ میل، ذمہ داران اور مطلوبہ فالو اپس کو ٹریک کرنے والا کینونیکل چینج لاگ۔
---

> [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md) سے ماخوذ۔

# SoraFS مائیگریشن لیجر

یہ لیجر SoraFS Arquitetura RFC میں ریکارڈ کردہ مائیگریشن چینج لاگ کی عکاسی کرتا ہے۔ اندراجات
سنگ میل کے حساب سے گروپ ہوتی ہیں اور janela efetiva, متاثرہ ٹیمیں, اور مطلوبہ ações درج
کرتی ہیں۔ O que você precisa saber sobre o RFC
(`docs/source/sorafs_architecture_rfc.md`) دونوں میں تبدیلی کریں تاکہ consumidores downstream
ہم آہنگ رہیں۔

| سنگ میل | Máquinas de lavar | تبدیلی کا خلاصہ | متاثرہ ٹیمیں | ایکشن آئٹمز | حیثیت |
|---------|----------|-------|-------------|-------------|-------|
| M1 | ہفتے 7–12 | Luminárias determinísticas CI teste de provas de alias میں دستیاب ہیں؛ ferramentas de sinalizadores de expectativa explícita | Documentos, armazenamento, governança | Fixtures کو assinado رکھیں، registro de teste میں aliases رجسٹر کریں، listas de verificação de liberação کو `--car-digest/--root-cid` aplicação کے ساتھ اپڈیٹ کریں۔ | ⏳ زیر التوا |

گورننس plano de controle کی minutos جو ان marcos کو حوالہ دیتی ہیں `docs/source/sorafs/` کے تحت
موجود ہیں۔ ٹیموں کو ہر قطار کے نیچے marcadores datados شامل کرنے چاہئیں جب نمایاں واقعات
ہوئیں (مثلا نئے registros de alias یا retrospectivas de incidentes de registro) تاکہ قابلِ آڈٹ
ریکارڈ فراہم ہو۔

## تازہ ترین اپڈیٹس

- 2025-11-01 — `migration_roadmap.md` کو conselho de governança اور listas de operadores میں revisão کے
  لیے بھیجا گیا؛ اگلی sessão do conselho میں aprovação کا انتظار ہے (ref:
  Acompanhamento `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 02/11/2025 — Registro de PIN ISI اب `sorafs_manifest` helpers کے ذریعے chunker compartilhado/
  validação de política نافذ کرتا ہے، جس سے caminhos na cadeia Torii verificações کے ساتھ alinhadas رہتے ہیں۔
- 13/02/2026 — Fases de lançamento do anúncio do provedor (R0–R3) لیجر میں شامل کی گئیں اور متعلقہ painéis
  اور orientação do operador شائع کی گئی
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).