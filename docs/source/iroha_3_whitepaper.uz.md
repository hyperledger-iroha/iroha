---
lang: uz
direction: ltr
source: docs/source/iroha_3_whitepaper.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 07e149429887b0dfc38cf0619552cbefcbae4dd1ec9fe9e9d47a05371ed08f29
source_last_modified: "2025-12-29T18:16:35.968351+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v3.0 (Nexus oldindan ko'rish)

Ushbu hujjat ko'p qatorli yo'nalishga qaratilgan istiqbolli Hyperledger Iroha v3 arxitekturasini qamrab oladi.
quvur liniyasi, Nexus ma'lumotlar bo'shliqlari va Asset Exchange Toolkit (AXT). U Iroha v2 oq qog'ozini to'ldiradi
faol rivojlanayotgan kelajakdagi imkoniyatlarni tavsiflash.

---

## 1. Umumiy ko'rinish

Iroha v3 v2 ning deterministik asosini gorizontal miqyosda va kengroq oʻzaro faoliyat domen bilan kengaytiradi.
ish oqimlari. **Nexus** kod nomi bilan chiqarilgan nashr quyidagilarni taqdim etadi:

- **SORA Nexus** nomli yagona, global umumiy tarmoq. Ushbu universalda barcha Iroha v3 tengdoshlari ishtirok etadi
  izolyatsiyalangan joylashtirishlarni ishlatishdan ko'ra kitob. Tashkilotlar o'zlarining ma'lumotlar maydonlarini ro'yxatdan o'tkazish orqali qo'shilishadi,
  umumiy kitobga biriktirilganda siyosat va maxfiylik uchun izolyatsiya qilingan.
- Umumiy kod bazasi: bir xil ombor Iroha v2 (oʻz-oʻzidan joylashtirilgan tarmoqlar) va Iroha v3 (SORA Nexus) quradi.
  Konfiguratsiya maqsadli rejimni tanlaydi, shuning uchun operatorlar dasturiy ta'minotni almashtirmasdan Nexus xususiyatlarini qabul qilishlari mumkin.
  steklar. Iroha virtual mashinasi (IVM) ikkala versiyada ham bir xil, shuning uchun Kotodama shartnomalar va bayt kod
  artefaktlar o'z-o'zidan joylashtirilgan tarmoqlarda va global Nexus kitobida muammosiz ishlaydi.
- Mustaqil ish yuklarini parallel ravishda qayta ishlash uchun ko'p qatorli bloklarni ishlab chiqarish.
- Zanjirli langarlar orqali birlashtirilishi mumkin bo'lgan holda ijro muhitlarini izolyatsiya qiluvchi ma'lumotlar bo'shliqlari (DS).
- Asset Exchange Toolkit (AXT).
- Ishonchli Broadcast Commit (RBC) yo'llari, deterministik muddatlar va dalillar orqali ishonchlilikni oshirdi
  namunaviy byudjetlar.

Bu xususiyatlar faol rivojlanishda qolmoqda; API va tartiblar v3 generalidan oldin rivojlanishi mumkin
mavjudligining muhim bosqichi. `nexus.md`, `nexus_transition_notes.md` va `new_pipeline.md` ga qarang.
muhandislik darajasidagi tafsilotlar.

## 2. Ko'p qatorli arxitektura

- **Rejalashtiruvchi:** Nexus rejalashtiruvchi bo'limlari ma'lumotlar maydoni identifikatorlari asosida qatorlarga bo'linadi va
  kompozitsion guruhlar. Yo'llar parallel ravishda bajariladi, shu bilan birga deterministik tartib kafolatlarini saqlab qoladi
  har bir qator.
- **Line guruhlari:** Tegishli maʼlumotlar boʻshliqlari `LaneGroupId` ga ega boʻlib, ish oqimlari uchun muvofiqlashtirilgan bajarilishini taʼminlaydi.
  bir nechta komponentlarni qamrab oladi (masalan, CBDC DS va uning to'lov dApp DS).
- **Muddatlari:** Har bir qator kafolat berish uchun deterministik muddatlarni (blok, isbot, ma'lumotlar mavjudligi) kuzatib boradi.
  taraqqiyot va cheklangan resurslardan foydalanish.
- **Telemetriya:** Yoʻlak darajasidagi koʻrsatkichlar oʻtkazish qobiliyati, navbat chuqurligi, oxirgi muddat buzilishi va tarmoqli kengligidan foydalanishni koʻrsatadi.
  CI skriptlari boshqaruv panelini rejalashtiruvchi bilan moslashtirish uchun ushbu hisoblagichlarning mavjudligini tasdiqlaydi.

## 3. Maʼlumotlar boʻshliqlari (Nexus)- **Izolyatsiya:** Har bir maʼlumot maydoni oʻzining konsensus yoʻlagi, dunyo holati segmenti va Kura xotirasini saqlaydi. Bu
  global SORA Nexus daftarini langarlar orqali izchil ushlab turish bilan birga maxfiylik domenlarini qo'llab-quvvatlaydi.
- **Anchorlar:** Muntazam topshiriqlar DS holatini umumlashtiruvchi langar artefaktlarini ishlab chiqaradi (Merkle ildizlari, dalillar,
  majburiyatlar) va ularni audit uchun global tarmoqqa e'lon qiling.
- **Line guruhlari va birlashtirilishi:** Ma'lumotlar bo'shliqlari atomik AXTga ruxsat beruvchi birikma guruhlarini e'lon qilishi mumkin
  tasdiqlangan ishtirokchilar o'rtasidagi operatsiyalar. Boshqaruv aʼzolik oʻzgarishlari va faollashuv davrlarini nazorat qiladi.
- **Oʻchirish kodli xotira:** Kura va WSV oniy suratlari maʼlumotlarni masshtablash uchun `(k, m)` oʻchirish kodlash parametrlarini qabul qiladi.
  determinizmdan voz kechmasdan mavjudligi. Qayta tiklash tartib-qoidalari etishmayotgan qismlarni deterministik tarzda tiklaydi.

## 4. Asset Exchange Toolkit (AXT)

- **Deskriptor va bog'lash:** Mijozlar deterministik AXT deskriptorlarini tuzadilar. `axt_binding` xesh ankerlari
  individual konvertlarga tavsiflovchilar, takrorlashni oldini olish va konsensus ishtirokchilarining bayt-for-
  bayt Norito foydali yuklar.
- **Tizimlar:** IVM `AXT_BEGIN`, `AXT_TOUCH` va `AXT_COMMIT` tizim chaqiruvlarini ochib beradi. Shartnomalar ularni e'lon qiladi
  har bir ma'lumot maydoni uchun o'qish/yozish to'plamlari, xostga qatorlar bo'ylab atomiklikni ta'minlashga imkon beradi.
- ** Tutqichlar va davrlar:** Hamyonlar `(dataspace_id, epoch_id, sub_nonce)` ga bog'langan qobiliyat tutqichlarini oladi.
  Bir vaqtning o'zida konflikt deterministik tarzda foydalanadi, cheklovlar mavjud bo'lganda kanonik `AxtTrap` kodlarini qaytaradi.
  buzilgan.
- **Siyosat ijrosi:** Asosiy xostlar endi AXT siyosatining suratlarini WSV-dagi Space Directory manifestlaridan oladi,
  manifest ildizi, maqsadli chiziq, faollashtirish davri, sub-nonce va amal qilish muddatini tekshirish (`current_slot >= expiry_slot`)
  abortlar) hatto minimal test xostlarida ham. Siyosatlar maʼlumotlar maydoni identifikatori bilan kalitlanadi va tarmoqli katalogidan tuzilgan
  tutqichlar chiqish chizig'idan chiqa olmaydi yoki eskirgan manifestlardan foydalana olmaydi.
  - Rad etish sabablari deterministikdir: noma'lum ma'lumotlar maydoni, manifest ildiz mos kelmasligi, maqsadli chiziqning mos kelmasligi,
    Manifest faollashuvi ostidagi handle_era, siyosat tagida sub_nonce, muddati tugagan tutqich, tegilmagan
    dastagi ma'lumotlar maydoni yoki kerak bo'lganda dalil etishmayapti.
- **Tasdiqlar va muddatlar:** Faol oyna D davomida validatorlar dalillarni, ma'lumotlar mavjudligi namunalarini,
  va namoyon bo'ladi. Belgilangan muddatlarga rioya qilmaslik mijozning qayta urinishlari bo'yicha ko'rsatma bilan AXTni qat'iy ravishda bekor qiladi.
- **Boshqaruv integratsiyasi:** Siyosat modullari AXTda qaysi maʼlumotlar boʻshliqlari ishtirok etishi mumkinligini belgilaydi, tezlik chegarasi
  majburiyatlarni, bekor qiluvchilarni va voqealar jurnallarini qamrab oluvchi auditorlik manifestlarini boshqaradi va nashr eting.

## 5. Ishonchli Broadcast Compmit (RBC) yo'llari- **Telefonga xos DA:** RBC yo‘laklari qatorlar guruhlarini aks ettiradi, bu har bir ko‘p qatorli quvur liniyasida maxsus ma’lumotlarga ega bo‘lishini ta’minlaydi.
  mavjudligi kafolatlari.
- **Tam olish byudjetlari:** Tasdiqlovchilar dalillarni tasdiqlash uchun deterministik namuna olish qoidalariga (`q_in_slot_per_ds`) amal qiladilar.
  va guvohlik materiallari markaziy muvofiqlashtirishsiz.
- ** Orqa bosim haqidagi ma'lumotlar:** Sumeragi yurak stimulyatori hodisalari to'xtab qolgan yo'llarni tashxislash uchun RBC statistikasi bilan bog'liq.
  (qarang: `scripts/sumeragi_backpressure_log_scraper.py`).

## 6. Operatsiyalar va migratsiya

- **O‘tish rejasi:** `nexus_transition_notes.md` bir qatorli (Iroha v2) dan bosqichma-bosqich ko‘chirishni belgilaydi.
  ko'p qatorli (Iroha v3), shu jumladan telemetriya bosqichlari, konfiguratsiyalar va genezis yangilanishlari.
- **Universal tarmoq:** SORA Nexus tengdoshlari umumiy genezis va boshqaruv stackini boshqaradi. Bortda yangi operatorlar
  ma'lumotlar maydonini (DS) yaratish va mustaqil tarmoqlarni ishga tushirish o'rniga Nexus qabul qilish siyosatini qondirish.
- **Konfiguratsiya:** Yangi konfiguratsiya tugmalari qator byudjetlari, isbotlash muddatlari, AXT kvotalari va maʼlumotlar maydoni metamaʼlumotlarini qamrab oladi.
  Operatorlar Nexus rejimiga o'tguncha birlamchi sozlamalar konservativ bo'lib qoladi.
- **Test:** Oltin testlar AXT deskriptorlari, chiziqli manifestlar va tizim ro'yxatlarini qamrab oladi. Integratsiya testlari
  (`integration_tests/tests/repo.rs`, `crates/ivm/tests/axt_host_flow.rs`) oxirigacha oqimlarni mashq qiling.
- **Asboblar:** `kagami` Nexus-dan xabardor genezis generatsiyasiga erishadi va asboblar paneli skriptlari yo‘lak o‘tkazuvchanligini tasdiqlaydi,
  isbotlangan byudjetlar va RBC salomatligi.

## 7. Yo'l xaritasi

- **1-bosqich:** Mahalliy AXT qo‘llab-quvvatlashi va auditi bilan bitta domenli ko‘p qatorli bajarilishini yoqing.
- **2-bosqich:** Ruxsat berilgan domenlararo AXT uchun birlashtirish guruhlarini faollashtiring va telemetriya qamrovini kengaytiring.
- **3-bosqich:** Toʻliq Nexus maʼlumotlar fazosi federatsiyasini, oʻchirish kodli xotirasini va ilgʻor isbot almashishni ishga tushiring.

Status yangilanishlari `roadmap.md` va `status.md` da ishlaydi. Nexus dizayniga mos keladigan hissa qo'shilishi kerak
v3 uchun belgilangan deterministik ijro va boshqaruv siyosati.