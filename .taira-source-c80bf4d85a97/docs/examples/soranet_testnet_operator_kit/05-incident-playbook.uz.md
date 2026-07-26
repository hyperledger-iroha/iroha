---
lang: uz
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-12-29T18:16:35.091815+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Brownout / Downgrade Response Playbook

1. **Aniqlash**
   - Ogohlantirish `soranet_privacy_circuit_events_total{kind="downgrade"}` yong'inlari yoki
     Brownout webhook boshqaruvdan kelib chiqadi.
   - 5 daqiqa ichida `kubectl logs soranet-relay` yoki tizim jurnali orqali tasdiqlang.

2. **Barqarorlash**
   - Muzlatish himoyasi aylanishi (`relay guard-rotation disable --ttl 30m`).
   - Ta'sirlangan mijozlar uchun faqat to'g'ridan-to'g'ri bekor qilishni yoqing
     (`sorafs fetch --transport-policy direct-only --write-mode read-only`).
   - Joriy muvofiqlik konfiguratsiyasi xeshini oling (`sha256sum compliance.toml`).

3. **Tashxis qo‘yish**
   - Eng so'nggi katalog snapshotini to'plang va ko'rsatkichlar to'plamini o'tkazing:
     `soranet-relay support-bundle --output /tmp/bundle.tgz`.
   - PoW navbatining chuqurligiga, gaz kelebeği hisoblagichlariga va GAR toifasidagi keskinliklarga e'tibor bering.
   - Hodisaga PQ tanqisligi, muvofiqlikni bekor qilish yoki o'rni nosozligi sabab bo'lganligini aniqlang.

4. **Eskalatsiya**
   - Boshqaruv ko'prigini (`#soranet-incident`) xulosa va to'plam xesh bilan xabardor qiling.
   - Ogohlantirish bilan bog'langan voqea chiptasini oching, shu jumladan vaqt belgilari va yumshatish bosqichlari.

5. **Qayta tiklash**
   - Asl sabab bartaraf etilgandan so'ng, aylanishni qayta yoqing
     (`relay guard-rotation enable`) va faqat to'g'ridan-to'g'ri bekor qilishni qaytaring.
   - KPIlarni 30 daqiqa davomida kuzatib borish; yangi jigarrang dog'lar paydo bo'lmasligiga ishonch hosil qiling.

6. **O'limdan keyingi**
   - Boshqaruv shablonidan foydalangan holda 48 soat ichida voqea hisobotini yuboring.
   - Agar yangi xato rejimi aniqlansa, runbook-larni yangilang.