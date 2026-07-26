---
lang: uz
direction: ltr
source: docs/source/crypto/sm_chinese_crypto_law_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5d0657539dfcca1869a0ab4fc9adee8665f18708f71b4c116dc8900ae5eae75
source_last_modified: "2026-01-05T09:28:12.004169+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM muvofiqligi haqida qisqacha — Xitoy kriptografiyasi qonuni majburiyatlari
% Iroha Muvofiqlik va Kripto ishchi guruhlari
% 2026-02-12

# Tezkor

> Siz LLM Hyperledger Iroha kripto va platformasi guruhlari uchun muvofiqlik tahlilchisi sifatida ishlaysiz.  
> Fon:  
> - Hyperledger Iroha bu Rust asosidagi ruxsat etilgan blokcheyn boʻlib, u endi Xitoy GM/T SM2 (imzolar), SM3 (xesh) va SM4 (blok shifr) primitivlarini qoʻllab-quvvatlaydi.  
> - Xitoy materikidagi operatorlar XXR kriptografiyasi qonuni (2019), Ko‘p darajali himoya sxemasi (MLPS 2.0), Davlat kriptografiya boshqarmasi (SCA) ariza berish qoidalariga va Savdo vazirligi (MOFCOM) va Bojxona boshqarmasi tomonidan nazorat qilinadigan import/eksport nazoratiga rioya qilishlari kerak.  
> - Iroha ochiq kodli dasturiy ta'minotni xalqaro miqyosda tarqatadi. Ba'zi operatorlar mahalliy SM-ni yoqadigan ikkilik fayllarni kompilyatsiya qiladilar, boshqalari esa oldindan yaratilgan artefaktlarni import qilishlari mumkin.  
> So'ralgan tahlil: Ochiq kodli blokcheyn dasturiy ta'minotida SM2/SM3/SM4 qo'llab-quvvatlashini jo'natish natijasida yuzaga keladigan asosiy yuridik majburiyatlarni umumlashtiring, jumladan: (a) tijorat va asosiy/umumiy kriptografiya chelaklari bo'yicha tasniflash; (b) davlat tijorat kriptografiyasini amalga oshiradigan dasturiy ta'minotni topshirish/tasdiqlash talablari; (c) ikkilik va manba uchun eksport nazorati; (d) MLPS 2.0 bo'yicha tarmoq operatorlari bo'yicha operatsion majburiyatlar (asosiy boshqaruv, ro'yxatga olish, hodisalarga javob berish). Iroha loyihasi (hujjatlar, manifestlar, muvofiqlik bayonotlari) va Xitoyda SM-ni yoqadigan tugunlarni joylashtirgan operatorlar uchun aniq harakatlarni belgilang.

# Ijroiya xulosasi

- **Tasnifi:** SM2/SM3/SM4 ilovalari “asosiy” yoki “umumiy” kriptografiya emas, balki “davlat tijoriy kriptografiyasi” (kàngìnìnì) ostida boʻladi, chunki ular fuqarolik/tijorat maqsadlarida foydalanish uchun ruxsat berilgan ommaviy algoritmlardir. Ochiq manbali tarqatishga ruxsat beriladi, lekin Xitoyda taklif etilayotgan tijorat mahsulotlari yoki xizmatlarida foydalanilganda ariza topshirish shart.
- **Loyiha majburiyatlari:** Algoritmning kelib chiqishi, deterministik tuzish bo'yicha ko'rsatmalar va ikkilik tizimlar davlat tijorat kriptografiyasini amalga oshiradigan muvofiqlik bayonotini taqdim eting. Norito SM qobiliyatini belgilab qo'yishini saqlab qoling, shunda quyi oqim integratorlari arizalarni to'ldirishlari mumkin.
- **Operator majburiyatlari:** Xitoylik operatorlar SM algoritmlaridan foydalangan holda mahsulot/xizmatlarni viloyat SCA byurosiga topshirishlari, MLPS 2.0 roʻyxatini toʻldirishlari (moliyaviy tarmoqlar uchun ehtimol 3-daraja), tasdiqlangan kalitlarni boshqarish va jurnallar roʻyxatini boshqarish vositalarini oʻrnatishlari va eksport/import deklaratsiyasi MOFCOM katalogi exemptions bilan mos kelishini taʼminlashlari kerak.

# Normativ landshaft| Nizom | Qo'llash doirasi | Iroha SM qo'llab-quvvatlashiga ta'siri |
|------------|-------|----------------------------|
| **XXR kriptografiya qonuni (2019)** | Asosiy/umumiy/tijorat kriptografiyasini, mandatlarni boshqarish tizimini, hujjatlarni topshirish va sertifikatlashni belgilaydi. | SM2/SM3/SM4 "tijorat kriptografiyasi" bo'lib, Xitoyda mahsulot/xizmat sifatida taqdim etilganda hujjatlarni topshirish/sertifikatlash qoidalariga amal qilishi kerak. |
| **Tijorat kriptografiya mahsulotlari uchun SCA ma'muriy choralari** | Ishlab chiqarish, sotish va xizmat ko'rsatishni boshqaradi; mahsulotni topshirish yoki sertifikatlashni talab qiladi. | SM algoritmlarini amalga oshiradigan ochiq kodli dasturiy ta'minot tijorat takliflarida foydalanilganda operator hujjatlarini talab qiladi; Ishlab chiquvchilar hujjatlarni topshirishga yordam berishlari kerak. |
| **MLPS 2.0 (Kiberxavfsizlik qonuni + MLPS qoidalari)** | Operatorlardan axborot tizimlarini tasniflashni va xavfsizlik nazoratini amalga oshirishni talab qiladi; 3 yoki undan yuqori daraja kriptografiyaga muvofiqligini tasdiqlaydi. | Moliyaviy/identifikatsiya ma'lumotlariga ishlov beruvchi blokcheyn tugunlari odatda MLPS 3-darajasida ro'yxatdan o'tadi; operatorlar SM dan foydalanishni, kalitlarni boshqarishni, jurnalni yozishni va hodisalarni boshqarishni hujjatlashtirishlari kerak. |
| **MOFCOM eksport nazorati katalogi va bojxona importi qoidalari** | Kriptografik mahsulotlarni eksport qilishni nazorat qiladi, muayyan algoritmlar/apparat uchun ruxsatnomalarni talab qiladi. | Manba kodini nashr qilish odatda “jamoat mulki” qoidalariga muvofiq ozod qilinadi, lekin SM qobiliyatiga ega tuzilgan ikkilik fayllarni eksport qilish, agar tasdiqlangan oluvchilarga yuborilmasa, katalogni ishga tushirishi mumkin; import qiluvchilar davlat tijorat kriptografiyasini e'lon qilishlari kerak. |

# Asosiy majburiyatlar

## 1. Mahsulot va xizmatlarni topshirish (davlat kriptografiya boshqarmasi)

- **Kim fayl beradi:** Xitoyda mahsulot/xizmat taqdim etuvchi tashkilot (masalan, operator, SaaS provayderi). Ochiq manba ta'minotchilari faylga kirishlari shart emas, lekin qadoqlash bo'yicha ko'rsatmalar quyi oqimlarni taqdim etishni ta'minlashi kerak.
- **Etkazib beriladigan narsalar:** Algoritm tavsifi, xavfsizlik dizayni hujjatlari, sinov dalillari, ta'minot zanjiri kelib chiqishi va aloqa ma'lumotlari.
- **Iroha amal:** Algoritm qamrovi, deterministik qurish bosqichlari, qaramlik xeshlari va xavfsizlik soʻrovlari uchun kontaktni oʻz ichiga olgan “SM kriptografiya bayonoti”ni nashr eting.

## 2. Sertifikatlash va sinov

- Ayrim sektorlar (moliya, telekommunikatsiya, muhim infratuzilma) akkreditatsiyalangan laboratoriya sinovi yoki sertifikatlashni talab qilishi mumkin (masalan, CC-Grade/OSCCA sertifikati).
- GM/T spetsifikatsiyalariga muvofiqligini ko'rsatadigan regressiya sinovi artefaktlarini qo'shing.

## 3. MLPS 2.0 Operatsion boshqaruvlari

Operatorlar:1. **Blokcheyn tizimini** jamoat xavfsizligi byurosida roʻyxatdan oʻtkazing, jumladan kriptografiyadan foydalanish boʻyicha xulosalar.
2. **Kalitlarni boshqarish siyosatini amalga oshirish**: SM2/SM4 talablariga muvofiq kalitlarni yaratish, tarqatish, aylantirish, yo'q qilish; asosiy hayot aylanishi voqealarini qayd etish.
3. **Xavfsizlik tekshiruvini yoqish**: SM-yoqilgan tranzaksiya jurnallarini, kriptografik operatsiya hodisalarini va anomaliyalarni aniqlashni yozib olish; jurnallarni ≥6 oy saqlang.
4. **Hodisaga javob:** kriptografiyani buzish tartib-qoidalari va hisobot berish vaqtini o‘z ichiga olgan hujjatlashtirilgan javob rejalarini saqlang.
5. **Vendor boshqaruvi:** yuqori oqim dasturiy ta'minot provayderlari (Iroha loyihasi) zaiflik haqida bildirishnomalar va yamoqlarni taqdim etishini ta'minlang.

## 4. Import/eksport masalalari

- **Ochiq kodli manba kodi:** Odatda ommaviy domen istisnolari ostida ozod qilinadi, lekin xizmat ko'rsatuvchilar kirish jurnallarini kuzatuvchi va davlat tijoriy kriptografiyasiga havola qiluvchi litsenziya/ravshanlikni o'z ichiga olgan serverlarda yuklab olishlari kerak.
- **Oldindan qurilgan ikkilik fayllar:** SM-ni qo'llab-quvvatlaydigan ikkilik fayllarni Xitoyga/tashqariga jo'natuvchi eksportchilar ushbu elementning "Tijorat kriptografiyasi eksport nazorati katalogi" bilan qamrab olinganligini tasdiqlashlari kerak. Maxsus uskunasiz umumiy maqsadli dasturiy ta'minot uchun oddiy ikki tomonlama foydalanish deklaratsiyasi etarli bo'lishi mumkin; agar mahalliy advokat ma'qullamasa, boshqaruvchilar qattiqroq nazoratga ega bo'lgan yurisdiktsiyalardan ikkilik fayllarni tarqatmasligi kerak.
- **Operator importi:** Xitoyga ikkilik fayllarni olib kiruvchi sub'ektlar kriptografiyadan foydalanishni e'lon qilishlari kerak. Bojxona tekshiruvini soddalashtirish uchun hash manifestlari va SBOMni taqdim eting.

# Tavsiya etilgan loyiha harakatlari

1. **Hujjatlar**
   - `docs/source/crypto/sm_program.md` ga davlat tijoriy kriptografiya holati, ariza topshirish kutilmalari va aloqa nuqtalarini ko'rsatuvchi muvofiqlik ilovasini qo'shing.
   - Operatorlar arizalarni tayyorlashda foydalanishi mumkin bo'lgan Norito manifest maydonini (`crypto.sm.enabled=true`, `crypto.sm.approval=l0|l1`) nashr qiling.
   - Torii `/v1/node/capabilities` reklamasi (va `iroha runtime capabilities` CLI taxalluslari) har bir nashrda yuborilishiga ishonch hosil qiling, shunda operatorlar MLPS/gàngín uchun `crypto.sm` manifest suratini olishlari mumkin.
   - Ikki tilda (EN/ZH) muvofiqlikni tezkor boshlash majburiyatlarini umumlashtirish.
2. **Artefaktlarni chiqarish**
   - SM-ni qo'llab-quvvatlaydigan tuzilmalar uchun SBOM/CycloneDX fayllarini jo'natish.
   - Deterministik qurilish skriptlarini va takrorlanadigan Dockerfilllarni qo'shing.
3. **Yordam operatori arizalari**
   - Algoritmga muvofiqligini tasdiqlovchi shablon harflarini taklif qiling (masalan, GM/T havolalari, test qamrovi).
   - Sotuvchining bildirishnomalari talablarini qondirish uchun xavfsizlik bo'yicha maslahatlar ro'yxatini yuriting.
4. **Ichki boshqaruv**
   - Relizlar roʻyxatida SM muvofiqligini tekshirish nuqtalarini kuzatib boring (tugallangan audit, hujjatlar yangilangan, manifest maydonlari mavjud).

# Operator harakati (Xitoy)1. O'rnatish "tijorat kriptografiyasi mahsuloti/xizmati" ekanligini aniqlang (ko'pchilik korporativ tarmoqlarda shunday).
2. Mahsulot/xizmatni viloyat SCA byurosiga topshirish; Iroha muvofiqlik bayonoti, SBOM, test hisobotlarini ilova qiling.
3. MLPS 2.0, maqsadli 3-darajali boshqaruv ostida blokcheyn tizimini ro'yxatdan o'tkazing; Iroha jurnallarini xavfsizlik monitoringiga integratsiyalash.
4. SM kalitining hayot aylanishi protseduralarini o'rnating (kerak bo'lganda tasdiqlangan KMS/HSM dan foydalaning).
5. Voqealarga javob berish mashqlariga kriptografiyani buzish stsenariylarini kiritish; Iroha ta'minotchilari bilan eskalatsiya aloqalarini o'rnating.
6. Transchegaraviy ma'lumotlar oqimi uchun, agar shaxsiy ma'lumotlar eksport qilinsa, qo'shimcha CAC (Cyberspace Administration) arizalarini tasdiqlang.

# Mustaqil soʻrov (nusxalash/qoʻyish)

> Siz LLM Hyperledger Iroha kripto va platformasi guruhlari uchun muvofiqlik tahlilchisi sifatida ishlaysiz.  
> Ma'lumot: Hyperledger Iroha - Rust-ga asoslangan ruxsat berilgan blokcheyn bo'lib, u endi Xitoy GM/T SM2 (imzolar), SM3 (xesh) va SM4 (blok shifr) primitivlarini qo'llab-quvvatlaydi. Xitoy materikidagi operatorlar XXR kriptografiyasi qonuni (2019), Ko‘p darajali himoya sxemasi (MLPS 2.0), Davlat kriptografiya ma’muriyati (SCA) ariza berish qoidalariga hamda MOFCOM va Bojxona boshqarmasi tomonidan nazorat qilinadigan import/eksport nazoratiga rioya qilishlari kerak. Iroha loyihasi SM-ni yoqadigan ochiq kodli dasturiy ta'minotni xalqaro miqyosda tarqatadi; ba'zi operatorlar ikkilik fayllarni mahalliy ravishda kompilyatsiya qiladi, boshqalari esa oldindan qurilgan artefaktlarni import qiladi.  
> Vazifa: Ochiq kodli blokcheyn dasturiy ta'minotida SM2/SM3/SM4 qo'llab-quvvatlashini jo'natish orqali yuzaga kelgan qonuniy majburiyatlarni umumlashtiring. Ushbu algoritmlar tasnifi (tijorat va asosiy/umumiy kriptografiya), dasturiy mahsulotlar uchun talab qilinadigan hujjatlar yoki sertifikatlar, manba va ikkiliklarga tegishli eksport/import boshqaruvlari va MLPS 2.0 (kalitlarni boshqarish, jurnalga yozish, hodisalarga javob) boʻyicha tarmoq operatorlari uchun operatsion majburiyatlarni qamrab oling. Iroha loyihasi (hujjatlar, manifestlar, muvofiqlik bayonotlari) va Xitoyda SM-ni yoqadigan tugunlarni joylashtirgan operatorlar uchun aniq harakatlarni taqdim eting.