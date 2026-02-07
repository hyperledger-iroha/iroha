---
lang: uz
direction: ltr
source: docs/source/ministry/moderation_red_team_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7ecf637fa28f68a997367118163224caecc6c71c604d5e7ece409941d5374f44
source_last_modified: "2025-12-29T18:16:35.978869+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Red-Team & Chaos Drill Plan
summary: Execution plan for roadmap item MINFO-9 covering recurring adversarial campaigns, telemetry hooks, and reporting requirements.
translator: machine-google-reviewed
---

# Moderatsiya Red-Team & Chaos Drills (MINFO-9)

Yoʻl xaritasi maʼlumotnomasi: **MINFO-9 — Moderatsiya qizil-jamoasi va tartibsizlik mashqlari** (maqsadlar Q32026)

Axborot vazirligi AI moderatsiyasi quvurlari, mukofot dasturlari, shlyuzlar va boshqaruv nazoratini ta'kidlaydigan takrorlanadigan raqib kampaniyalarini o'tkazishi kerak. Ushbu reja yoʻl xaritasida qayd etilgan “🈳 Boshlanmagan” boʻshliqni toʻgʻridan-toʻgʻri mavjud moderatsiya panellari (`dashboards/grafana/ministry_moderation_overview.json`) va favqulodda vaziyatlarda ish oqimlari bilan bogʻlovchi koʻlam, kadens, mashq shablonlari va dalillar talablarini belgilash orqali yopadi.

## Maqsadlar va bajariladigan narsalar

- Har chorakda bir necha qismli kontrabandaga urinishlar, poraxo'rlik/apellyatsiyani buzish va shlyuz yon kanali tekshiruvlarini qamrab oluvchi "qizil jamoa" mashqlarini qo'shimcha tahdid sinflariga kengaytirishdan oldin o'tkazing.
- Har bir matkap uchun deterministik artefaktlarni (CLI jurnallari, Norito manifestlari, Grafana eksportlari, SoraFS CIDlar) yozib oling va ularni `docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md` orqali fayllang.
- Matkap chiqishini kalibrlash manifestlariga (`docs/examples/ai_moderation_calibration_*.json`) va rad etish siyosatiga qaytaring, shuning uchun tuzatish vazifalari kuzatilishi mumkin bo'lgan yo'l xaritasi chiptalariga aylanadi.
- Wire alert/runbook integratsiyasi, shuning uchun nosozliklar MINFO-1 asboblar paneli va Alertmanager paketlari (`dashboards/alerts/ministry_moderation_rules.yml`) bilan birga yuzaga keladi.

## Qo'llanish doirasi va bog'liqliklar

- **Sinov qilinayotgan tizimlar:** SoraFS qabul qilish/orkestrator yoʻllari, `docs/source/sorafs_ai_moderation_plan.md` da belgilangan AI moderatsiyasi, rad etish roʻyxati/Merkle ijrosi, shikoyat xazinasi vositalari va shlyuz tezligi chegaralari.
- **Talablar:** Favqulodda kanon va TTL siyosati (`docs/source/ministry/emergency_canon_policy.md`), moderatsiyani kalibrlash moslamalari va Torii soxta jabduqlar pariteti, shuning uchun tartibsizliklar paytida takrorlanadigan foydali yuklarni takrorlash mumkin.
- **Dasturdan tashqari:** SoraDNS siyosatlari, Kaigi konferensiyasi yoki vazirlikka tegishli bo‘lmagan aloqa kanallari (SNNet va DOCS-SORA dasturlari ostida alohida kuzatiladi).

## Rol va mas'uliyat

| Rol | Mas'uliyat | Asosiy egasi | Zaxira |
|------|------------------|---------------|--------|
| Burg'ulash direktori | Stsenariylar ro'yxatini tasdiqlaydi, qizil jamoalarni tayinlaydi, runbooks | da imzo chekadi Xavfsizlik vazirligi rahbari | Moderator o'rinbosari |
| Raqib hujayra | Foydali yuklarni ishlab chiqaradi, hujumlarni amalga oshiradi, dalillarni qayd qiladi | Xavfsizlik muhandisligi gildiyasi | Ko'ngilli operatorlar |
| Kuzatish qobiliyati yetakchisi | Boshqaruv panellari/ogohlantirishlarni kuzatadi, Grafana eksportlarini yozib oladi, voqea vaqt jadvallarini fayllari | SRE / Kuzatish mumkinligi TL | Qo'ng'iroq bo'yicha SRE |
| Navbatdagi moderator | Eskalatsiya oqimini boshqaradi, bekor qilish so'rovlarini tasdiqlaydi, favqulodda vaziyatlarda kanon yozuvlarini yangilaydi | Hodisa komandiri | Zaxira qo'mondoni |
| Reporting Scribe | Shablonni `docs/source/ministry/reports/moderation_red_team_template.md` ostida to'ldiradi, artefaktlarni bog'laydi, keyingi muammolarni ochadi | Docs/DevRel | Mahsulot bilan aloqa |

## Tezlik va vaqt jadvali| Bosqich | Maqsadli oyna | Asosiy tadbirlar | Artefaktlar |
|-------|---------------|----------------|-----------|
| **Reja** | T−4 hafta | `scripts/ministry/scaffold_red_team_drill.py` orqali stsenariylarni tanlang, foydali yuk moslamalarini, iskala artefaktlarini yangilang va matkaplar oynasi | Ssenariy haqida qisqacha ma'lumot, chipta kuzatuvchisi |
| **Tayyor** | T−1 hafta | Ishtirokchilarni bloklash, bosqich SoraFS/Torii qum qutilari, asboblar paneli/ogohlantirish xeshlarini muzlatish | Tayyor nazorat ro'yxati, asboblar paneli dayjestlari |
| **Bajarish** | Burg'ilash kuni (4 soat) | Raqib oqimlarini ishga tushiring, Alertmanager bildirishnomalarini to'plang, Torii/CLI izlarini yozib oling, bekor qilishni tasdiqlashni amalga oshiring | Jonli jurnal, Grafana suratlar |
| **Qayta tiklash** | T+1 kun | `artifacts/ministry/red-team/<YYYY-MM>/` va SoraFS ga bekor qilish, tozalash maʼlumotlar toʻplami, arxiv artefaktlarini qaytarish | Dalillar to'plami, manifest |
| **Hisobot** | T+1 hafta | Andozadan Markdown hisobotini nashr qilish, tuzatish chiptalarini jurnalga kiritish, roadmap/status.md yangilash | Hisobot fayli, Jira/GitHub havolalari |

Har choraklik mashqlar (mart/iyun/sentyabr/dekabr) kamida bajariladi; yuqori xavfga ega topilmalar bir xil dalillar ish oqimiga amal qiladigan vaqtinchalik ishga tushirishni keltirib chiqaradi.

## Ssenariylar kutubxonasi (boshlang'ich)

| Ssenariy | Tavsif | Muvaffaqiyat signallari | Dalil kiritishlari |
|----------|-------------|-----------------|-----------------|
| Ko'p qismli kontrabanda | Bo'laklar zanjiri vaqt o'tishi bilan AI filtrlarini chetlab o'tishga harakat qiladigan polimorfik foydali yuklarga ega SoraFS provayderlari bo'ylab tarqaldi. Orkestr diapazonini olish, moderatsiya TTLlari va rad etish ro‘yxatini ko‘paytirish mashqlari. | Kontrabanda foydalanuvchi yetkazib berishdan oldin aniqlangan; rad etuvchi delta chiqarildi; `ministry_moderation_overview` SLA doirasida yong'in haqida ogohlantiradi. | `sorafs.fetch.*` asboblar panelidagi CLI takrorlash jurnallari, bo'lak manifestlari, rad etish ro'yxati farqi, Trace ID'lari. |
| Poraxo'rlik va apellyatsiyani buzish | Yovuz niyatli moderatorlar poraxo‘rlik bilan bog‘liq bekor qilishni ma’qullashga urinishadi; g'aznachilik oqimlarini sinovdan o'tkazadi, tasdiqlashlarni bekor qiladi va audit jurnalini yuritadi. | Majburiy dalillar bilan qayd qilingan bekor qilish, xazina o'tkazmalari belgilandi, boshqaruv ovozi qayd etildi. | Norito yozuvlarni bekor qilish, `docs/source/ministry/volunteer_brief_template.md` yangilanishlari, xazina kitobi yozuvlari. |
| Gateway yon-kanal probing | Mo''tadil kontentni aniqlash uchun kesh vaqtini va TTLlarni o'lchaydigan yolg'on nashriyotlarni taqlid qiladi. SNNet-15 dan oldin CDN/shlyuzning qattiqlashishini mashq qiladi. | Rate-chegara va anomaliya asboblar paneli problarni ta'kidlaydi; admin CLI siyosatning bajarilishini ko'rsatadi; kontent oqishi yo'q. | Shlyuzga kirish jurnallari, Grafana `ministry_gateway_observability` panellarini qirib tashlash, oflayn ko'rib chiqish uchun paket izlarini (pcap) yozib oling. |

Dastlabki uchta stsenariy o'rganish kadansini tugatgandan so'ng, kelgusi iteratsiyalar `honey-payload beacons`, `AI adversarial prompt floods` va `SoraFS metadata poisoning` qo'shiladi.

## Amalga oshirishni tekshirish ro'yxati1. **Pre-Drill**
   - Runbook + stsenariy hujjati nashr etilganligini va tasdiqlanganligini tasdiqlang.
   - Snapshot asboblar paneli (`dashboards/grafana/ministry_moderation_overview.json`) va ogohlantirish qoidalari `artifacts/ministry/red-team/<YYYY-MM>/dashboards/`.
   - Hisobot + artefakt kataloglarini yaratish uchun `scripts/ministry/scaffold_red_team_drill.py` dan foydalaning, so'ngra matkap uchun tayyorlangan har qanday armatura to'plamlari uchun SHA256 dayjestlarini yozib oling.
   - Qarama-qarshi foydali yuklarni kiritishdan oldin rad etilgan Merkle ildizlari va favqulodda kanon yozuvlarini tekshiring.
2. **Matkap paytida**
   - Har bir harakatni (vaqt tamg'asi, operator, buyruq) jonli jurnalga (shahared doc yoki `docs/source/ministry/reports/tmp/<timestamp>.md`) kiriting.
   - Torii javoblari va AI moderatsiyasi qarorlarini, jumladan so'rov identifikatorlari, model nomlari va xavf ballarini yozib oling.
   - Eskalatsiya ish jarayonini mashq qiling (so'rovni bekor qilish → qo'mondonni tasdiqlash → `emergency_canon_policy` yangilanishi).
   - Kamida bitta ogohlantirishni tozalash mashqlarini va hujjatlarga javob berishning kechikishini ishga tushiring.
3. **Post-Drill**
   - Qayta belgilashlarni o'chiring, rad etish ro'yxatidagi yozuvlarni ishlab chiqarish qiymatlariga qaytaring va ogohlantirishlarning jimligini tekshiring.
   - Grafana/Alertmanager tarixi, CLI jurnallari, Norito manifestlarini eksport qiling va ularni dalillar to'plamiga biriktiring.
   - Egalari va to'lash muddati bilan fayllarni tuzatish muammolari (yuqori/o'rta/past); yakuniy hisobotga havola.

## Telemetriya, metrika va dalillar

- **Boshqaruv panellari:** `dashboards/grafana/ministry_moderation_overview.json` + kelajakdagi `ministry_red_team_heatmap.json` (to‘ldiruvchi) jonli signallarni ushlaydi. Har bir mashq uchun JSON snapshotlarini eksport qiling.
- **Ogohlantirish:** `dashboards/alerts/ministry_moderation_rules.yml` va kelgusi `ministry_red_team_rules.yml` auditlarni soddalashtirish uchun matkap identifikatori va stsenariyga havola qiluvchi izohlarni o'z ichiga olishi kerak.
- **Norito artefaktlari:** har bir mashq ishini `RedTeamDrillV1` voqealari (kelajak) sifatida kodlaydi, shuning uchun Torii/CLI eksportlari deterministik bo'lib, boshqaruv bilan bo'lishish mumkin.
- **Hisobot shabloni:** `docs/source/ministry/reports/moderation_red_team_template.md` nusxasini oling va stsenariy, ko‘rsatkichlar, dalillar dayjestlari, tuzatish holati va boshqaruv imzosini to‘ldiring.
- **Arxiv:** Artefaktlarni `artifacts/ministry/red-team/<YYYY-MM>/<scenario>/` (jurnallar, CLI chiqishi, Norito toʻplamlari, asboblar paneli eksporti)da saqlang va boshqaruv tasdiqlagandan keyin jamoatchilik koʻrib chiqishi uchun mos keladigan SoraFS CAR manifestini eʼlon qiling.

## Avtomatlashtirish va keyingi qadamlar1. `scripts/ministry/` ostida yordamchi skriptlarni foydali yuklash moslamalarini o'rnatish, rad etish ro'yxatidagi yozuvlarni almashtirish va CLI/Grafana eksportlarini yig'ish uchun amalga oshiring. (`scaffold_red_team_drill.py`, `moderation_payload_tool.py` va `check_red_team_reports.py` endi iskala, foydali yuklarni birlashtirish, rad etish roʻyxatini tuzatish va toʻldiruvchini qoʻllashni qamrab oladi; `export_red_team_evidence.py` etishmayotgan asboblar paneli/jurnalni eksport qiladi, `export_red_team_evidence.py` API00 bilan qoʻshiladi. deterministik bo'ling.)
2. Hisobotlarni birlashtirishdan oldin shablon toʻliqligi va dalillar dayjestlarini tekshirish uchun `ci/check_ministry_red_team.sh` bilan CIni kengaytiring. ✅ (`scripts/ministry/check_red_team_reports.py` barcha bajarilgan matkap hisobotlarida to'ldiruvchini olib tashlashni ta'minlaydi.)
3. `ministry_red_team_status` bo'limini `status.md` ga bo'lajak mashqlar, ochiq tuzatish elementlari va oxirgi ish ko'rsatkichlarini ko'rsatish uchun qo'shing.
4. Burg'ulash metama'lumotlarini shaffoflik quvuriga integratsiya qiling, shunda choraklik hisobotlar eng so'nggi tartibsizlik natijalariga havola qilishi mumkin.
5. Matkap hisobotlari to‘g‘ridan-to‘g‘ri `cargo xtask ministry-transparency ingest --red-team-report <path>...` ichiga kiritiladi, shuning uchun sanitarlashtirilgan choraklik ko‘rsatkichlar va boshqaruv manifestlarida mavjud daftar/apellyatsiya/inkor ro‘yxati tasmasi bilan bir qatorda matkap identifikatorlari, dalillar to‘plamlari va asboblar panelidagi SHA ko‘rsatiladi.

Bu qadamlar qo‘yilgach, MINFO-9 kuzatilishi mumkin bo‘lgan artefaktlar va o‘lchanadigan muvaffaqiyat mezonlari bilan 🈳 Boshlanmagan dan 🈺 Davom etilmoqda ga o‘tadi.