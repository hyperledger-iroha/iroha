---
lang: uz
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/06-verification-report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf489427d0eba2beebfdefc44092730c3963cbd77e83669853f4e9681ac9fd2d
source_last_modified: "2025-12-29T18:16:35.092552+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Operatorni tekshirish hisoboti (T0 fazasi)

- Operator nomi: ______________________
- Rele deskriptor identifikatori: ______________________
- Taqdim etilgan sana (UTC): ___________________
- Aloqa uchun elektron pochta / matritsa: ___________________

### Nazorat ro'yxati xulosasi

| Element | Tugallandi (Y/N) | Eslatmalar |
|------|-----------------|-------|
| Uskuna va tarmoq tasdiqlangan | | |
| Muvofiqlik bloki qo'llaniladi | | |
| Qabul konverti tasdiqlangan | | |
| Guard aylanish tutun sinovi | | |
| Telemetriya qirib tashlandi va asboblar paneli jonli | | |
| Brownout matkap bajarildi | | |
| maqsad doirasida PoW chipta muvaffaqiyat | | |

### Ko'rsatkichlar surati

- PQ nisbati (`sorafs_orchestrator_pq_ratio`): ________
- Oxirgi 24 soat ichida pasaytirish soni: ________
- O'rtacha sxema RTT (p95): ________ ms
- PoW o'rtacha hal qilish vaqti: ________ ms

### Qo'shimchalar

Iltimos, ilova qiling:

1. Relay qo‘llab-quvvatlash to‘plami xesh (`sha256`): __________________________
2. Boshqaruv paneli skrinshotlari (PQ nisbati, sxema muvaffaqiyati, PoW gistogrammasi).
3. Imzolangan matkap to'plami (`drills-signed.json` + imzolovchi ochiq kalit olti burchakli va qo'shimchalar).
4. SNNet-10 ko'rsatkichlari hisoboti (`cargo xtask soranet-testnet-metrics --input <snapshot> --out metrics-report.json`).

### Operator imzosi

Men yuqoridagi maʼlumotlarning toʻgʻriligini va barcha kerakli qadamlar bajarilganligini tasdiqlayman
yakunlandi.

Imzo: _______________________ Sana: ___________________