---
lang: uz
direction: ltr
source: docs/fraud_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4ac4c98cc4aa6ab0c34e58e6428d0ee33eb9a0c3fdad9e6958bdc75f2a48dc66
source_last_modified: "2026-01-22T16:26:46.488648+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Firibgarlikni boshqarish bo'yicha qo'llanma

Ushbu hujjatda PSP firibgarlik to'plami uchun zarur bo'lgan iskala jamlangan
to'liq mikroservislar va SDK faol ishlab chiqilmoqda. Tutib oladi
analitik, auditor ish oqimlari va qayta tiklash protseduralari uchun taxminlar
kelgusi ilovalar buxgalteriya kitobiga xavfsiz tarzda ulanishi mumkin.

## Xizmatlar haqida umumiy ma'lumot

1. **API Gateway** – sinxron `RiskQuery` foydali yuklarni qabul qiladi, ularni boshqa joyga yuboradi.
   xususiyatlarni birlashtirish va `FraudAssessment` javoblarini buxgalteriya daftariga qaytaradi
   oqadi. Yuqori mavjudlik (faol-faol) talab qilinadi; bilan mintaqaviy juftliklardan foydalaning
   so'rovning burilishini oldini olish uchun deterministik xeshlash.
2. **Feature Aggregation** – ball olish uchun xususiyat vektorlarini tuzadi. Emit
   Faqat `FeatureInput` xeshlari; sezgir foydali yuklar zanjirdan tashqarida qoladi. Kuzatish qobiliyati
   kechikish gistogrammalarini, navbat chuqurligi oʻlchagichlarini va takroriy hisoblagichlarni nashr etishi kerak.
   ijarachi.
3. **Risk Engine** – qoidalar/modellarni baholaydi va deterministik ishlab chiqaradi
   `FraudAssessment` chiqishlari. Qoidalarni bajarish tartibi barqaror va qo'lga kiritilishiga ishonch hosil qiling
   har bir baholash ID uchun audit jurnallari.

## Tahlil va modelni reklama qilish

- **Anomaliyani aniqlash**: og'ishlarni belgilovchi oqim ishini saqlang
  har bir ijarachi uchun qaror stavkalari. Boshqaruv boshqaruv paneli va doʻkonga ogohlantirishlarni yuboring
  choraklik sharhlar uchun xulosalar.
- **Grafik tahlili**: relyatsion eksport bo‘yicha tungi grafik o‘tishlarini amalga oshirish
  kelishuv klasterlarini aniqlang. Topilmalar orqali boshqaruv portaliga eksport qiling
  `GovernanceExport` tasdiqlovchi dalillarga havolalar bilan.
- **Fikr-mulohazalarni qabul qilish**: qo‘lda ko‘rib chiqish natijalari va to‘lovni qaytarish hisobotlarini tuzing.
  Ularni xususiyat deltalariga aylantiring va ularni o'quv ma'lumotlar to'plamiga kiriting.
  Xavfli guruh toʻxtab qolgan tasmalarni aniqlashi uchun qabul qilish holati koʻrsatkichlarini eʼlon qiling.
- **Modelni ilgari surish quvuri**: nomzodlarni baholashni avtomatlashtirish (oflayn ko'rsatkichlar,
  kanareyka balli, orqaga qaytishga tayyorlik). Aktsiyalar imzolangan bo'lishi kerak
  `FraudAssessment` namunasini o'rnating va `model_version` maydonini yangilang
  `GovernanceExport`.

## Auditor ish jarayoni

1. Eng oxirgi `GovernanceExport` suratini oling va `policy_digest` mosligini tekshiring
   xavf guruhi tomonidan taqdim etilgan manifest.
2. Qoidalar yig'indilari buxgalteriya hisobi bo'yicha qarorlar yig'indisi bilan mos kelishini tasdiqlang
   namunali oyna.
3. Anomaliyalarni aniqlash va grafik tahlil hisobotlarini ko'rib chiqing
   masalalar. Hujjatning kuchayishi va kutilayotgan tuzatish egalari.
4. Ko'rib chiqish ro'yxatini imzolang va arxivlang. Norito kodlangan artefaktlarni saqlang
   takror ishlab chiqarish uchun boshqaruv portali.

## Qayta o'yin kitoblari

- **Dvigatelning uzilishi**: agar xavf mexanizmi 60 soniyadan ko'proq vaqt davomida ishlamasa,
  shlyuz faqat ko'rib chiqish rejimiga o'tishi kerak, `AssessmentDecision::Review`
  barcha so'rovlar va ogohlantiruvchi operatorlar uchun.
- **Telemetriya bo'shlig'i**: ko'rsatkichlar yoki izlar ortda qolsa (5 daqiqaga etishmayotgan),
  Avtomatik model aktsiyalarini to'xtating va qo'ng'iroq bo'yicha muhandisga xabar bering.
- **Regressiya modeli**: agar o'rnatishdan keyingi fikr-mulohazalar firibgarlikning kuchayganligini ko'rsatsa
  yo'qotishlar, oldingi imzolangan modellar to'plamiga qayting va yo'l xaritasini yangilang
  tuzatuvchi harakatlar bilan.

## Ma'lumot almashish shartnomalari

- Saqlash, shifrlash va o'z ichiga olgan yurisdiktsiyaga oid qo'shimchalarni saqlang
  SLAlarni buzish bildirishnomasi. Hamkorlar qabul qilishdan oldin ilovani imzolashlari kerak
  `FraudAssessment` eksport qiladi.
- Har bir integratsiya uchun ma'lumotlarni minimallashtirish amaliyotini hujjatlash (masalan, xeshlash
  hisob identifikatorlari, kesilgan karta raqamlari).
- Har yili yoki me'yoriy talablar o'zgarganda shartnomalarni yangilang.

## API sxemalari

Shlyuz endi aniq JSON konvertlarini ochadi, ular birma-bir xaritaga tushadi
`crates/iroha_data_model::fraud` da joriy qilingan Norito turlari:

- **Riskni qabul qilish** – `POST /v2/fraud/query` `RiskQuery` sxemasini qabul qiladi:
  - `query_id` (`[u8; 32]`, olti burchakli kodlangan)
  - `subject` (`AccountId`, kanonik I105 harfi; ixtiyoriy `@<domain>` maslahat yoki taxallus)
  - `operation` (`RiskOperation`ga mos keladigan tegli raqam; JSON `type`
    diskriminator enum variantini aks ettiradi)
  - `related_asset` (`AssetId`, ixtiyoriy)
  - `features` (`{ key: String, value_hash: hex32 }` massivi xaritadan olingan
    `FeatureInput`)
  - `issued_at_ms` (`u64`)
  - `context` (`RiskContext`; tashuvchisi `tenant_id`, ixtiyoriy `session_id`,
    ixtiyoriy `reason`)
- **Xavf qarori** – `POST /v2/fraud/assessment` sarflaydi
  `FraudAssessment` foydali yuk (shuningdek, boshqaruv eksportida aks ettirilgan):
  - `query_id`, `engine_id`, `risk_score_bps`, `confidence_bps`,
    `decision` (`AssessmentDecision` raqam), `rule_outcomes`
    (`{ rule_id, score_delta_bps, rationale? }` massivi)
  - `generated_at_ms`
  - `signature` (ixtiyoriy base64 Norito bilan kodlangan baholashni o'rab oladi)
- **Boshqaruv eksporti** – `GET /v2/fraud/governance/export` qaytaradi
  `governance` funksiyasi yoqilganda `GovernanceExport` tuzilishi, birlashma
  faol parametrlar, so'nggi qonun, model versiyasi, siyosat dayjesti va
  `DecisionAggregate` gistogrammasi.

`crates/iroha_data_model/src/fraud/types.rs`-dagi ikki tomonlama testlar buni ta'minlaydi
sxemalar Norito kodek bilan ikkilik mos bo'lib qoladi va
`integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs` mashqlari
to'liq qabul qilish / qaror qabul qilish quvur liniyasi uchidan uchiga.

## PSP SDK havolalari

Quyidagi til stublari PSP-ga qaragan integratsiya misollarini kuzatadi:

- **Rust** – `integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs`
  `RiskQuery` metamaʼlumotlarini yaratish va tekshirish uchun `iroha` ish maydoni mijozidan foydalanadi
  qabul qilishdagi muvaffaqiyatsizliklar/muvaffaqiyatlar.
- **TypeScript** – `docs/source/governance_api.md` REST sirtini hujjatlashtiradi
  PSP demo asboblar panelida ishlatiladigan engil Torii shlyuzi tomonidan iste'mol qilinadi; the
  skriptli mijoz tutun uchun `scripts/ci/schedule_fraud_scoring.sh` da yashaydi
  matkaplar.
- **Swift & Kotlin** – mavjud SDKlar (`IrohaSwift` va
  `crates/iroha_cli/docs/multisig.md` havolalari) Torii metamaʼlumotlarini ochib beradi
  `fraud_assessment_*` maydonlarini biriktirish uchun kancalar kerak. PSP-ga xos yordamchilar
  yilda “Firibgarlik va telemetriya boshqaruvi tsikli” doirasida kuzatilgan
  `status.md` va ushbu SDK tranzaksiya tuzuvchilaridan qayta foydalaning.

Ushbu havolalar mikroservis shlyuzi bilan sinxronlashtiriladi, shuning uchun PSP
Amalga oshiruvchilar har doim eng so'nggi sxema va har biri uchun namuna kod yo'liga ega
qo'llab-quvvatlanadigan til.