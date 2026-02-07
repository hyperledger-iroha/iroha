---
lang: uz
direction: ltr
source: docs/source/compliance/android/and6_compliance_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a0ce1be46f9c468915f50de5e38e2f34657b26bf4243fb5ea45dab175789393
source_last_modified: "2026-01-05T09:28:12.002460+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android AND6 muvofiqligini tekshirish roʻyxati

Ushbu nazorat ro'yxati muhim bosqichga olib keladigan muvofiqlik natijalarini kuzatib boradi **VA 6 -
CI va muvofiqlikni mustahkamlash**. U so'ralgan tartibga soluvchi artefaktlarni birlashtiradi
`roadmap.md` da va ostida saqlash tartibini belgilaydi
`docs/source/compliance/android/` shuning uchun muhandislik, qo'llab-quvvatlash va huquqni chiqaring
Android versiyalarini tasdiqlashdan oldin bir xil dalillar to'plamiga murojaat qilishi mumkin.

## Qo'llanish doirasi va egalari

| Hudud | Yetkazib berish | Asosiy egasi | Zaxira / Sharhlovchi |
|------|--------------|---------------|-------------------|
| Evropa Ittifoqi tartibga solish to'plami | ETSI EN 319 401 xavfsizlik maqsadi, GDPR DPIA xulosasi, SBOM attestatsiyasi, dalillar jurnali | Muvofiqlik va qonunchilik (Sofiya Martins) | Chiqarish muhandisligi (Aleksey Morozov) |
| Yaponiya tartibga solish to'plami | FISC xavfsizlik nazorati nazorat roʻyxati, ikki tilli StrongBox attestatsiya toʻplamlari, dalillar jurnali | Muvofiqlik va qonunchilik (Daniel Park) | Android dasturi rahbari |
| Qurilma laboratoriyasining tayyorligi | Imkoniyatlarni kuzatish, favqulodda vaziyatlar tetikleyicilari, kuchayish jurnali | Uskuna laboratoriyasi rahbari | Android Observability TL |

## Artefakt matritsasi| Artefakt | Tavsif | Saqlash yo'li | Kadensni yangilash | Eslatmalar |
|----------|-------------|--------------|-----------------|-------|
| ETSI EN 319 401 xavfsizlik maqsadi | Android SDK ikkiliklari uchun xavfsizlik maqsadlari/taxminlarini tavsiflovchi hikoya. | `docs/source/compliance/android/eu/security_target.md` | Har bir GA + LTS versiyasini qayta tasdiqlang. | Chiqaruvchi poyezd uchun asos yaratish xeshlarini keltirish kerak. |
| GDPR DPIA xulosasi | Telemetriya/loggingni qamrab olgan ma'lumotlarni himoya qilish ta'sirini baholash. | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | Yillik + moddiy telemetriya o'zgarishidan oldin. | `sdk/android/telemetry_redaction.md` da havolalarni tahrirlash siyosati. |
| SBOM attestatsiyasi | Gradle/Maven artefaktlari uchun SBOM va SLSA kelib chiqishi imzolangan. | `docs/source/compliance/android/eu/sbom_attestation.md` | Har bir GA versiyasi. | CycloneDX hisobotlari, kosign to'plamlari va nazorat summalarini yaratish uchun `scripts/android_sbom_provenance.sh <version>` ni ishga tushiring. |
| FISC xavfsizlik nazorati nazorat ro'yxati | SDK boshqaruv elementlarini FISC talablariga moslashtirish toʻliq nazorat roʻyxati. | `docs/source/compliance/android/jp/fisc_controls_checklist.md` | Yillik + JP hamkor uchuvchilardan oldin. | Ikki tilli sarlavhalarni taqdim eting (EN/JP). |
| StrongBox attestatsiya to'plami (JP) | Har bir qurilma attestatsiyasi xulosasi + JP regulyatorlari uchun zanjir. | `docs/source/compliance/android/jp/strongbox_attestation.md` | Hovuzga yangi jihozlar kirganda. | `artifacts/android/attestation/<device>/` ostidagi xom artefaktlarga ishora qiling. |
| Huquqiy imzolash eslatmasi | ETSI/GDPR/FISC qamrovi, maxfiylik holati va biriktirilgan artefaktlar uchun qamoqqa olish zanjiri haqidagi maslahat xulosasi. | `docs/source/compliance/android/eu/legal_signoff_memo.md` | Har safar artefakt to'plami o'zgarganda yoki yangi yurisdiktsiya qo'shilganda. | Eslatma dalillar jurnalidagi xeshlarga havolalar va qurilma-laboratoriya favqulodda vaziyatlar to'plamiga havolalar. |
| Dalillar jurnali | Xesh/vaqt tamgʻasi metamaʼlumotlari bilan taqdim etilgan artefaktlar indeksi. | `docs/source/compliance/android/evidence_log.csv` | Yuqoridagi har qanday yozuv o'zgarganda yangilanadi. | Buildkite havolasini qo'shing + ko'rib chiquvchidan chiqish. |
| Qurilma-laboratoriya asboblari to'plami | `device_lab_instrumentation.md` da belgilangan jarayon bilan qayd qilingan uyaga xos telemetriya, navbat va attestatsiya dalillari. | `artifacts/android/device_lab/<slot>/` (qarang: `docs/source/compliance/android/device_lab_instrumentation.md`) | Har bir ajratilgan slot + o'chirish matkap. | SHA-256 manifestlarini yozib oling va dalillar jurnali + nazorat ro'yxatidagi slot identifikatoriga havola qiling. |
| Qurilma laboratoriyasini bron qilish jurnali | Muzlatish vaqtida StrongBox hovuzlarini ≥80% ushlab turish uchun bandlash ish jarayoni, tasdiqlashlar, sig‘im snapshotlari va eskalatsiya narvonlaridan foydalaniladi. | `docs/source/compliance/android/device_lab_reservation.md` | Rezervasyonlar yaratilgan/o'zgartirilganda yangilang. | Jarayonda qayd etilgan `_android-device-lab` chipta identifikatorlari va haftalik taqvim eksportiga havola qiling. |
| Device-laborover failover runbook & drill bundle | Har chorakda mashq qilish rejasi va artefakt manifest bo'lib, orqaga qaytish yo'llari, Firebase portlash navbati va tashqi StrongBox saqlovchi tayyorligini ko'rsatadi. | `docs/source/compliance/android/device_lab_failover_runbook.md` + `artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` | Har chorakda (yoki apparat ro'yxati o'zgargandan keyin). | Dalillar jurnaliga matkap identifikatorlarini kiriting va runbookda qayd etilgan manifest xesh + PagerDuty eksportini ilova qiling. |

> **Maslahat:** PDF-fayllarni yoki tashqi imzolangan artefaktlarni biriktirganda, qisqacha saqlang
> O'zgarmas artefakt bilan bog'langan jadval yo'lidagi Markdown o'rami
> boshqaruv ulushi. Bu saqlagan holda reponing engil vaznini saqlaydi
> audit izi.

## Yevropa Ittifoqi tartibga solish paketi (ETSI/GDPR)Evropa Ittifoqi paketi yuqoridagi uchta artefaktni va qonuniy eslatmani birlashtiradi:

- `security_target.md` ni reliz identifikatori, Torii manifest xeshi bilan yangilang,
  va SBOM dayjesti, shuning uchun auditorlar ikkilik fayllarni e'lon qilingan hajmga moslashtirishi mumkin.
- DPIA xulosasini eng soʻnggi telemetriyani tahrirlash siyosatiga muvofiq saqlang va
  `docs/source/sdk/android/telemetry_redaction.md` da havola qilingan Norito farq parchasini ilova qiling.
- SBOM attestatsiyasi yozuvi quyidagilarni o'z ichiga olishi kerak: CycloneDX JSON xeshi, kelib chiqishi
  bundle xesh, kosign bayonoti va ularni yaratgan Buildkite ish URL manzili.
- `legal_signoff_memo.md` maslahat/sanani yozib olishi, har bir artefaktni sanab o'tishi kerak +
  SHA-256, har qanday kompensatsiya boshqaruvlarini belgilang va dalillar jurnali qatoriga havola qiling
  shuningdek, tasdiqlashni kuzatgan PagerDuty chipta identifikatori.

## Yaponiya tartibga solish paketi (FISC/StrongBox)

Yaponiya regulyatorlari ikki tilli hujjatlar bilan parallel to'plamni kutishmoqda:

- `fisc_controls_checklist.md` rasmiy jadvalni aks ettiradi; ikkalasini ham to'ldiring
  EN va JA ustunlari va `sdk/android/security.md` ning maxsus bo'limiga murojaat qiling
  yoki har bir nazoratni qondiradigan StrongBox attestatsiya to'plami.
- `strongbox_attestation.md` ning so'nggi ishlarini umumlashtiradi
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  (har bir qurilma uchun JSON + Norito konvertlari). O'zgarmas artefaktlarga havolalarni joylashtiring
  `artifacts/android/attestation/<device>/` ostida va aylanish kadansiga e'tibor bering.
- Ichkarida taqdim etilgan hujjatlar bilan birga yuboriladigan ikki tilli muqova maktubi shablonini yozib oling
  `docs/source/compliance/android/jp/README.md`, shuning uchun Yordam uni qayta ishlatishi mumkin.
- Dalillar jurnalini nazorat ro'yxatiga havola qiluvchi bitta qator bilan yangilang
  attestatsiya toʻplami xesh va yetkazib berishga bogʻlangan har qanday JP hamkor chipta identifikatorlari.

## Yuborish ish jarayoni

1. **Qoralama** - Egasi artefaktni tayyorlaydi, rejalashtirilgan fayl nomini yozib oladi
   yuqoridagi jadvalga o'ting va yangilangan Markdown stub va a ni o'z ichiga olgan PRni ochadi
   tashqi biriktirmaning nazorat summasi.
2. **Ko'rib chiqish** - Release Engineering kelib chiqish xeshlari bosqichli bilan mos kelishini tasdiqlaydi
   ikkilik; Muvofiqlik tartibga soluvchi tilni tasdiqlaydi; Qo'llab-quvvatlash SLA va
   telemetriya siyosatiga to'g'ri havola qilingan.
3. **Ro‘yxatdan o‘tish** - Tasdiqlovchilar o‘z nomlari va sanalarini `Sign-off` jadvaliga qo‘shadilar.
   quyida. Dalillar jurnali PR URL va Buildkite ishga tushirilishi bilan yangilanadi.
4. **Publish** - SRE boshqaruvi imzolangandan keyin artefaktni ulang
   `status.md` va Android Support Playbook havolalarini yangilang.

### Ro'yxatdan o'tish jurnali

| Artefakt | Ko'rib chiqqan | Sana | PR / Dalil |
|----------|-------------|------|---------------|
| *(kutishda)* | - | - | - |

## Qurilma laboratoriyasini band qilish va favqulodda vaziyatlar rejasi

Yo'l xaritasida **qurilma laboratoriyasining mavjudligi** xavfini kamaytirish uchun:- `docs/source/compliance/android/evidence_log.csv` da haftalik quvvatni kuzatib boring
  (`device_lab_capacity_pct` ustuni). Mavjud bo'lsa Alert Release Engineering
  ketma-ket ikki hafta davomida 70% dan pastga tushadi.
- Quyidagi StrongBox/umumiy yo'laklarni zaxiralang
  `docs/source/compliance/android/device_lab_reservation.md` hammadan oldinda
  muzlatish, mashq qilish yoki muvofiqlikni tekshirish, shuning uchun so'rovlar, tasdiqlashlar va artefaktlar
  `_android-device-lab` navbatda olingan. Olingan chipta identifikatorlarini bog'lang
  sig'imli suratlarni yozib olishda dalillar jurnalida.
- **Orqaga pullar:** avval umumiy Pixel hovuziga kirish; hali to'yingan bo'lsa,
  CI tekshiruvi uchun Firebase Test Lab tutunini rejalashtirish.
- **Tashqi laboratoriya ushlagichi:** ushlagichni StrongBox hamkori bilan saqlang
  lab., shuning uchun biz derazalarni muzlatish paytida uskunani zahiralashimiz mumkin (kamida 7 kunlik muddat).
- **Eskalatsiya:** PagerDuty-da `AND6-device-lab` hodisasini ko'targanda, ikkalasi ham
  asosiy va zaxira hovuzlar sig'imi 50% dan pastga tushadi. Uskuna laboratoriyasi rahbari
  qurilmalarni qaytadan ustuvorlashtirish uchun SRE bilan muvofiqlashtiradi.
- **Muvaffaqiyatsizlik dalillari to'plamlari:** har bir mashqni ostida saqlang
  `artifacts/android/device_lab_contingency/<YYYYMMDD>/` bandlov bilan
  so'rov, PagerDuty eksporti, apparat manifesti va tiklash transkripti. Malumot
  `device_lab_contingency.md` to'plamini oling va SHA-256 ni dalillar jurnaliga qo'shing
  Shunday qilib, yuridik favqulodda ish jarayoni amalga oshirilganligini isbotlashi mumkin.
- **Har choraklik mashqlar:** Runbookda mashq qiling
  `docs/source/compliance/android/device_lab_failover_runbook.md`, ilova qiling
  natijada to'plam yo'li + `_android-device-lab` chiptasiga manifest xesh va
  matkap identifikatorini favqulodda vaziyatlar jurnalida va dalillar jurnalida aks ettiring.

Favqulodda vaziyatlar rejasining har bir faollashuvini hujjatlashtiring
`docs/source/compliance/android/device_lab_contingency.md` (sana, shu jumladan,
tetik, harakatlar va kuzatuvlar).

## Statik-tahlil prototipi

- `make android-lint`, `ci/check_android_javac_lint.sh`, kompilyatsiya
  `java/iroha_android` va umumiy `java/norito_java` manbalari
  `javac --release 21 -Xlint:all -Werror` (belgilangan toifalar bilan
- Kompilyatsiyadan so'ng, skript AND6 qaramlik siyosatini amalga oshiradi
  `jdeps --summary`, agar modul tasdiqlangan ruxsat roʻyxatidan tashqarida boʻlsa, muvaffaqiyatsiz
  (`java.base`, `java.net.http`, `jdk.httpserver`) paydo bo'ladi. Bu saqlaydi
  Android yuzasi SDK kengashining "yashirin JDK bog'liqligi yo'q" bilan moslangan
  StrongBox muvofiqligini tekshirishdan oldin talab.
- CI endi xuddi shu darvoza orqali ishlaydi
  `.github/workflows/android-lint.yml`, chaqiradi
  `ci/check_android_javac_lint.sh` Android-ga tegadigan har bir surish/PRda yoki
  birgalikda Norito Java manbalari va yuklashlar `artifacts/android/lint/jdeps-summary.txt`
  shuning uchun muvofiqlik tekshiruvlari imzolangan modullar ro'yxatiga qayta ishga tushirmasdan murojaat qilishi mumkin
  mahalliy skript.
- Vaqtinchalik saqlash kerak bo'lganda `ANDROID_LINT_KEEP_WORKDIR=1` ni o'rnating
  ish maydoni. Skript allaqachon yaratilgan modul xulosasini ko'chiradi
  `artifacts/android/lint/jdeps-summary.txt`; o'rnatish
  `ANDROID_LINT_SUMMARY_OUT=docs/source/compliance/android/evidence/android_lint_jdeps.txt`
  (yoki shunga o'xshash) tekshiruvlar uchun qo'shimcha versiyali artefakt kerak bo'lganda.
  Muhandislar Android PR-larini yuborishdan oldin buyruqni mahalliy sifatida ishlatishlari kerak
  Java manbalariga teginish va yozilgan xulosani/jurnalni muvofiqlikka biriktirish
  sharhlar. Uni nashr yozuvlaridan “Android javac lint + qaramlik” deb havola qiling
  skanerlash”.

## CI dalillari (Lint, testlar, attestatsiya)- `.github/workflows/android-and6.yml` endi barcha AND6 eshiklarini ishga tushiradi (javac lint +
  qaramlikni skanerlash, Android test to'plami, StrongBox attestatsiya tekshiruvi va
  qurilma-laboratoriya uyasi tekshiruvi) Android yuzasiga tegilgan har bir PR/surishda.
- `ci/run_android_tests.sh` `ci/run_android_tests.sh`ni oʻrab, chiqaradi
  esa `artifacts/android/tests/test-summary.json` da deterministik xulosa
  konsol jurnalini `artifacts/android/tests/test.log` ga saqlash. Ikkalasini biriktiring
  CI ishga tushirilganda moslik paketlariga fayllar.
- `scripts/android_strongbox_attestation_ci.sh --summary-out` ishlab chiqaradi
  `artifacts/android/attestation/ci-summary.json`, to'plamni tasdiqlaydi
  StrongBox uchun `artifacts/android/attestation/**` ostida attestatsiya zanjirlari va
  TEE basseynlari.
- `scripts/check_android_device_lab_slot.py --root fixtures/android/device_lab`
  CI da ishlatiladigan namuna uyasi (`slot-sample/`)ni tekshiradi va unga ishora qilish mumkin
  bilan `artifacts/android/device_lab/<slot-id>/` ostida real ishlaydi
  `--require-slot --json-out <dest>` asboblar to'plamlarini isbotlash uchun amal qiladi
  hujjatlashtirilgan tartib. CI tasdiqlash xulosasini yozadi
  `artifacts/android/device_lab/summary.json`; namuna uyasi o'z ichiga oladi
  to'ldiruvchi telemetriya/attestatsiya/navbat/jurnal ko'chirmalari va qayd qilingan
  Qayta tiklanadigan xeshlar uchun `sha256sum.txt`.

## Device-Lab Instrumentation Workflow

Har bir band qilish yoki o'z-o'zidan o'tmaslik repetisiyasiga rioya qilish kerak
`device_lab_instrumentation.md` ko'rsatmasi telemetriya, navbat va attestatsiya
artefaktlar bron qilish jurnaliga to'g'ri keladi:

1. **Urug' uyasi artefaktlari.** Yaratish
   `artifacts/android/device_lab/<slot>/` standart pastki papkalar bilan va ishga tushiring
   Slot yopilgandan so'ng `shasum` (yangi sahifaning “Artifakt tartibi” bo'limiga qarang.
   qo'llanma).
2. **Asboblar buyruqlarini ishga tushiring.** Telemetriya/navbatni yozib olishni bajaring,
   dayjest, StrongBox jabduqlar va lint/qaramlik skanerini xuddi shunday bekor qiling
   hujjatlashtirilgan, shuning uchun chiqishlar CIni aks ettiradi.
3. **Fayl dalillari.** Yangilash
   `docs/source/compliance/android/evidence_log.csv` va bron chiptasi
   slot identifikatori, SHA-256 manifest yo'li va mos keladigan asboblar paneli/Buildkite bilan
   havolalar.

Artefakt papkasini va xesh-manifestni AND6 relizlar paketiga biriktiring
ta'sirlangan muzlatish oynasi. Boshqaruvni ko'rib chiquvchilar nazorat ro'yxatlarini rad etadi
uyasi identifikatorini va asbob qo'llanmasini keltirmang.

### Rezervatsiya va to'xtatib turishga tayyorlik dalillari

Yoʻl xaritasining “Regulyativ artefaktlarni tasdiqlash va laboratoriya holatlari” bandi koʻproq narsani talab qiladi
asboblardan ko'ra. Har bir AND6 paketi proaktivga ham murojaat qilishi kerak
band qilish ish jarayoni va har chorakda takroriy takroriy takrorlash:- **Rezervatsiya kitobi (`device_lab_reservation.md`).** Bandlovni kuzatib boring
  jadval (etkazib berish vaqtlari, egalari, uyalar uzunligi), birgalikda taqvimni orqali eksport qiling
  `scripts/android_device_lab_export.py` va `_android-device-lab` yozib oling
  chipta identifikatorlari `evidence_log.csv` da sig'im suratlari bilan birga. O'yin kitobi
  eskalatsiya zinapoyasi va favqulodda vaziyatlar tetikleyicilarini talaffuz qiladi; bu tafsilotlarni nusxalash
  Rezervasyonlar o'zgarganda yoki sig'imdan pastga tushganda nazorat ro'yxatiga kiriting
  80% yo'l xaritasi maqsadi.
- **Failover drill runbook (`device_lab_failover_runbook.md`).** Buni bajaring
  Har chorakda bir marta mashq qilish (to'xtashni taqlid qilish → qo'shimcha yo'laklarni targ'ib qilish → jalb qilish
  Firebase portlashi + tashqi StrongBox hamkori) va artefaktlarni ostida saqlang
  `artifacts/android/device_lab_contingency/<drill-id>/`. Har bir to'plam bo'lishi kerak
  Manifestni, PagerDuty eksportini, Buildkite ishga tushirish havolalarini, Firebase yorilishini o'z ichiga oladi
  hisobot va ish kitobida qayd etilgan saqlovchining tasdig'i. ga murojaat qiling
  matkap identifikatori, SHA-256 manifest va dalillar jurnalida va kuzatuv chiptasi
  ushbu nazorat ro'yxati.

Ushbu hujjatlar birgalikda qurilma sig'imini rejalashtirish, uzilish mashqlari,
va asboblar to'plamlari talab qilgan bir xil tekshirilgan izga ega
yo'l xaritasi va huquqiy sharhlovchilar.

## Ko'rib chiqish tezligi

- **Har chorakda** - EI/JP artefaktlarining yangilanganligini tekshirish; yangilash
  dalillar jurnali xeshlari; kelib chiqishini olish uchun mashq qiling.
- **Relizdan oldingi** - Ushbu nazorat ro'yxatini har bir GA/LTS kesish paytida ishga tushiring va biriktiring
  tugallangan jurnalni chiqarish RFC ga.
- **Hodisadan keyingi** - Agar Sev 1/2 hodisasi telemetriya, imzolash yoki tegsa
  attestatsiya, tegishli artefakt stublarini tuzatish yozuvlari bilan yangilash va
  dalillar jurnaliga havolani yozib oling.