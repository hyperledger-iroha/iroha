---
lang: uz
direction: ltr
source: docs/source/fraud_monitoring_system.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c8262bacbb15b83bd70c824990e4948832418b59f184bca353eee899e44f4d4
source_last_modified: "2025-12-29T18:16:35.960562+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Firibgarlik monitoringi tizimi

Ushbu hujjat asosiy daftarga hamroh bo'ladigan umumiy firibgarlik monitoringi qobiliyati uchun mos yozuvlar dizaynini qamrab oladi. Maqsad to'lov xizmatlari provayderlarini (PSP) har bir tranzaksiya uchun yuqori sifatli xavf signallari bilan ta'minlash, shu bilan birga saqlash, maxfiylik va siyosat qarorlarini hisob-kitob mexanizmidan tashqarida tayinlangan operatorlar nazorati ostida saqlashdir.

## Maqsadlar va muvaffaqiyat mezonlari
- Hisob-kitob mexanizmiga tegadigan har bir to'lov uchun real vaqt rejimida firibgarlik xavfini baholashni (<120 ms 95p, <40 ms median) taqdim eting.
- Markaziy xizmat hech qachon shaxsiy identifikator ma'lumotlarini (PII) qayta ishlamasligi va faqat taxallusli identifikatorlar va xatti-harakatlar telemetriyasini qabul qilishini ta'minlash orqali foydalanuvchi maxfiyligini saqlang.
- Har bir provayder operatsion avtonomiyani saqlaydigan, lekin umumiy ma'lumotni so'rashi mumkin bo'lgan ko'p PSP muhitlarini qo'llab-quvvatlang.
- Deterministik bo'lmagan daftar xatti-harakatlarini kiritmasdan, nazorat qilinadigan va nazoratsiz modellar orqali yangi hujum shakllariga doimiy ravishda moslashtiring.
- Nozik hamyonlar yoki kontragentlarni oshkor qilmasdan tartibga soluvchilar va mustaqil sharhlovchilar uchun tekshiriladigan qaror izlarini taqdim eting.

## Qo'llash doirasi
- **Ko'lamda:** Tranzaksiya xavfini baholash, xatti-harakatlar tahlili, o'zaro PSP korrelyatsiyasi, anomaliyalar haqida ogohlantirish, boshqaruv ilgaklari va PSP integratsiya API'lari.
- **Qoʻldan tashqarida:** Toʻgʻridan-toʻgʻri qoʻllash (PSP masʼuliyatini saqlab qolish), sanktsiyalarni tekshirish (mavjud muvofiqlik quvurlari orqali koʻrib chiqiladi) va identifikatsiyani tekshirish (taxallus boshqaruvi buni qamrab oladi).

## Funktsional talablar
1. **Transaction Scoring API**: PSP to‘lovni hisob-kitob mexanizmiga yo‘naltirishdan oldin qo‘ng‘iroq qiladigan sinxron API, xavf ballini, toifali hukmni va mulohaza yuritish xususiyatlarini qaytaradi.
2. **Voqeani yutish**: uzluksiz o‘rganish uchun hisob-kitoblar natijalari oqimi, hamyonning hayotiy tsikli voqealari, qurilma barmoq izlari va PSP darajasidagi firibgarlik bo‘yicha fikr-mulohaza.
3. **Model Lifecycle Management**: oflayn o‘qitish, soyada joylashtirish, bosqichma-bosqich chiqarish va orqaga qaytarishni qo‘llab-quvvatlashga ega versiyali modellar. Har bir xususiyat uchun deterministik qayta evristik mavjud bo'lishi kerak.
4. **Tekshiruv davri**: PSPlar tasdiqlangan firibgarlik holatlari, noto'g'ri pozitivlar va tuzatish bo'yicha eslatmalarni taqdim etishi kerak. Tizim fikr-mulohazalarni xavf xususiyatlari bilan moslashtiradi va tahlillarni yangilaydi.
5. **Maxfiylik boshqaruvlari**: Saqlangan va uzatiladigan barcha maʼlumotlar taxallusga asoslangan boʻlishi kerak. Xom identifikator metamaʼlumotlarini oʻz ichiga olgan har qanday soʻrov rad etiladi va tizimga kiritiladi.
6. **Boshqaruv hisoboti**: jamlangan ko‘rsatkichlarni rejalashtirilgan eksporti (PSP bo‘yicha aniqlashlar, tipologiyalar, javob kutish vaqti) va vakolatli auditorlar uchun maxsus tergov API’lari.
7. **Mustahkamlik**: Avtomatik navbatni to'kib tashlash va takrorlash bilan kamida ikkita ob'ekt bo'ylab faol-faol joylashtirish. Agar xizmat yomonlashgan bo'lsa, PSP daftarni bloklamasdan mahalliy qoidalarga qaytadi.## Funktsional bo'lmagan talablar
- **Determinizm va izchillik**: Xavf ballari PSP qarorlarini boshqaradi, lekin buxgalteriya hisobining bajarilishini o'zgartirmaydi. Ledger majburiyatlari tugunlar bo'ylab deterministik bo'lib qoladi.
- **Mashqlanishi**: gorizontal masshtablash va soxta hamyon identifikatorlari bilan kalitlangan xabarlarni qismlarga ajratish bilan soniyada ≥10k xavfni baholashni davom ettiring.
- **Kuzatilishi**: Har bir ball qo‘ng‘irog‘i uchun ko‘rsatkichlarni (`fraud.scoring_latency_ms`, `fraud.risk_score_distribution`, `fraud.api_error_rate`, `fraud.model_version_active`) va tuzilgan jurnallarni ko‘rsating.
- **Xavfsizlik**: PSP va markaziy xizmat o'rtasidagi o'zaro TLS, javob konvertlarini imzolash uchun apparat xavfsizlik modullari, o'zgartirishlar aniqlangan audit izlari.
- **Muvofiqlik**: AML/CFT talablariga muvofiqlashtirish, sozlanishi mumkin saqlash muddatlarini ta'minlash va dalillarni saqlash bo'yicha ish oqimlari bilan birlashtirish.

## Arxitekturaga umumiy nuqtai
1. **API Gateway Layer**
  - Autentifikatsiya qilingan HTTP/JSON API-lar orqali baholash va fikr-mulohaza so'rovlarini qabul qiladi.
   - Norito kodeklari yordamida sxemani tekshirishni amalga oshiradi va har bir PSP identifikatori uchun tezlik chegaralarini qo'llaydi.

2. **Funksiyalarni birlashtirish xizmati**
   - Vaqt seriyali xususiyatlar do'konida saqlangan tarixiy agregatlar (tezlik, geofazoviy naqshlar, qurilmadan foydalanish) bilan kiruvchi so'rovlarni birlashtiradi.
   - Deterministik agregatsiya funksiyalaridan foydalangan holda sozlanishi funksiya oynalarini (daqiqalar, soatlar, kunlar) qo'llab-quvvatlaydi.

3. **Xavf mexanizmi**
   - faol model quvur liniyasini amalga oshiradi (gradient kuchaytirilgan daraxtlar ansambli, anomaliya detektorlari, qoidalar).
   - Model ballari mavjud bo'lmaganda cheklangan javoblarni kafolatlash uchun deterministik qayta tiklash qoidalarini o'z ichiga oladi.
   - `FraudAssessment` konvertlarini ball, tarmoqli, hissa qo'shadigan xususiyatlar va model versiyasi bilan chiqaradi.## Skorlash modellari va evristika
- **Baholar shkalasi va diapazonlari**: Xavf ballari 0–1000 gacha normallashtiriladi. Bandlar quyidagicha aniqlanadi: `0–249` (past), `250–549` (o'rta), `550–749` (yuqori), `750+` (tanqidiy). Guruhlar PSP uchun tavsiya etilgan amallarni (avtomatik tasdiqlash, bosqichma-bosqich oshirish, ko'rib chiqish uchun navbatga qo'yish, avtomatik rad etish) xaritasini ko'rsatadi, ammo ijro PSP-ga xos bo'lib qoladi.
- **Model ansambli**:
  - Gradient bilan mustahkamlangan qarorlar daraxtlari miqdor, taxallus/qurilma tezligi, savdogar toifasi, autentifikatsiya kuchi, PSP ishonch darajasi va oʻzaro hamyon grafigi xususiyatlari kabi tuzilgan xususiyatlarni oʻzlashtiradi.
  - Avtokoderga asoslangan anomaliya detektori vaqt oralig'idagi xatti-harakat vektorlarida ishlaydi (boshqa nom uchun sarflangan kadans, qurilma almashinuvi, vaqtinchalik entropiya). Driftni cheklash uchun ballar so'nggi PSP faoliyatiga nisbatan sozlangan.
  - Deterministik siyosat qoidalari birinchi navbatda amalga oshiriladi; ularning natijalari statistik modellarni ikkilik/uzluksiz xususiyatlar sifatida ta'minlaydi, shuning uchun ansambl o'zaro ta'sirlarni o'rganishi mumkin.
- **Fallback evristika**: Model bajarilmasa, deterministik qatlam qoida jazolarini jamlagan holda chegaralangan ball ishlab chiqaradi. Har bir qoida sozlanishi mumkin boʻlgan ogʻirlikni qoʻshadi, soʻngra 0–1000 shkalasiga biriktiriladi, bu esa eng yomon holatda kechikish va tushuntirishni kafolatlaydi.
- **Kechikish budjeti**: API shlyuzi + tekshiruvi uchun reyting quvurlari maqsadlari <20 ms, xususiyatlarni yig‘ish uchun <30 ms (xotiradagi keshlardan doimiy do‘konlarga yozib qo‘yiladi) va ansamblni baholash uchun <40 ms. Agar ML xulosasi byudjetdan oshib ketgan bo'lsa, deterministik qaytarilish <10 ms ichida qaytadi va bu umumiy P95 120 ms dan past bo'lishini ta'minlaydi.
 - **Kechikish budjeti**: API shlyuzi + tekshiruvi uchun reyting quvurlari maqsadlari <20 ms, xususiyatlarni yig‘ish uchun <30 ms (xotiradagi keshlardan doimiy do‘konlarga yozib qo‘yiladi) va ansamblni baholash uchun <40 ms. Agar ML xulosasi byudjetdan oshib ketgan bo'lsa, deterministik qaytarilish <10 ms ichida qaytadi va bu umumiy P95 120 ms dan past bo'lishini ta'minlaydi.## Xotira ichidagi funksiya kesh dizayni
- **Shard Layout**: Xususiyatlar do‘konlari 64-bit taxallusli xesh orqali `N = 256` parchalariga ajratilgan. Har bir parcha egalik qiladi:
  - Kesh-liniyaning joylashishini maksimal darajada oshirish uchun massivlar tuzilmasi sifatida saqlangan so'nggi tranzaksiya deltalari uchun qulfsiz halqa buferi (5 daqiqa + 1 soat derazalar).
  - 24 soat / 7 kunlik agregatlarni to'liq qayta hisoblashsiz saqlab turish uchun siqilgan Fenwick daraxti (bitli 16 bitli chelaklar).
  - Har bir taxallus uchun 1024 ta yozuv bilan chegaralangan kontragentlar → o'zgaruvchan statistika (hisoblash, yig'indi, dispersiya, oxirgi vaqt tamg'asi) xaritalash hop-skotch xesh xaritasi.
- **Xotira rezidentligi**: Issiq parchalar operativ xotirada qoladi. Oxirgi soatda 1% faol boʻlgan 50 M taxallusli koinot uchun kesh rezidentligi ~ 500 ming taxallusni tashkil qiladi. Faol metadata taxalluslari uchun ~ 320 B da, ishchi to'plam ~ 160 MB ni tashkil qiladi - zamonaviy serverlarda L3 kesh uchun etarlicha kichik.
- **Bir vaqtning o'zida**: O'quvchilar davrga asoslangan rekultivatsiya orqali o'zgarmas ma'lumotnomalarni oladilar; yozuvchilar deltalarni qo'shadilar va taqqoslash va almashtirish yordamida agregatlarni yangilaydilar. Bu mutex tortishuvidan qochadi va ikkita atomik operatsiyaga + chegaralangan ko'rsatkichni ta'qib qilish uchun issiq yo'llarni saqlaydi.
- **Oldindan yuklash**: Baholash xodimi soʻrovni tekshirish tugallangandan soʻng keyingi taxallus boʻlagi uchun `prefetch_read` qoʻllanmasini beradi va asosiy xotira kechikishini (~80 ns) xususiyatlarni birlashtirish ortida yashiradi.
- **Orqaga yozish jurnali**: har bir parcha uchun WAL deltalarni har 50 ms (yoki 4 KB) to'playdi va bardoshli do'konga o'tadi. Qayta tiklash chegaralarini qattiq ushlab turish uchun nazorat punktlari har 5 daqiqada ishlaydi.

### Nazariy kechikishning buzilishi (Intel Ice Lake-sinf serveri, 3,1 GGts)
- **Shard qidirish + oldindan yuklash**: 1 kesh o'tkazib yuborilgan (~80 ns) va xesh hisobi (<10 ns).
- **Ring bufer iteratsiyasi (32 ta yozuv)**: 32 × 2 yuk = 64 yuk; 32 B kesh liniyasi va ketma-ket kirish bilan bu L1 → ~20 ns ichida qoladi.
- **Fenwick yangilanishlari (log₂ 2048 ≈ 11 qadam)**: 11 koʻrsatkich sakrashi; yarmi L1, yarmi L2 uriladi faraz → ~30 ns.
- **Hop-skotch xaritasi probi (yuk koeffitsienti 0,75, 2 zond)**: 2 kesh liniyasi, ~2 × 15 ns.
- **Model funksiyalarini yig'ish**: 150 skalyar operatsiya (har biri <0,1 ns) → ~15 ns.Ularni jamlash har bir so'rov uchun ~160 ns hisoblash va ~120 ns xotira to'xtashini (~0,28 ms) beradi. Har bir yadroda bir vaqtning o'zida to'rtta yig'ish ishchisi bilan sahna portlash yukida ham 30 milodiy byudjetga osongina javob beradi; haqiqiy joylashtirish tasdiqlash uchun gistogrammalarni yozib olishi kerak (`fraud.feature_cache_lookup_ms` orqali).
- **Windows va yig'ish xususiyati**:
  - Qisqa muddatli (5 daqiqa, 1 soat) va uzoq muddatli (24 soat, 7 kun) Windows xarajat tezligini, qurilmadan qayta foydalanishni va taxallus grafik darajalarini kuzatib boradi.
  - Grafik xususiyatlari (masalan, taxalluslar bo'yicha umumiy qurilmalar, to'satdan paydo bo'lishi, yuqori xavfli klasterlardagi yangi kontragentlar) muntazam ravishda siqilgan xulosalarga tayanadi, shuning uchun so'rovlar sub-millisekundlarda qoladi.
  - Joylashuv evristikasi qo'pol geobuketlarni tarixiy xatti-harakatlar bilan taqqoslaydi, Haversine-ga asoslangan xavfning chegaralangan o'sishidan foydalanib, ehtimol bo'lmagan sakrashlarni (masalan, bir necha daqiqada bir nechta uzoq joylarni) belgilaydi.
  - Oqim shaklidagi detektorlar kiruvchi/chiqib ketgan miqdorlar va kontragentlarning aylanma gistogrammalarini aralashtirish/tumbling imzolarini aniqlash uchun saqlaydi (tezkor kirish, keyin shunga o'xshash fan chiqish, tsiklik hop ketma-ketliklari, qisqa muddatli vositachilar).
- **Qoidalar katalogi (to'liq emas)**:
  - **Tezlikning buzilishi**: har bir taxallus yoki har bir qurilma chegarasidan oshib ketadigan yuqori qiymatli uzatishlarning tezkor seriyasi.
  - **Taxallus grafigi anomaliyasi**: taxallus tasdiqlangan firibgarlik holatlari yoki maʼlum xachir naqshlari bilan bogʻlangan klaster bilan oʻzaro taʼsir qiladi.
  - **Qurilmani qayta ishlatish**: oldindan bog‘lanmagan holda turli PSP foydalanuvchilari kogortalariga tegishli taxalluslar bo‘yicha umumiy qurilma barmoq izi.
  - **Birinchi marta yuqori qiymatli**: yangi taxallus PSP’ning odatiy bortga kirish yo‘lagidan yuqoriroq miqdorga harakat qiladi.
  - **Autentifikatsiyani pasaytirish**: Tranzaksiya PSP tomonidan e'lon qilingan asossiz hisobning asosiy darajasidan (masalan, biometrikdan PIN-kodga qaytish) zaifroq omillardan foydalanadi.
  - **Aralash/tumbling naqsh**: Taxallus qisqa oynalar ichida bir nechta taxalluslar bo‘ylab chambarchas bog‘langan vaqt, takroriy aylanma yoki aylanma oqimlar bilan yuqori fan-in/fan-out zanjirlarida ishtirok etadi. Qoidalar grafik markazlashtirilgan ko'rsatkichlar va oqim shakli detektorlari yordamida ballni oshiradi; og'ir holatlar ML chiqishidan oldin ham `high` bandiga yopishadi.
  - **Tranzaksiyalarning qora roʻyxatiga kirish**: Taxallus yoki kontragent tarmoqdagi ovoz berish orqali yoki `sudo` boshqaruviga ega vakolat berilgan vakolatli (masalan, meʼyoriy buyruqlar, tasdiqlangan firibgarlik) orqali tuzilgan umumiy qora roʻyxat tasmasida paydo boʻladi. `critical` bandiga qisqichlarni kiriting va `BLACKLIST_MATCH` sabab kodini chiqaradi; PSPlar audit uchun bekor qilishlarni jurnalga kiritishlari kerak.
  - **Sandbox imzosi nomuvofiqligi**: PSP eskirgan model imzosi bilan yaratilgan baholashni taqdim etadi; ball `critical` gacha ko'tariladi va audit kancasi ishga tushadi.
- **Sabab kodlari**: Har bir baholash hissa ogʻirligi boʻyicha tartiblangan mashina tomonidan oʻqiladigan sabab kodlarini oʻz ichiga oladi (masalan, `VELOCITY_BREACH`, `NEW_DEVICE`, `GRAPH_HIGH_RISK`, `AUTH_DOWNGRADE`). PSP'lar foydalanuvchi xabarlarini yuborish uchun ularni operatorlar yoki hamyonlarga yuborishi mumkin.- **Model boshqaruvi**: kalibrlash va chegara sozlamalari hujjatlashtirilgan o‘quv kitoblari asosida amalga oshiriladi — ROC/PR egri chiziqlari har chorakda ko‘rib chiqiladi, belgilangan firibgarlikka qarshi sinovdan o‘tkaziladi va raqib modellari barqaror bo‘lmaguncha soyada ishlaydi. Har qanday chegara yangilanishi ikki tomonlama tasdiqlashni talab qiladi (firibgarlik operatsiyalari + mustaqil xavf).

## Boshqaruv manbalaridan olingan qora ro'yxat oqimi
- **Zanjirda mualliflik**: Qora ro‘yxat yozuvlari boshqaruv quyi tizimi (`iroha_core::smartcontracts::isi::governance`) orqali `BlacklistProposal` ISI sifatida kiritiladi, unda taxalluslar, PSP identifikatorlari yoki blokirovka qilinadigan qurilma barmoq izlari ro‘yxati keltirilgan. Manfaatdor tomonlar standart saylov byulletenidan foydalangan holda ovoz berishadi; kvorum bajarilgandan so'ng, zanjir tasdiqlangan qo'shimchalar/olib tashlashlar va monoton ravishda ortib borayotgan `blacklist_epoch` ni o'z ichiga olgan `GovernanceEvent::BlacklistUpdated` yozuvini chiqaradi.
- **Delegatsiya qilingan sudo yo'li**: Favqulodda harakatlar `sudo::Execute` ko'rsatmasi orqali bajarilishi mumkin, u bir xil `BlacklistUpdated` hodisasini chiqaradi, lekin o'zgarishni `origin = Sudo` deb belgilaydi. Bu zanjirdagi tarixni aniq kelib chiqishi bilan aks ettiradi, shuning uchun auditorlar konsensus ovozlarini vakolatli aralashuvlardan ajrata oladilar.
- **Tarqatish kanali**: FMS ko'prigi xizmati `LedgerEvent` oqimiga (Norito kodlangan) obuna bo'ladi va `BlacklistUpdated` voqealarini tomosha qiladi. Har bir voqea boshqaruv Merkle isboti bilan tasdiqlangan va qo'llanilishidan oldin blok imzosi bilan tasdiqlangan. Voqealar idempotent; FMS qayta o'ynamaslik uchun so'nggi `blacklist_epoch` ni saqlaydi.
- **FMS ichidagi ilova**: Yangilanish qabul qilingandan so'ng, yozuvlar deterministik qoidalar do'koniga yoziladi (taftish jurnallari bilan faqat qo'shimcha saqlash bilan ta'minlangan). Hisoblash mexanizmi qora ro'yxatni 30 soniya ichida qayta yuklaydi, bu keyingi baholashlar `BLACKLIST_MATCH` qoidasini ishga tushirishini va `critical` ga mahkamlanishini ta'minlaydi.
- **Audit va orqaga qaytarish**: Boshqaruv bir xil quvur orqali yozuvlarni olib tashlash uchun ovoz berishi mumkin. FMS tarixiy suratlarni `blacklist_epoch` tomonidan belgilab qo'yadi, shuning uchun operatorlar sud-tibbiy savollarga javob berishlari yoki tergov paytida o'tgan qarorlarni takrorlashlari mumkin.

4. **Learning & Analytics platformasi**
   - Tasdiqlangan firibgarlik hodisalari, hisob-kitoblar natijalari va PSP bo'yicha fikr-mulohazalarni faqat qo'shimcha daftar orqali oladi (masalan, Kafka + ob'ektni saqlash).
   - Modellarni qayta o'qitish uchun ma'lumotlar olimlari uchun oflayn noutbuklar/ish joylarini taqdim etadi. Model artefaktlari reklamadan oldin versiyalanadi va imzolanadi.

5. **Boshqaruv portali**
   - Auditorlar uchun tendentsiyalarni ko'rib chiqish, tarixiy baholashlarni qidirish va voqea hisobotlarini eksport qilish uchun cheklangan interfeys.
   - Siyosat tekshiruvlarini amalga oshiradi, shuning uchun tergovchilar PSP bilan hamkorlik qilmasdan PII ga tusha olmaydi.

6. **Integratsiya adapterlari**
   - Norito so'rovlari/javoblarini va mahalliy keshlashni amalga oshiradigan PSP (Rust, Kotlin, Swift, TypeScript) uchun engil SDKlar.
   - Hisob-kitob mexanizmi kancasi (`iroha_core` ichida), bu PSP tekshiruvdan keyingi tranzaktsiyalarni yo'naltirganda xavfni baholash ma'lumotnomalarini qayd qiladi.## Ma'lumotlar oqimi
1. PSP API shlyuziga autentifikatsiya qiladi va quyidagilarni o'z ichiga olgan `RiskQuery` ni yuboradi:
   - Toʻlovchi/toʻlovchi uchun taxallus identifikatorlari, xeshlangan qurilma identifikatori, tranzaksiya summasi, toifa, geolokatsion qoʻpol chelak, PSP ishonch bayroqlari va oxirgi sessiya metamaʼlumotlari.
2. Gateway foydali yukni tasdiqlaydi, PSP metama'lumotlari (litsenziya darajasi, SLA) bilan boyitadi va xususiyatlarni birlashtirish uchun navbatlar.
3. Xususiyatlar xizmati eng so'nggi agregatlarni oladi, model vektorini tuzadi va uni xavf mexanizmiga yuboradi.
4. Risk mexanizmi so'rovni baholaydi, deterministik sabab kodlarini biriktiradi, `FraudAssessment` imzolaydi va uni PSPga qaytaradi.
5. PSP tranzaksiyani tasdiqlash, rad etish yoki autentifikatsiyani oshirish uchun baholashni mahalliy siyosatlari bilan birlashtiradi.
6. Natija (tasdiqlangan/rad etilgan, firibgarlik tasdiqlangan/noto‘g‘ri ijobiy) uzluksiz takomillashtirish uchun o‘quv platformasiga asinxron tarzda suriladi.
7. Kundalik ommaviy jarayonlar boshqaruv hisobotlari uchun o'lchovlarni to'playdi va siyosat ogohlantirishlarini (masalan, ko'tarilgan ijtimoiy-muhandislik holatlari) PSP boshqaruv paneliga yuboradi.

## Iroha komponentlari bilan integratsiya
- ** Asosiy xost ilgaklari**: tranzaksiyani qabul qilish endi `fraud_assessment_band` metamaʼlumotlarini har doim `fraud_monitoring.enabled` va `required_minimum_band` oʻrnatilganda amalga oshiradi. Xost maydon yetishmayotgan yoki konfiguratsiya qilingan minimaldan past diapazonga ega tranzaktsiyalarni rad etadi va `missing_assessment_grace_secs` nolga teng bo'lmaganda deterministik ogohlantirish chiqaradi (masofaviy tekshirgich simi ulangandan so'ng FM-204 bosqichida o'chirish rejalashtirilgan). Baholar `fraud_assessment_score_bps` ni ham o'z ichiga olishi kerak; mezbon ballni e'lon qilingan diapazonga (0–249 ➜ past, 250–549 ➜ o'rta, 550–749 ➜ yuqori, 750+ ➜ kritik, 10000 gacha qo'llab-quvvatlanadigan bazaviy ball qiymatlari bilan) o'zaro tekshiradi. `fraud_monitoring.attesters` sozlanganda, tranzaktsiyalar Norito kodli `fraud_assessment_envelope` (base64) va mos keladigan `fraud_assessment_digest` (olti burchakli) biriktirilishi kerak. Demon konvertni deterministik tarzda dekodlaydi, Ed25519 imzosini attestator reestriga qarshi tekshiradi, imzolanmagan foydali yuk bo'yicha dayjestni qayta hisoblaydi va mos kelmaslikni rad etadi, shuning uchun faqat tasdiqlangan baholashlar konsensusga erishadi.
- **Konfiguratsiya**: `iroha_config::fraud_monitoring` ostida xavf xizmatining so'nggi nuqtalari, kutish vaqtlari va talab qilinadigan baholash diapazonlari uchun konfiguratsiya yozuvlarini qo'shing. Birlamchi sozlamalar mahalliy rivojlanish uchun amal qilishni o'chirib qo'yadi.| Kalit | Tur | Standart | Eslatmalar |
  | --- | --- | --- | --- |
  | `enabled` | bool | `false` | Qabul tekshiruvlari uchun asosiy kalit; `required_minimum_band` holda xost ogohlantirishni qayd qiladi va ijroni o'tkazib yuboradi. |
  | `service_endpoints` | massiv | `[]` | Firibgarlik xizmatining asosiy URL manzillarining tartiblangan roʻyxati. Dublikatlar deterministik tarzda olib tashlanadi; kelgusi tekshiruvchi uchun ajratilgan. |
  | `connect_timeout_ms` | davomiyligi | `500` | Ulanishni to'xtatishga urinishdan oldin millisekundlar; nol qiymatlari standartga qaytariladi. |
  | `request_timeout_ms` | davomiyligi | `1500` | Xavf xizmatidan javob kutish uchun millisekundlar. |
  | `missing_assessment_grace_secs` | davomiyligi | `0` | Etishmayotgan baholarga ruxsat beruvchi imtiyozli oyna; nolga teng bo'lmagan qiymatlar tranzaktsiyaga ruxsat beruvchi deterministik qayta tiklashni ishga tushiradi. |
  | `required_minimum_band` | enum (`low`, `medium`, `high`, `critical`) | `null` | Belgilansa, tranzaktsiyalar ushbu jiddiylik oralig'ida yoki undan yuqoriroq bahoni qo'shishi kerak; pastroq qiymatlar rad etiladi. `enabled` rost bo'lsa ham, shlyuzni o'chirish uchun `null` ni o'rnating. |
  | `attesters` | massiv | `[]` | Attestatsiya dvigatellarining ixtiyoriy reestri. To'ldirilganda konvertlar sanab o'tilgan kalitlardan biri bilan imzolanishi va tegishli dayjestni o'z ichiga olishi kerak. |

- **Validatsiya**: `crates/iroha_core/tests/fraud_monitoring.rs` da birlik testlari o'chirilgan, etishmayotgan va yetarli bo'lmagan tarmoqli yo'llarni qamrab oladi; `integration_tests::fraud_monitoring_requires_assessment_bands` masxaralangan baholash oqimini oxirigacha mashq qiladi.

- **Telemetriya**: `iroha_telemetry` baholash hisoblarini (`fraud_psp_assessments_total{tenant,band,lane,subnet}`), etishmayotgan metamaʼlumotlarni (`fraud_psp_missing_assessment_total{tenant,lane,subnet,cause}`), kechikish gistogrammalarini (`fraud_psp_latency_ms{tenant,lane,subnet}`), 804000 ball taqsimotini qayd qiluvchi PSP-ga qaragan kollektorlarni eksport qiladi. foydali yuklar (`fraud_psp_invalid_metadata_total{tenant,field,lane,subnet}`), attestatsiya natijalari (`fraud_psp_attestation_total{tenant,engine,lane,subnet,status}`) va natijalarning mos kelmasligi (`fraud_psp_outcome_mismatch_total{tenant,direction,lane,subnet}`). Har bir tranzaksiyada kutilayotgan metamaʼlumotlar kalitlari quyidagilardir: `fraud_assessment_band`, `fraud_assessment_tenant`, `fraud_assessment_score_bps`, `fraud_assessment_latency_ms`, attester konverti/digest juftligi (`fraud_assessment_envelope`, `fraud_assessment_envelope`, I00NI0X va I00300-dan keyingi), `fraud_assessment_disposition` bayroq (qiymatlar: `approved`, `declined`, `manual_review`, `confirmed_fraud`, `false_positive`, Prometheus, Prometheus).
- **Norito sxemasi**: `RiskQuery`, `FraudAssessment` va boshqaruv hisobotlari uchun Norito turlarini aniqlang. Kodek barqarorligini kafolatlash uchun aylanma sinovlarni taqdim eting.

## Maxfiylik va maʼlumotlarni minimallashtirish
- Taxalluslar, xeshlangan qurilma identifikatorlari va qo'pol geolokatsiya chelaklari markaziy xizmat bilan birgalikda umumiy ma'lumotlar tekisligini tashkil qiladi.
- PSPlar taxalluslardan haqiqiy identifikatorlarga xaritalashni saqlaydi; Bunday xaritalash ularning perimetrini tark etmaydi.
- Xavfli modellar faqat taxallusli xatti-harakatlar signallari va PSP tomonidan taqdim etilgan kontekstda ishlaydi (savdogar toifasi, kanal, autentifikatsiya kuchi).
- Audit eksporti jamlangan (masalan, bir kunlik PSP uchun hisoblar). Har qanday matkap ikki tomonlama nazoratni va PSP tomonida anonimizatsiyani talab qiladi.## Operatsiyalar va joylashtirish
- Hisoblash platformasini markaziy bank tugun operatorlaridan farqli, tayinlangan operator tomonidan boshqariladigan maxsus quyi tizim sifatida joylashtirish.
- Moviy/yashil muhitlarni taqdim eting: `fraud-scoring-prod`, `fraud-scoring-shadow`, `fraud-lab`.
- Avtomatlashtirilgan sog'liqni tekshirishni amalga oshirish (API kechikishi, xabarlar to'plami, modelni yuklash muvaffaqiyati). Agar sog'liq tekshiruvlari muvaffaqiyatsiz bo'lsa, PSP SDK avtomatik ravishda faqat mahalliy rejimga o'tadi va operatorlarni xabardor qiladi.
- Saqlash chelaklarini saqlang: issiq saqlash (xususiyatlar do'konida 30 kun), issiq saqlash (ob'ektni saqlashda 1 yil), sovuq arxiv (5 yil siqilgan).

## Telemetriya kollektorlari va asboblar paneli

### Kerakli kollektorlar

- **Prometheus scrape**: `fraud_psp_*` seriyali eksport qilinishi uchun PSP integratsiya profilida ishlaydigan har bir validatorda `/metrics` ni yoqing. Birlamchi teglar `subnet="global"` va `lane` identifikatorlarini o'z ichiga oladi, shuning uchun asboblar paneli ko'p tarmoqli marshrutlash kemalarida bir marta aylana oladi.
- **Baholashning umumiy summasi**: `fraud_psp_assessments_total{tenant,band}` har bir jiddiylik diapazoni uchun qabul qilingan baholashlarni hisoblaydi; agar ijarachi 5 daqiqa davomida hisobot berishni to'xtatsa, yong'in haqida ogohlantiradi.
- **Yo'qolgan metama'lumotlar**: `fraud_psp_missing_assessment_total{tenant,cause}` qattiq rad etishlarni (`cause="missing"`) imtiyozli oyna imtiyozlaridan (`cause="grace"`) ajratib turadi. Qayta-qayta chelakka tushadigan darvoza operatsiyalari.
- **Keshirish gistogrammasi**: `fraud_psp_latency_ms_bucket` PSP tomonidan bildirilgan skoring kechikishini kuzatadi. Maqsad 20% chetga chiqsa, kuchaying.
- **Metamaʼlumotlar notoʻgʻri**: `fraud_psp_invalid_metadata_total{field}` PSP foydali yuk regressiyalarini (masalan, ijarachi identifikatorlari etishmayotgan, notoʻgʻri tuzilgan joylashuvlar) belgilaydi, shuning uchun SDK yangilanishlari tezda chiqarilishi mumkin.
- **Attestatsiya holati**: `fraud_psp_attestation_total{tenant,engine,status}` konvertlar imzolanayotganini va dayjestlar mos kelishini tasdiqlaydi. Agar `status!="verified"` har qanday ijarachi yoki dvigatel uchun keskin oshib ketsa, ogohlantirish.

### Boshqaruv paneli qamrovi

- **Ijroiya haqida umumiy ma'lumot**: har bir ijarachi bo'yicha `fraud_psp_assessments_total` diagrammasi, P95 kechikish va nomuvofiqliklarni umumlashtiruvchi jadval bilan birlashtirilgan.
- **Operatsiyalar**: `fraud_psp_latency_ms` va `fraud_psp_score_bps` uchun gistogramma panellari, haftalik taqqoslash, shuningdek, `fraud_psp_missing_assessment_total` uchun `cause` bo'lingan yagona statistik hisoblagichlar.
- **Xavf monitoringi**: har bir ijarachiga `fraud_psp_outcome_mismatch_total` shtrixli diagrammasi, `band` `low` yoki `medium` bo'lgan so'nggi `fraud_assessment_disposition=confirmed_fraud` holatlari ko'rsatilgan batafsil jadval.
- **Ogohlantirish qoidalari**:
  - `rate(fraud_psp_missing_assessment_total{cause="missing"}[5m]) > 0` → peyjing ogohlantirishi (qabul qilish PSP trafigini rad etadi).
  - `histogram_quantile(0.95, sum(rate(fraud_psp_latency_ms_bucket[10m])) by (le,tenant)) > 150` → kechikish SLO buzilishi.
  - `sum by (tenant) (rate(fraud_psp_outcome_mismatch_total{direction="missed_fraud"}[1h])) > 0.01` → model drift / siyosat bo'shlig'i.

### Muvaffaqiyatsizlik kutishlari- PSP SDK-lari ikkita faol skoring so'nggi nuqtasini saqlab turishi va transport xatolari yoki 200ms dan yuqori kechikishlar aniqlangandan keyin 15 soniya ichida ishlamay qolishi kerak. Buxgalteriya kitobi eng ko'p `fraud_monitoring.missing_assessment_grace_secs` uchun qulay trafikka toqat qiladi; operatorlar ishlab chiqarishda tugmani <=30 soniyada ushlab turishi kerak.
- Validatorlar zaxira holatida `fraud_psp_missing_assessment_total{cause="grace"}` qayd qiladi; agar ijarachi 5 daqiqadan ortiq vaqt davomida imtiyozda qolsa, PSP qo'lda ko'rib chiqishga o'tishi va umumiy firibgarlik operatsiyalari jamoasi bilan Sev2 hodisasini ochishi kerak.
- Faol-faol joylashtirishlar falokatni tiklash mashqlari paytida navbatni yo'qotish/qayta o'ynashni ko'rsatishi kerak. Takrorlash ko'rsatkichlari `fraud_psp_latency_ms` P99 ni takrorlash oynasi uchun 400 ms ostida saqlashi kerak.

## PSP ma'lumotlar almashish nazorat ro'yxati

1. **Telemetriya sanitariya-tesisat**: buxgalteriya kitobiga topshirilgan har bir tranzaksiya uchun yuqorida sanab o'tilgan metama'lumotlar kalitlarini ko'rsating; ijarachi identifikatorlari taxallusli bo'lishi va PSP shartnomasiga mos kelishi kerak.
2. **Anonimlashtirish**: qurilma xeshlari, taxallus identifikatorlari va dispozitsiyalari PSP perimetrini tark etishdan oldin taxallus bilan atalganligini tasdiqlang; Norito metamaʼlumotlariga hech qanday PII kiritib boʻlmaydi.
3. **Kechikish hisoboti**: `fraud_assessment_latency_ms` ni oxirigacha vaqtni (PSP ga shlyuz) to'ldiring, shunda SLA regressiyalari darhol yuzaga keladi.
4. **Natijalarni solishtirish**: nomuvofiqlik ko‘rsatkichlarini to‘g‘ri saqlash uchun firibgarlik holatlari tasdiqlangach (masalan, to‘lovni qaytarib olish) `fraud_assessment_disposition`-ni yangilang.
5. **To‘xtab qolish mashqlari**: umumiy nazorat ro‘yxatidan foydalanib, har chorakda mashq qiling — so‘nggi nuqtaning avtomatik o‘zgartirilishini tekshiring, qulay oynalar ro‘yxatini ta’minlang va `scripts/ci/schedule_fraud_scoring.sh` tomonidan topshirilgan kuzatuv topshirig‘iga mashq eslatmalarini qo‘shing.
6. **Boshqaruv panelini tekshirish**: PSP operatsion guruhlari ishga tushirilgandan so‘ng va har bir qizil jamoa mashg‘ulotidan so‘ng Prometheus asboblar panelini ko‘rib chiqishlari kerak, bu ko‘rsatkichlar ijarachilarning kutilayotgan yorliqlari bilan oqayotganini tasdiqlashi kerak.

## Xavfsizlik masalalari
- Barcha javoblar apparat kalitlari bilan imzolanadi; PSPlar ballarga ishonishdan oldin imzolarni tasdiqlaydi.
- Model chegaralarini o'rganishga qaratilgan tekshiruv hujumlarini yumshatish uchun taxallus/qurilma bo'yicha tarif chegarasi.
- PSP identifikatorini ommaga oshkor qilmasdan, sizib chiqqan javoblarni kuzatish uchun baholashlar ichiga suv belgilarini kiriting.
- Har chorakda Xavfsizlik guruhi (Milestone 0) bilan kelishilgan holda qizil jamoa mashqlarini o'tkazing va topilmalarni yo'l xaritasi yangilanishiga kiriting.## Amalga oshirish bosqichlari
1. **0-bosqich – asoslar**
   - Norito sxemalarini, PSP SDK iskalasini, konfiguratsiya simlarini va daftar tomonini tekshirishni yakunlang.
   - Majburiy xavf tekshiruvlarini o'z ichiga olgan deterministik qoidalar mexanizmini yarating (tezlik, taxallus juftligi uchun tezlik, qurilmani qayta ishlatish).
2. **1-bosqich – Markaziy reytingdagi MVP**
   - Xususiyatlar do'konini, skoring xizmatini va telemetriya asboblar panelini o'rnating.
   - Cheklangan PSP kohorti bilan real vaqt rejimida ballarni birlashtirish; kechikish va sifat ko'rsatkichlarini yozib oling.
3. **2-bosqich – Kengaytirilgan tahlil**
   - Anomaliyalarni aniqlash, grafik asosidagi havolalar tahlili va moslashuvchan chegaralarni joriy qilish.
   - Boshqaruv portali va to'plamli hisobot quvurlarini ishga tushirish.
4. **3-bosqich – uzluksiz o‘rganish va avtomatlashtirish**
   - Modelni o'qitish/validatsiya quvurlarini avtomatlashtirish, kanareykalarni joylashtirishni qo'shish va SDK qamrovini kengaytirish.
   - Yurisdiktsiyalararo ma'lumotlar almashish kelishuvlariga moslang va kelajakdagi ko'p tarmoqli ko'priklarga ulang.

## Ochiq savollar
- Firibgarlik xizmati operatorini qaysi nazorat organi tuzadi va nazorat vazifalari qanday taqsimlanadi?
- PSPlar provayderlar bo'ylab izchil UXni saqlab turganda oxirgi foydalanuvchi muammolari oqimini qanday ochib beradi?
- Asosiy xizmat barqaror bo'lgandan keyin qaysi maxfiylikni yaxshilaydigan texnologiyalar (masalan, xavfsiz anklavlar, gomomorf yig'ish) ustuvor bo'lishi kerak?