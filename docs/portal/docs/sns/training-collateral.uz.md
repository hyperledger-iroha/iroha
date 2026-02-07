---
lang: uz
direction: ltr
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 76fb66d0b0380ea75c7515d3c9b5f73bafc98481f66f28eed05ab5afd3509abf
source_last_modified: "2025-12-29T18:16:35.177356+00:00"
translation_last_reviewed: 2026-02-07
id: training-collateral
title: SNS Training Collateral
description: Curriculum, localization workflow, and annex evidence capture required by SN-8.
translator: machine-google-reviewed
---

> Nometall `docs/source/sns/training_collateral.md`. Brifing paytida ushbu sahifadan foydalaning
> har bir qo'shimchani ishga tushirishdan oldin ro'yxatga oluvchi, DNS, vasiy va moliya guruhlari.

## 1. Oʻquv dasturining surati

| Track | Maqsadlar | Oldindan o'qish |
|-------|------------|-----------|
| Registrator operatsiyalari | Manifestlarni yuboring, KPI asboblar panelini kuzatib boring, xatolarni oshiring. | `sns/onboarding-kit`, `sns/kpi-dashboard`. |
| DNS va shlyuz | Resolver skeletlarini qo'llang, muzlatish/orqaga qaytarishni mashq qiling. | `sorafs/gateway-dns-runbook`, toʻgʻridan-toʻgʻri rejimdagi siyosat namunalari. |
| Vasiylar va kengash | Nizolarni hal qiling, boshqaruv qo'shimchalarini yangilang, qo'shimchalarni jurnalga kiriting. | `sns/governance-playbook`, boshqaruvchi ko'rsatkich kartalari. |
| Moliya va tahlil | ARPU/ommaviy koʻrsatkichlarni yozib oling, ilova toʻplamlarini nashr eting. | `finance/settlement-iso-mapping`, KPI asboblar paneli JSON. |

### Modul oqimi

1. **M1 — KPI yoʻnalishi (30 daqiqa):** Yurish qoʻshimchasi filtrlari, eksport va qochoq
   hisoblagichlarni muzlatish. Etkazib beriladi: SHA-256 dayjestiga ega PDF/CSV suratlari.
2. **M2 — Manifestning ishlash davri (45 daqiqa):** Registrator manifestlarini yaratish va tasdiqlash,
   `scripts/sns_zonefile_skeleton.py` orqali hal qiluvchi skeletlari hosil qiling. Yetkazib beriladi:
   git diff skelet + GAR dalillarini ko'rsatadi.
3. **M3 — bahsli mashqlar (40 daqiqa):** Qo'riqchini muzlatish + apellyatsiya, qo'lga olishni taqlid qilish
   qo'riqchi CLI jurnallari `artifacts/sns/training/<suffix>/<cycle>/logs/` ostida.
4. **M4 — Ilovani suratga olish (25 daqiqa):** JSON boshqaruv panelini eksport qiling va ishga tushiring:

   ```bash
   cargo xtask sns-annex \
     --suffix <suffix> \
     --cycle <cycle> \
     --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --output docs/source/sns/reports/<suffix>/<cycle>.md \
     --regulatory-entry docs/source/sns/regulatory/<memo>.md \
     --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```

   Yetkazib beriladi: yangilangan Markdown ilovasi + normativ hujjatlar + portal memo bloklari.

## 2. Lokalizatsiya ish jarayoni

- Tillar: `ar`, `es`, `fr`, `ja`, `pt`, `ru`, I180NI05X00.
- Har bir tarjima manba fayli yonida yashaydi
  (`docs/source/sns/training_collateral.<lang>.md`). `status` + yangilang
  Yangilangandan keyin `translation_last_reviewed`.
- Har bir tilga tegishli aktivlar
  `artifacts/sns/training/<suffix>/<lang>/<cycle>/` (slaydlar/, ish kitoblari/,
  yozuvlar/, jurnallar/).
- Ingliz tilini tahrir qilgandan so'ng `python3 scripts/sync_docs_i18n.py --lang <code>` ni ishga tushiring
  manba, shuning uchun tarjimonlar yangi xeshni ko'rishadi.

### Etkazib berishni tekshirish ro'yxati

1. Mahalliylashtirilgandan keyin tarjima stubini (`status: complete`) yangilang.
2. Slaydlarni PDF formatiga eksport qiling va har bir til uchun `slides/` katalogiga yuklang.
3. Yozish ≤10min KPI yurishi; til stubidan havola.
4. `sns-training` yorlig'i ostida slayd/ishchi kitobi bo'lgan faylni boshqarish chiptasi
   dayjestlar, havolalarni yozib olish va qo'shimcha dalillar.

## 3. O'quv aktivlari

- Slayd konturi: `docs/examples/sns_training_template.md`.
- Ish kitobi shabloni: `docs/examples/sns_training_workbook.md` (har bir ishtirokchiga bittadan).
- Taklif + eslatmalar: `docs/examples/sns_training_invite_email.md`.
- Baholash shakli: `docs/examples/sns_training_eval_template.md` (javoblar
  `artifacts/sns/training/<suffix>/<cycle>/feedback/` ostida arxivlangan).

## 4. Rejalashtirish va ko'rsatkichlar

| Velosiped | Oyna | Ko'rsatkichlar | Eslatmalar |
|-------|--------|---------|-------|
| 2026-03 | KPI sharhini yuborish | Davomat %, ilova dayjest qayd etilgan | `.sora` + `.nexus` kohortlar |
| 2026-06 | Oldindan `.dao` GA | Moliyaviy tayyorgarlik ≥90% | Siyosat yangilanishini qo'shish |
| 2026-09 | Kengaytirish | Munozarali matkap <20min, ilova SLA ≤2days | SN-7 rag'batlantirishlari bilan moslash |

`docs/source/sns/reports/sns_training_feedback.md` da anonim fikr-mulohazalarni yozib oling
shuning uchun keyingi kohortlar mahalliylashtirish va laboratoriyalarni yaxshilashi mumkin.