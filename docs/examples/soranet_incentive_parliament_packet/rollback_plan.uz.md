---
lang: uz
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/rollback_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 47b6ac4be21202943d4145c604557a2ee50823acc139633dd6cf690a81cbce8e
source_last_modified: "2026-01-22T14:35:37.885394+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Relayni rag'batlantirishni qaytarish rejasi

Agar boshqaruv soʻrovlari boʻlsa, avtomatik oʻtkazma toʻlovlarini oʻchirish uchun ushbu kitobdan foydalaning a
to'xtating yoki telemetriya to'siqlari yonib ketsa.

1. **Otomatlashtirishni muzlatish.** Har bir orkestr xostidagi rag‘batlantiruvchi demonni to‘xtating.
   (`systemctl stop soranet-incentives.service` yoki ekvivalent konteyner
   joylashtirish) va jarayon endi ishlamayotganligini tasdiqlang.
2. **To‘kish bo‘yicha ko‘rsatmalar kutilmoqda.** Ishga tushiring
   `iroha app sorafs incentives service daemon --state <state.json> --config <daemon.json> --metrics-dir <spool> --once`
   to'lov bo'yicha ko'rsatmalar mavjud emasligiga ishonch hosil qilish uchun. Olingan ma'lumotlarni arxivlash
   Audit uchun Norito foydali yuklar.
3. **Boshqaruvni tasdiqlashni bekor qilish.** tahrirlash `reward_config.json`, sozlash
   `"budget_approval_id": null` va konfiguratsiyani bilan qayta joylashtiring
   `iroha app sorafs incentives service init` (yoki ishlayotgan bo'lsa `update-config`)
   uzoq umr ko'rgan demon). To'lov mexanizmi endi yopilmaydi
   `MissingBudgetApprovalId`, shuning uchun demon to'lovlarni yangisigacha rad etadi
   tasdiqlash xeshi tiklandi. Git commit va SHA-256 ni yozib oling
   hodisalar jurnalida o'zgartirilgan konfiguratsiya.
4. **Sora parlamentiga xabar bering.** Toʻlangan toʻlovlar daftarini, shadow-runni ilova qiling.
   hisobot va voqeaning qisqacha xulosasi. Parlament bayonnomasi xeshni qayd etishi kerak
   bekor qilingan konfiguratsiya va demon to'xtatilgan vaqt.
5. **Orqaga qaytarish tekshiruvi.** Demonni o‘chirib qo‘ying:
   - telemetriya ogohlantirishlari (`soranet_incentives_rules.yml`) >=24 soat davomida yashil rangda,
   - g'aznachilik solishtirish hisobotida nol etishmayotgan transferlar ko'rsatilgan va
   - Parlament yangi byudjet xeshini tasdiqlaydi.

Boshqaruv byudjetni tasdiqlash xeshini qayta chiqargach, `reward_config.json` ni yangilang
yangi dayjest bilan so'nggi telemetriyada `shadow-run` buyrug'ini qayta ishga tushiring,
va rag'batlantirish demonini qayta ishga tushiring.