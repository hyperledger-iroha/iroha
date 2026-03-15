---
lang: uz
direction: ltr
source: docs/source/global_feature_matrix.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6a406b7656a87bb1469444db1cc2d2d5922f16660b53cc7eaef5b838199127e8
source_last_modified: "2026-01-23T23:46:10.135119+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Global xususiyat matritsasi

Legend: `◉` toʻliq amalga oshirildi · `○` asosan amalga oshirildi · `▲` qisman amalga oshirildi · `△` joriy etildi · `✖︎` boshlanmadi

## Konsensus va tarmoq

| Xususiyat | Holati | Eslatmalar | Dalil |
|---------|--------|-------|----------|
| Ko'p kollektor K/r qo'llab-quvvatlash & birinchi-commit-sertifikat-yutdi | ◉ | Deterministik kollektorni tanlash, ortiqcha fan-out, zanjirdagi K/r parametrlari va sinovlar bilan jo'natilgan birinchi yaroqli-kommit-sertifikatni qabul qilish. | status.md:255; status.md:314 |
| Elektron yurak stimulyatori orqa o'chirilishi, RTT qavati, deterministik jitter | ◉ | Konfiguratsiya, telemetriya va hujjatlar orqali simli jitter diapazoni bilan sozlanadigan taymerlar. | status.md:251 |
| NEW_VIEW gating va eng yuqori QC kuzatuvi | ◉ | Boshqaruv oqimi NEW_VIEW/Dalillarni olib boradi, eng yuqori QC monoton tarzda qabul qilinadi, qo'l siqish esa hisoblangan barmoq izini himoya qiladi. | status.md:210 |
| mavjudligi dalillarni kuzatish (maslahat) | ◉ | Mavjudlik dalillari chiqarilgan va kuzatilgan; commit v1-da mavjudligini cheklamaydi. | status.md:so'nggi |
| Ishonchli translyatsiya (DA foydali yuk tashish) | ◉ | RBC xabarlar oqimi (Init/Chunk/Ready/Deliver) transport/tiklash yo'li sifatida `da_enabled=true` yoqilganda; Mavjudlik dalillari kuzatib boriladi (maslahat), daromadlar mustaqil ravishda amalga oshiriladi. | status.md:so'nggi |
| QC holatini ildiz bilan bog'lash | ◉ | QCs `parent_state_root`/`post_state_root` ga ega; alohida ijro etuvchi-QC eshigi yo'q. | status.md:so'nggi |
| Dalillarni tarqatish va auditning yakuniy nuqtalari | ◉ | ControlFlow::Dalillar, Torii dalil so'nggi nuqtalari va salbiy testlar qo'ndi. | status.md:176; status.md:760-761 |
| RBC telemetriyasi, tayyorlik/etkazib berilgan ko'rsatkichlari | ◉ | Operatorlar uchun `/v1/sumeragi/rbc*` so'nggi nuqtalari va telemetriya hisoblagichlari/gistogrammasi mavjud. | status.md:283-284; status.md:772 |
| Konsensus parametri reklama va topologiyani tekshirish | ◉ | Tugunlar `(collectors_k, redundant_send_r)` ni translyatsiya qiladi va tengdoshlar orasida tenglikni tasdiqlaydi. | status.md:255 |
| Ruxsat berilgan PRF asosidagi aylanish | ◉ | Ruxsat berilgan yetakchi/kollektor tanlash kanonik ro‘yxat bo‘yicha PRF urug‘i + balandlik/ko‘rinishdan foydalanadi; oldingi xesh aylanishi eski yordamchi bo'lib qoladi. | status.md:so'nggi |

## Quvur liniyasi, Kura va Davlat| Xususiyat | Holati | Eslatmalar | Dalil |
|---------|--------|-------|----------|
| Karantin yo'lakchalari va telemetriya | ◉ | Konfiguratsiya tugmalari, deterministik ortiqcha ishlov berish va telemetriya hisoblagichlari amalga oshirildi. | status.md:263 |
| Quvur liniyasi ishchi hovuz tugmasi | ◉ | `[pipeline].workers` env tahlil qilish testlari bilan boshlang'ich holati orqali o'tkazildi. | status.md:264 |
| Snapshot so'rovlar qatori (saqlangan/efemer kursorlar) | ◉ | Torii integratsiyasi va ishchi hovuzlarini blokirovka qilish bilan saqlangan kursor rejimi. | status.md:265; status.md:371; status.md:501 |
| Statik DAG barmoq izini tiklovchi yonchalar | ◉ | Kurada saqlanadigan yonbosh mashinalar ishga tushirilganda tasdiqlangan, mos kelmasligi haqida ogohlantirishlar berilgan. | status.md:106; status.md:349 |
| Kura blok do'kon hash dekodlash qotib | ◉ | Xesh o'qishlari Norito-mustaqil aylanish testlari bilan 32 baytlik xom ishlovga o'tkazildi. | status.md:608; status.md:668 |
| Norito kodeklar uchun moslashtirilgan telemetriya | ◉ | AoS va NCB tanlov koʻrsatkichlari Norito ga qoʻshildi. | status.md:156 |
| Torii | orqali Snapshot WSV so'rovlari ◉ | Torii snapshot so'rovlar qatori blokirovka qiluvchi ishchilar pulidan, deterministik semantikadan foydalanadi. | status.md:501 |
| Chaqiruv bo'yicha ijro zanjirini ishga tushirish | ◉ | Ma'lumotlar deterministik tartibda qo'ng'iroqlar bajarilgandan so'ng darhol zanjirni ishga tushiradi. | status.md:668 |

## Norito Seriyalashtirish va asboblar

| Xususiyat | Holati | Eslatmalar | Dalil |
|---------|--------|-------|----------|
| Norito JSON migratsiyasi (ish maydoni) | ◉ | Serda ishlab chiqarishdan olib tashlangan; inventar + to'siqlar ish joyini faqat Norito saqlaydi. | status.md:112; status.md:124 |
| Serde deny-list & CI guardrails | ◉ | Guard ish oqimlari/skriptlari ish maydoni bo'ylab yangi to'g'ridan-to'g'ri Serde foydalanishni oldini oladi. | status.md:218 |
| Norito kodek oltin va AoS/NCB testlari | ◉ | AoS/NCB oltin ranglari, kesish testlari va hujjatlarni sinxronlash qo'shildi. | status.md:140-147; status.md:149-150; status.md:332; status.md:666 |
| Norito xususiyatli matritsa asboblari | ◉ | `scripts/run_norito_feature_matrix.sh` quyi oqimdagi tutun sinovlarini qo'llab-quvvatlaydi; CI paketli-seq/struct kombinatsiyalarini qamrab oladi. | status.md:146; status.md:152 |
| Norito til ulanishlari (Python/Java) | ◉ | Python va Java Norito kodeklari sinxronlash skriptlari bilan ishlaydi. | status.md:74; status.md:81 |
| Norito 1-bosqich SIMD tizimli tasniflagichlari | ◉ | NEON/AVX2 bosqich-1 klassifikatorlari ko'ndalang arkali oltin ranglar va tasodifiy korpus testlari. | status.md:241 |

## Boshqaruv va ish vaqtini yangilash| Xususiyat | Holati | Eslatmalar | Dalil |
|---------|--------|-------|----------|
| Ish vaqtini yangilashga kirish (ABI gating) | ◉ | Faol ABI to'plami qabul qilinganda tuzilgan xatolar va testlar bilan amalga oshiriladi. | status.md:196 |
| Himoyalangan nomlar maydonini joylashtirish gating | ▲ | Meta-ma'lumotlarga bo'lgan talablarni va simli o'tkazgichlarni o'rnatish; siyosat/UX hali ham rivojlanmoqda. | status.md:171 |
| Torii boshqaruv so'nggi nuqtalarini o'qish | ◉ | `/v1/gov/*` marshrutizator sinovlari bilan yo'naltirilgan API'larni o'qish. | status.md:212 |
| Tekshirish kaliti registrning hayot aylanishi va hodisalari | ◉ | VK registri/yangilash/bekor qilish, voqealar, CLI filtrlari va saqlash semantikasi amalga oshirildi. | status.md:236-239; status.md:595; status.md:603 |

## Nolinchi bilim infratuzilmasi

| Xususiyat | Holati | Eslatmalar | Dalil |
|---------|--------|-------|----------|
| Qo'shimchalarni saqlash API'lari | ◉ | Deterministik identifikatorlari va testlari bilan `POST/GET/LIST/DELETE` biriktirma so'nggi nuqtalari. | status.md:231 |
| Fon prover ishchi & hisobot TTL | ▲ | Xususiyat bayrog'i orqasida Prover stub; TTL GC va konfiguratsiya tugmalari simli; to'liq quvur liniyasi kutilmoqda. | status.md:212; status.md:233 |
| CoreHost | da konvertni xesh bilan bog'lash ◉ | CoreHost orqali bog'langan va audit impulslari orqali chiqarilgan konvert xeshlarini tekshiring. | status.md:250 |
| Himoyalangan ildiz tarixi gating | ◉ | Chegaralangan tarix va boʻsh ildiz konfiguratsiyasi bilan CoreHost-ga oʻrnatilgan ildiz suratlari. | status.md:303 |
| ZK byulleten ijrosi va boshqaruv qulflari | ○ | Nullifier hosilasi, qulfni yangilash, tekshirish o'tish tugmalari amalga oshirildi; to'liq isbot hayot aylanishi hali etuk. | status.md:126-128; status.md:194-195 |
| Tasdiqlash ilovasi oldindan tekshirish va dedup | ◉ | Backend-teg aql-idrok, deuplikatsiya va isbot yozuvlari ijrodan oldin davom etdi. | status.md:348; status.md:602 |
| ZK Torii isbotlangan so'nggi nuqta | ◉ | `/v1/zk/proof/{backend}/{hash}` isbot yozuvlarini (holat, balandlik, vk_ref/commitment) ochib beradi. | status.md:94 |

## IVM va Kotodama integratsiyasi| Xususiyat | Holati | Eslatmalar | Dalil |
|---------|--------|-------|----------|
| CoreHost syscall→ISI ko'prigi | ○ | Pointer TLV dekodlash va tizim chaqiruvi navbati ishlaydi; qamrov bo'shliqlari/paritet testlari rejalashtirilgan. | status.md:299-307; status.md:477-486 |
| Pointer konstruktorlari va domen konstruksiyalari | ◉ | Kotodama o'rnatilgan qurilmalari Norito TLV va SCALLlarni, IR/e2e testlari va hujjatlarni chiqaradi. | status.md:299-301 |
| Pointer-ABI qat'iy tekshirish va hujjat sinxronlash | ◉ | TLV siyosati xost/IVM boʻylab oltin testlar va yaratilgan hujjatlar bilan qoʻllaniladi. | status.md:227; status.md:317; status.md:344; status.md:366; status.md:527 |
| CoreHost | orqali ZK syscall gating ◉ | Har bir operatsiya uchun navbatlar tekshirilgan konvertlarni o'tkazadi va ISI bajarilishidan oldin xesh mosligini ta'minlaydi. | crates/iroha_core/src/smartcontracts/ivm/host.rs:213; crates/iroha_core/src/smartcontracts/ivm/host.rs:279 |
| Kotodama ko'rsatkichi-ABI hujjatlari va grammatika | ◉ | Grammatika/hujjatlar jonli konstruktorlar va SCALL xaritalari bilan sinxronlangan. | status.md:299-301 |
| ISO 20022 sxemasiga asoslangan dvigatel va Torii ko'prigi | ◉ | Oʻrnatilgan kanonik ISO 20022 sxemalari, deterministik XML tahlili va `/v1/iso20022/status/{MsgId}` API ochilgan. | status.md:65-70 |

## Uskuna tezlashuvi

| Xususiyat | Holati | Eslatmalar | Dalil |
|---------|--------|-------|----------|
| SIMD quyruq/noto'g'ri tenglik testlari | ◉ | Tasodifiy paritet testlari SIMD vektor operatsiyalarini o'zboshimchalik bilan moslashtirish uchun skaler semantikaga mos kelishini ta'minlaydi. | status.md:243 |
| Metall/CUDA zaxira va o'z-o'zini sinovlari | ◉ | GPU backendlari oltin o'z-o'zini sinovdan o'tkazadi va mos kelmasligi sababli skaler/SIMDga qaytadi; paritet to'plamlari SHA-256/Keccak/AES-ni qamrab oladi. | status.md:244-246 |

## Tarmoq vaqti va konsensus rejimlari

| Xususiyat | Holati | Eslatmalar | Dalil |
|---------|--------|-------|----------|
| Tarmoq vaqt xizmati (NTS) | ✖︎ | Dizayn `new_pipeline.md` da mavjud; Amalga oshirish holat yangilanishlarida hali kuzatilmagan. | new_pipeline.md |
| Nominatsiyalangan PoS konsensus rejimi | ✖︎ | Nexus dizayn hujjatlari yopiq to'plam va NPoS rejimlari; asosiy amalga oshirish kutilmoqda. | new_pipeline.md; nexus.md |

## Nexus Ledger yo'l xaritasi| Xususiyat | Holati | Eslatmalar | Dalil |
|---------|--------|-------|----------|
| Space Directory shartnoma iskala | ✖︎ | DS manifestlari/boshqaruvi uchun global registr shartnomasi hali amalga oshirilmagan. | nexus.md |
| Data Space manifest formati va hayot aylanishi | ✖︎ | Norito manifest sxemasi, versiya va boshqaruv oqimi yo'l xaritasida qolmoqda. | nexus.md |
| DS boshqaruvi va validator aylanishi | ✖︎ | DS a'zoligi/aylanish uchun zanjirdagi protseduralar hali dizayn bosqichida. | nexus.md |
| Cross-DS ankraj & Nexus blok tarkibi | ✖︎ | Tarkib qatlami va biriktirish majburiyatlari ko'rsatilgan, ammo amalga oshirilmagan. | nexus.md |
| Kura/WSV o'chirish kodli saqlash | ✖︎ | Ommaviy/xususiy DS uchun oʻchirish kodli blob/snapshot xotirasi hali yaratilmagan. | nexus.md |
| DS boshiga ZK/optimistik isbot siyosati | ✖︎ | Per-DS isboti talablari va ularning bajarilishi kodda kuzatilmaydi. | nexus.md |
| Har bir maʼlumot maydoni uchun toʻlov/kvota izolyatsiyasi | ✖︎ | DS uchun maxsus kvotalar va to'lov siyosati mexanizmlari kelajakdagi ish bo'lib qolmoqda. | nexus.md |

## Betartiblik va nosozliklarni kiritish

| Xususiyat | Holati | Eslatmalar | Dalil |
|---------|--------|-------|----------|
| Izanami xaosnet orkestratsiyasi | ○ | Izanami ish yuki endi yangi yo'llar uchun birlik qamrovi bilan aktivlarni aniqlash, metadata, NFT va tetik-takrorlash retseptlarini boshqaradi. | crates/izanami/src/instructions.rs; crates/izanami/src/instructions.rs#tests |