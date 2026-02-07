---
lang: uz
direction: ltr
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2025-12-29T18:16:35.962003+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Ishonchli tengdoshlardan Genesis Bootstrap

Mahalliy `genesis.file`ga ega bo'lmagan Iroha tengdoshlari ishonchli tengdoshlaridan imzolangan genezis blokini olishlari mumkin.
Norito kodlangan bootstrap protokoli yordamida.

- **Protokol:** tengdoshlar almashishadi `GenesisRequest` (metamaʼlumotlar uchun `Preflight`, foydali yuk uchun `Fetch`) va
  `request_id` kalitli `GenesisResponse` ramkalar. Javob beruvchilar qatoriga zanjir identifikatori, imzolovchi pubkeyi,
  hash va ixtiyoriy o'lcham bo'yicha maslahat; foydali yuklar faqat `Fetch` va takroriy so'rov identifikatorlarida qaytariladi
  `DuplicateRequest` qabul qiling.
- **Qo'riqchilar:** javob beruvchilar ruxsat etilgan ro'yxatni (`genesis.bootstrap_allowlist` yoki ishonchli tengdoshlar) amalga oshiradilar
  to'siq), zanjir identifikatori/pubkey/xesh moslashuvi, tezlik chegaralari (`genesis.bootstrap_response_throttle`) va
  o'lcham qopqog'i (`genesis.bootstrap_max_bytes`). Ruxsat etilgan ro'yxatdan tashqari so'rovlar `NotAllowed` oladi va
  noto'g'ri kalit bilan imzolangan foydali yuklar `MismatchedPubkey` ni oladi.
- **So‘rovlar oqimi:** xotira bo‘sh va `genesis.file` o‘rnatilmaganda (va
  `genesis.bootstrap_enabled=true`), tugun ishonchli tengdoshlarni ixtiyoriy ravishda oldindan ishlaydi.
  `genesis.expected_hash`, keyin foydali yukni oladi, `validate_genesis_block` orqali imzolarni tasdiqlaydi,
  va blokni qo'llashdan oldin Kura bilan birga `genesis.bootstrap.nrt` davom etadi. Bootstrap qayta urinishlari
  honor `genesis.bootstrap_request_timeout`, `genesis.bootstrap_retry_interval` va
  `genesis.bootstrap_max_attempts`.
- **Muvaffaqiyatsizlik rejimlari:** ruxsat etilgan roʻyxat, zanjir/pubkey/xesh nomuvofiqligi, oʻlcham uchun soʻrovlar rad etiladi
  chegara buzilishi, tarif limitlari, mahalliy kelib chiqish yo‘qligi yoki takroriy so‘rov identifikatorlari. Qarama-qarshi xeshlar
  tengdoshlar bo'ylab olishni to'xtatish; mahalliy konfiguratsiyaga hech qanday javob beruvchilar/vaqt tugamaydi.
- **Operator qadamlari:** hech bo'lmaganda bitta ishonchli tengdoshning haqiqiy kelib chiqishiga ishonch hosil qiling, sozlang
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` va qayta urinish tugmalari va
  mos kelmaydigan foydali yuklarni qabul qilmaslik uchun ixtiyoriy ravishda `expected_hash` pinini qo'ying. Doimiy yuklamalar bo'lishi mumkin
  `genesis.file` ni `genesis.bootstrap.nrt` ga ko'rsatib, keyingi botlarda qayta foydalaniladi.