---
lang: uz
direction: ltr
source: CHANGELOG.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 26f5115a14476de15fbc8f26c5a9807954df6884763a818b2bc98ec6cfe1a4cc
source_last_modified: "2026-01-05T09:28:11.640562+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# O'zgarishlar jurnali

[Unreleased]: https://github.com/hyperledger-iroha/iroha/compare/v2.0.0-rc.2.0...HEAD
[2.0.0-rc.2.0]: https://github.com/hyperledger-iroha/iroha/releases/tag/v2.0.0-rc.2.0

Ushbu loyihaga kiritilgan barcha muhim o'zgarishlar ushbu faylda hujjatlashtiriladi.

## [Nashr qilinmagan]

- SCALE shimini tushiring; `norito::codec` endi mahalliy Norito serializatsiyasi bilan amalga oshirilmoqda.
- `parity_scale_codec` foydalanishni qutilar bo'ylab `norito::codec` bilan almashtiring.
- Asboblarni mahalliy Norito serializatsiyasiga ko'chirishni boshlang.
- Qolgan `parity-scale-codec` bog'liqligini ish maydonidan mahalliy Norito seriyalash foydasiga olib tashlang.
- SCALE belgilarining qoldiq hosilalarini mahalliy Norito ilovalari bilan almashtiring va versiyali kodek modulining nomini o'zgartiring.
- `iroha_config_base_derive` va `iroha_futures_derive`ni xususiyatli makroslar bilan `iroha_derive` ga birlashtiring.
- *(multisig)* Barqaror xato kodi/sabablari bilan multisig organlarining toʻgʻridan-toʻgʻri imzolarini rad eting, oʻrnatilgan relayerlar boʻylab multisig TTL chegaralarini kiriting va yuborishdan oldin CLIda TTL chegaralarini kiriting (SDK pariteti kutilmoqda).
- FFI protsessual makroslarini `iroha_ffi` ichiga ko'chiring va `iroha_ffi_derive` kassasini olib tashlang.
- *(schema_gen)* `iroha_data_model` qaramligidan keraksiz `transparent_api` funksiyasini olib tashlang.
- *(data_model)* Takroriy ishga tushirish xarajatlarini kamaytirish uchun `Name` tahlil qilish uchun ICU NFC normalizatorini keshlang.
- 📚 Torii mijozi uchun JS hujjatini tezkor ishga tushirish, konfiguratsiyani hal qiluvchi, nashr qilish ish jarayoni va konfiguratsiyadan xabardor retsept.
- *(IrohaSwift)* iOS 15 / macOS 12 uchun minimal joylashtirish maqsadlarini oshiring, Torii mijoz API’lari boʻylab Swift parallelligini qabul qiling va ommaviy modellarni `Sendable` sifatida belgilang.
- *(IrohaSwift)* `ToriiDaProofSummaryArtifact` va `DaProofSummaryArtifactEmitter.emit` qoʻshildi, shuning uchun Swift ilovalari CLI-ga oʻtkazmasdan CLI-mos keluvchi DA isbotlovchi toʻplamlarni yaratishi/chiqarishi mumkin, bu hujjatlar va regressiya testlari bilan toʻliq xotirada ham, diskda ham. ish oqimlari.【F:IrohaSwift/Sources/IrohaSwift/ToriiDaProofSummaryArtifact.swift:1】【F:IrohaSwift/Test s/IrohaSwiftTests/ToriiDaProofSummaryArtifactTests.swift:1】【F:docs/source/sdk/swift/index.md:260】
- *(data_model/js_host)* `KaigiParticipantCommitment` dan arxivlangan qayta foydalanish bayrogʻini olib tashlash orqali Kaigi opsiyasini seriyalilashtirishni tuzating, mahalliy aylanish testlarini qoʻshing va JS kodini qayta tiklashni oʻchirib qoʻying, shunda Kaigi koʻrsatmalari hozir Norito aylanish oldidan ishlaydi. topshirish.【F:crates/iroha_data_model/src/kaigi.rs:128】【F:crates/iroha_js_host/src/lib.rs:1379】【F:javascript/iroha_js/test/instructionBuilders.test.js:30【
- *(javascript)* `ToriiClient` qo'ng'iroq qiluvchilarga standart sarlavhalarni o'chirishga ruxsat bering (`null` orqali), `getMetrics` JSON va Prometheus matnlari o'rtasida toza o'tadi Qabul qilaman sarlavhalar.【F:javascript/iroha_js/src/toriiClient.js:488】【F:javascript/iroha_js/src/toriiClient.js:761】
- *(javascript)* NFTlar, har bir hisob aktivlari balanslari va aktivlar taʼrifi egalari (TypeScript taʼriflari, hujjatlar va testlar bilan) uchun takrorlanadigan yordamchilar qoʻshildi, shuning uchun Torii sahifalash endi qolgan ilovani qamrab oladi. so'nggi nuqtalar.【F:javascript/iroha_js/src/toriiClient.js:105】【F:javascript/iroha_js/index.d.ts:8 0】【F:javascript/iroha_js/test/toriiClient.test.js:365】【F:javascript/iroha_js/README.md:470】
- *(javascript)* JS mijozlari takliflar, saylov byulletenlari, qonunlar qabul qilish va kengashning qat'iyatliligini bosqichma-bosqich joylashtirishi uchun boshqaruv bo'yicha ko'rsatmalar/tranzaksiya tuzuvchilari va boshqaruv retsepti qo'shildi. end.【F:javascript/iroha_js/src/instructionBuilders.js:1012】【F:javascript/iroha_js/src/transaction.js:1082】【F:javascript/iroha_js/recipes/governance.mjs:1】
- *(javascript)* ISO 20022 pacs.008 yuborish/status yordamchilari va mos retsept qo‘shildi, bu JS qo‘ng‘iroq qiluvchilarga Torii ISO ko‘prigini buyurtma HTTPsiz ishlatish imkonini beradi. santexnika.【F:javascript/iroha_js/src/toriiClient.js:888】【F:javascript/iroha_js/index.d.ts:706】【F:javascript/iroha_js/recipes/iso_bridge.mjs:
- *(javascript)* pacs.008/pacs.009 quruvchi yordamchilari va konfiguratsiyaga asoslangan retsept qo‘shildi, shuning uchun JS qo‘ng‘iroq qiluvchilar ISO 20022 foydali yuklarini tasdiqlangan BIC/IBAN metama’lumotlari bilan sintez qilishlari mumkin. bridge.【F:javascript/iroha_js/src/isoBridge.js:1】【F:javascript/iroha_js/test/isoBridge.test.js :1】【F:javascript/iroha_js/recipes/iso_bridge_builder.mjs:1】【F:javascript/iroha_js/index.d.ts:1】
- *(javascript)* DA qabul qilish/olish/tasdiqlash davri tugallandi: `ToriiClient.fetchDaPayloadViaGateway` endi chunker tutqichlarini avtomatik ravishda oladi (yangi `deriveDaChunkerHandle` ulanishi orqali), ixtiyoriy isbot xulosalari mahalliy `generateDaProofSummary` va qaytadan foydalanadi. qo'ng'iroq qiluvchilar buyurtmasiz `iroha da get-blob/prove-availability`ni aks ettirishi mumkin santexnika.【F:javascript/iroha_js/src/toriiClient.js:1123】【F:javascript/iroha_js/src/dataAvailability.js:1】【F:javascrip t/iroha_js/test/toriiClient.test.js:1454】【F:javascript/iroha_js/index.d.ts:3275】【F:javascript/iroha_js/README.md:760】
- *(javascript/js_host)* `sorafsGatewayFetch` skorbord meta-ma'lumotlari endi shlyuz provayderlaridan foydalanilganda shlyuz manifest identifikatori/CID-ni qayd qiladi, shuning uchun qabul qilish artefaktlari CLI bilan mos keladi. ushlaydi.【F:crates/iroha_js_host/src/lib.rs:3017】【F:docs/source/sorafs_orchestrator_rollout.md:23】
- *(torii/cli)* ISO piyodalar o‘tish joylarini qo‘llash: Torii endi noma’lum agent BIC’lari bilan `pacs.008` jo‘natmalarini rad etadi va DvP CLI oldindan ko‘rish `--delivery-instrument-id` orqali tasdiqlaydi `--iso-reference-crosswalk`.【F:crates/iroha_torii/src/iso20022_bridge.rs:704】【F:crates/iroha_cli/src/main.rs:3892】
- *(torii)* Qurilishdan oldin `Purp=SECU` va BIC maʼlumotlarini tekshirishni amalga oshirib, `POST /v1/iso20022/pacs009` orqali PvP naqd pulni qoʻshing transferlar.【F:crates/iroha_torii/src/iso20022_bridge.rs:1070】【F:crates/iroha_torii/src/lib.rs:4759】
- *(asboblar)* ISIN/CUSIP, BIC↔LEI va MIC snapshotlarini saqlash uchun tekshirish uchun `cargo xtask iso-bridge-lint` (plyus `ci/check_iso_reference_data.sh`) qoʻshildi armatura.【F:xtask/src/main.rs:146】【F:ci/check_iso_reference_data.sh:1】
- *(javascript)* Repository metamaʼlumotlarini eʼlon qilish orqali qattiqlashtirilgan npm nashri, aniq fayllar ruxsat etilgan roʻyxati, kelib chiqishi yoqilgan `publishConfig`, `prepublishOnly` oʻzgarishlar jurnali/test qoʻriqchisi va 20-sonni mashq qiladigan GitHub Actions ish jarayoni. CI【F:javascript/iroha_js/package.json:1】【F:javascript/iroha_js/scripts/check-changelog .mjs:1】【F:docs/source/sdk/js/publishing.md:1】【F:.github/workflows/javascript-sdk.yml:1】
- *(ivm/cuda)* BN254 maydoni add/sub/mul endi yangi CUDA yadrolarida `bn254_launch_kernel` orqali xost tomonida paketlash bilan bajariladi, bu esa deterministikni saqlab qolgan holda Poseidon va ZK gadjetlari uchun apparat tezlashishiga imkon beradi. zaxiralar.【F:crates/ivm/cuda/bn254.cu:1】【F:crates/ivm/src/cuda.rs:66】【F:crates/ivm/src/cuda.rs:1244】

## [2.0.0-rc.2.0] - 2025-05-08

### 🚀 Xususiyatlar

- *(cli)* `iroha transaction get` va boshqa muhim buyruqlarni qo'shing (#5289)
- [**buzilish**] Alohida qo'zg'atiladigan va o'zgarmas aktivlar (#5308)
- [**buzish**] Bo'sh bo'lmagan bloklarni yakunlang, ulardan keyin bo'sh bloklarga ruxsat bering (#5320)
- Sxema va mijozda telemetriya turlarini ko'rsatish (#5387)
- *(iroha_torii)* Xususiyatga ega so'nggi nuqtalar uchun stublar (#5385)
- Qabul qilish vaqti ko'rsatkichlarini qo'shing (#5380)

### 🐛 Xatolar tuzatildi

- Nol bo'lmaganlarni qayta ko'rib chiqish (№5278)
- Hujjat fayllaridagi matn terish xatolari (№5309)
- *(kripto)* `Signature::payload` oluvchini ochish (#5302) (#5310)
- *(yadro)* Rol borligini tekshirishdan oldin qoʻshing (#5300)
- *(yadro)* Ulangan tengdoshni qayta ulash (#5325)
- Do'kon aktivlari va NFT bilan bog'liq pytestlarni tuzating (№5341)
- *(CI)* She'riyat v2 (#5374) uchun python statik tahlil ish jarayonini tuzating
- Muddati o'tgan tranzaksiya hodisasi bajarilgandan keyin paydo bo'ladi (№5396)

### 💼 Boshqa

- `rust-toolchain.toml` (#5376) qo'shing
- `deny` emas, `unused` da ogohlantiring (#5377)

### 🚜 Refaktor

- Soyabon Iroha CLI (№5282)
- *(iroha_test_network)* Jurnallar uchun chiroyli formatdan foydalaning (#5331)
- [**breaking**] `NumericSpec` ning `genesis.json` (#5340) da ketma-ketligini soddalashtiring
- Muvaffaqiyatsiz p2p ulanishi uchun jurnalni yaxshilash (#5379)
- `logger.level`ni qaytaring, `logger.filter` qo'shing, konfiguratsiya marshrutlarini kengaytiring (#5384)

### 📚 Hujjatlar

- `network.public_address` ni `peer.template.toml` ga qo'shing (#5321)

### ⚡ Ishlash

- *(kura)* diskka ortiqcha blok yozishni oldini olish (#5373)
- Tranzaktsiyalar xeshlari uchun maxsus xotira joriy etildi (№5405)

### ⚙️ Har xil vazifalar

- She'riyatdan foydalanishni tuzatish (#5285)
- `iroha_torii_const` (#5322) dan ortiqcha konstlarni olib tashlang
- Ishlatilmagan `AssetEvent::Metadata*` (#5339) ni olib tashlang
- Bump Sonarqube Action versiyasi (#5337)
- Foydalanilmayotgan ruxsatlarni olib tashlash (#5346)
- ci-image-ga unzip paketini qo'shing (#5347)
- Ba'zi izohlarni tuzatish (#5397)
- Integratsiya testlarini `iroha` qutisidan olib tashlang (№5393)
- Defectdojo ishini o'chirib qo'ying (№5406)
- etishmayotgan majburiyatlar uchun DCO imzosini qo'shing
- Ish oqimlarini qayta tashkil qilish (ikkinchi urinish) (#5399)
- Asosiyga surishda Pull Request CI-ni ishga tushirmang (#5415)

<!-- generated by git-cliff -->

## [2.0.0-rc.1.3] - 2025-03-07

### Qo'shilgan

- bo'sh bo'lmagan bloklarni ulardan keyin bo'sh bloklarga ruxsat berish orqali yakunlang (#5320)

## [2.0.0-rc.1.2] - 2025-02-25

### Tugallangan

- qayta ro'yxatdan o'tgan tengdoshlar endi tengdoshlar ro'yxatida to'g'ri aks ettirilgan (№5327)

## [2.0.0-rc.1.1] - 2025-02-12

### Qo'shilgan

- `iroha transaction get` va boshqa muhim buyruqlarni qo'shing (#5289)

## [2.0.0-rc.1.0] - 2024-12-06

### Qo'shilgan- so'rov prognozlarini amalga oshirish (#5242)
- doimiy ijrochidan foydalaning (#5082)
- iroha cli-ga tinglash vaqtini qo'shing (#5241)
- torii-ga /peers API so'nggi nuqtasini qo'shing (#5235)
- agnostik p2p manzili (№5176)
- multisig yordam dasturi va foydalanish qulayligini yaxshilash (#5027)
- `BasicAuth::password`ni chop etishdan himoya qiling (#5195)
- `FindTransactions` so'rovida kamayish bo'yicha tartiblang (#5190)
- har bir aqlli shartnomani bajarish kontekstiga blok sarlavhasini kiriting (#5151)
- ko'rinishni o'zgartirish indeksiga asoslangan dinamik majburiyat vaqti (#4957)
- standart ruxsatlar to'plamini aniqlang (#5075)
- `Option<Box<R>>` uchun Niche ilovasini qo'shing (#5094)
- tranzaksiya va blok predikatlar (№5025)
- so'rovdagi qolgan elementlar miqdori haqida hisobot berish (№5016)
- cheklangan diskret vaqt (#4928)
- `Numeric` (#4976) ga etishmayotgan matematik amallarni qo'shing
- bloklangan sinxronlash xabarlarini tasdiqlash (#4965)
- so'rov filtrlari (#4833)

### O'zgartirildi

- tengdosh identifikatorini tahlil qilishni soddalashtiring (#5228)
- tranzaksiya xatosini blokning foydali yukidan chiqarish (#5118)
- JsonString nomini Json (#5154) ga o'zgartiring
- aqlli shartnomalarga mijoz ob'ektini qo'shing (#5073)
- tranzaktsiyalarni buyurtma qilish xizmati sifatida etakchi (№4967)
- eski bloklarni xotiradan kura tashlab yuborish (#5103)
- `Executable` (#5096) dagi ko'rsatmalar uchun `ConstVec` dan foydalaning
- ko'pi bilan bir marta g'iybat qiling (#5079)
- `CommittedTransaction` xotiradan foydalanishni kamaytirish (#5089)
- so'rov kursoridagi xatolarni aniqroq qilish (#5086)
- qutilarni qayta tashkil qilish (#4970)
- `FindTriggers` so'rovini kiriting, `FindTriggerById` (#5040) olib tashlang
- yangilash uchun imzolarga bog'liq emas (#5039)
- genesis.json da parametrlar formatini o'zgartirish (#5020)
- faqat joriy va oldingi ko'rinishni o'zgartirish isbotini yuboring (#4929)
- band bo'lmagan tsiklni oldini olishga tayyor bo'lmaganda xabar yuborishni o'chirib qo'ying (#5032)
- aktivlarning umumiy miqdorini aktivlar taʼrifiga oʻtkazing (№5029)
- butun foydali yukni emas, faqat blok sarlavhasini belgilang (#5000)
- blok xesh turi sifatida `HashOf<BlockHeader>` dan foydalaning (#4998)
- `/health` va `/api_version` (#4960) soddalashtirish
- `configs` nomini `defaults` deb o'zgartiring, `swarm` (#4862) olib tashlang

### Tugallangan

- json-da ichki rolni tekislang (#5198)
- `cargo audit` ogohlantirishlarini tuzatish (#5183)
- imzo indeksiga diapazon tekshiruvini qo'shing (#5157)
- hujjatlardagi model makro misolini tuzatish (#5149)
- bloklar/hodisalar oqimida ws-ni to'g'ri yoping (#5101)
- buzilgan ishonchli tengdoshlar tekshiruvi (№5121)
- keyingi blokning balandligi +1 ekanligini tekshiring (#5111)
- genezis blokining vaqt tamg'asini tuzatish (#5098)
- `iroha_genesis` kompilyatsiyasini `transparent_api` xususiyatisiz tuzatish (#5056)
- `replace_top_block` (#4870) to'g'ri ishlov berish
- ijrochini klonlashni tuzatish (#4955)
- xato tafsilotlarini ko'rsatish (#4973)
- bloklar oqimi uchun `GET` dan foydalaning (#4990)
- navbatdagi tranzaktsiyalarni boshqarishni yaxshilash (#4947)
- ortiqcha blokirovkali blokirovka xabarlarini oldini olish (#4909)
- bir vaqtning o'zida katta hajmdagi xabarlarni yuborishda blokirovkaning oldini olish (#4948)
- muddati o'tgan tranzaksiyani keshdan olib tashlash (#4922)
- yo'l bilan torii urlni tuzatish (#4903)

### O'chirildi

- mijozdan modulga asoslangan api-ni olib tashlang (#5184)
- `riffle_iter` (#5181) olib tashlang
- foydalanilmagan bog'liqliklarni olib tashlang (№ 5173)
- `max` prefiksini `blocks_in_memory` dan olib tashlang (#5145)
- konsensus bahosini olib tashlash (#5116)
- `event_recommendations`ni blokdan olib tashlang (#4932)

### Xavfsizlik

## [2.0.0-pre-rc.22.1] - 2024-07-30

### Tugallangan

- docker tasviriga `jq` qo'shildi

## [2.0.0-pre-rc.22.0] - 2024-07-25

### Qo'shilgan

- genezisda zanjirdagi parametrlarni aniq belgilang (#4812)
- bir nechta `Instruction` (#4805) bilan turbobaliqlarga ruxsat bering
- ko'p imzoli tranzaktsiyalarni qayta tiklash (#4788)
- o'rnatilgan va maxsus zanjirdagi parametrlarni amalga oshirish (#4731)
- maxsus ko'rsatmalardan foydalanishni yaxshilash (#4778)
- JsonString (#4732) ni qo'llash orqali metama'lumotlarni dinamik qilish
- bir nechta tengdoshlarga genezis blokini yuborishga ruxsat bering (#4775)
- tengdoshga `SignedTransaction` o'rniga `SignedBlock` yetkazib bering (#4739)
- ijrochida maxsus ko'rsatmalar (#4645)
- json so'rovlarini so'rash uchun mijoz cli-ni kengaytiring (#4684)
 - `norito_decoder` (#4680) uchun aniqlash yordamini qo'shing
- ijrochi ma'lumotlar modeliga ruxsatlar sxemasini umumlashtirish (#4658)
- standart ijrochida registrni ishga tushirish uchun ruxsatlar qo'shildi (#4616)
 - `norito_cli` da JSON-ni qo'llab-quvvatlash
- p2p bo'sh turish vaqtini kiritish

### O'zgartirildi

- `lol_alloc`ni `dlmalloc` bilan almashtiring (#4857)
- sxemada `type_` nomini `type` ga o'zgartiring (#4855)
- sxemada `Duration` ni `u64` bilan almashtiring (#4841)
- jurnalga yozish uchun `RUST_LOG` kabi EnvFilterdan foydalaning (#4837)
- iloji bo'lsa ovoz berish blokini saqlang (#4828)
- warpdan axumga o'tish (#4718)
- split ijrochi ma'lumotlar modeli (#4791)
- sayoz ma'lumotlar modeli (#4734) (#4792)
- ochiq kalitni imzo bilan yubormang (#4518)
- `--outfile` nomini `--out-file` (#4679) ga o'zgartiring
- iroha server va mijoz nomini o'zgartirish (#4662)
- `PermissionToken` nomini `Permission` (#4635) ga o'zgartiring
- `BlockMessages`ni ishtiyoq bilan rad qiling (#4606)
- `SignedBlock` ni o'zgarmas qilish (#4620)
- TransactionValue nomini CommittedTransaction (#4610) ga o'zgartiring
- shaxsiy hisoblarni ID (#4411) bo'yicha autentifikatsiya qilish
- shaxsiy kalitlar uchun multihash formatidan foydalaning (#4541)
 - `parity_scale_decoder` nomini `norito_cli` ga o'zgartiring
- Set B validatorlariga bloklarni yuborish
- `Role` ni shaffof qilish (#4886)
- sarlavhadan blok xeshini olish (#4890)

### Tugallangan

- o'tkazish uchun vakolatli domenga ega ekanligini tekshiring (№4807)
- loggerning ikki marta ishga tushirilishini olib tashlang (#4800)
- aktivlar va ruxsatlar uchun nomlash konventsiyasini tuzatish (#4741)
- genezis blokidagi alohida tranzaksiyada ijrochini yangilash (#4757)
- `JsonString` uchun to'g'ri standart qiymat (#4692)
- deserializatsiya xato xabarini yaxshilash (#4659)
- agar uzatilgan Ed25519Sha512 ochiq kaliti yaroqsiz uzunlikda bo'lsa, vahima qo'ymang (#4650)
- init blok yuklashda to'g'ri ko'rinishni o'zgartirish indeksidan foydalaning (#4612)
- `start` vaqt tamg'asidan oldin vaqt triggerlarini muddatidan oldin bajarmang (#4333)
- `torii_url` (#4601) (#4617) uchun `https`-ni qo'llab-quvvatlash
- SetKeyValue/RemoveKeyValue (#4547) dan serde (tekislash) olib tashlang
- trigger to'plami to'g'ri ketma-ketlashtirilgan
- `Upgrade<Executor>` (#4503) da olib tashlangan `PermissionToken`larni bekor qilish
- joriy tur uchun to'g'ri ko'rinish o'zgarishi indeksini xabar qilish
- `Unregister<Domain>` (#4461) da tegishli triggerlarni olib tashlang
- genesis raundida genesis pub kalitini tekshiring
- genezis domenini yoki hisob qaydnomasini ro'yxatdan o'tkazishni oldini olish
- ob'ektni ro'yxatdan o'chirish bo'yicha rollardan ruxsatlarni olib tashlash
- trigger metama'lumotlariga aqlli shartnomalarda kirish mumkin
- nomuvofiq holat ko'rinishini oldini olish uchun rw qulfidan foydalaning (#4867)
- snapshotda yumshoq vilka bilan ishlov berish (#4868)
- ChaCha20Poly1305 uchun MinSize-ni tuzatish
- yuqori xotiradan foydalanishni oldini olish uchun LiveQueryStore-ga cheklovlar qo'shing (#4893)

### O'chirildi

- ed25519 shaxsiy kalitidan ochiq kalitni olib tashlang (#4856)
- kura.lock-ni o'chirish (#4849)
- konfiguratsiyada `_ms` va `_bytes` qo'shimchalarini qaytaring (#4667)
- genezis maydonlaridan `_id` va `_file` qo'shimchasini olib tashlang (#4724)
- AssetDefinitionId (#4701) bo'yicha AssetsMap-dagi aktivlar indeksini olib tashlang
- trigger identifikatoridan domenni olib tashlash (#4640)
- Iroha (#4673) dan genezis imzosini olib tashlang
- `Visit` ni `Validate` dan olib tashlang (#4642)
- `TriggeringEventFilterBox` (#4866) olib tashlang
- p2p qo'l siqishda `garbage`ni olib tashlang (#4889)
- `committed_topology`ni blokdan olib tashlang (#4880)

### Xavfsizlik

- sirlarning tarqalishidan ehtiyot bo'ling

## [2.0.0-pre-rc.21] - 2024-04-19

### Qo'shilgan

- trigger identifikatorini trigger kirish nuqtasiga kiriting (#4391)
- sxemada bit maydonlari sifatida o'rnatilgan hodisani ko'rsatish (#4381)
- donador kirish imkoniyatiga ega yangi `wsv`ni taqdim eting (#2664)
- `PermissionTokenSchemaUpdate`, `Configuration` va `Executor` hodisalari uchun hodisa filtrlarini qo'shing
- suratga olish "rejimini" joriy etish (#4365)
- rol ruxsatlarini berishga/bekor qilishga ruxsat berish (#4244)
- aktivlar uchun ixtiyoriy aniqlikdagi raqamli turni kiritish (barcha boshqa raqamli turlarini olib tashlash) (#3660)
- Ijrochi uchun boshqa yoqilg'i chegarasi (#3354)
- pprof profilerni birlashtirish (#4250)
- mijoz CLI-ga aktiv quyi buyrug'ini qo'shing (#4200)
- `Register<AssetDefinition>` ruxsatnomalari (#4049)
- takroriy hujumlarning oldini olish uchun `chain_id` qo'shing (#4185)
- CLI mijozida domen metama'lumotlarini tahrirlash uchun kichik buyruqlar qo'shing (#4175)
- Client CLI-da do'konlar to'plamini amalga oshirish, olib tashlash, operatsiyalarni olish (#4163)
- triggerlar uchun bir xil aqlli kontraktlarni sanash (#4133)
- domenlarni uzatish uchun mijoz CLI-ga kichik buyruq qo'shing (№3974)
- FFI-da qutilangan bo'laklarni qo'llab-quvvatlash (#4062)
- git CLI mijoziga SHAni topshiradi (#4042)
- standart validator qozon plitasi uchun proc makros (#3856)
- Client API-ga so'rovlar so'rovi yaratuvchisi kiritildi (№3124)
- aqlli kontraktlar ichidagi dangasa so'rovlar (#3929)
- `fetch_size` so'rov parametri (#3900)
- aktivlar do'konini o'tkazish bo'yicha yo'riqnoma (№4258)
- sirlarning tarqalishidan ehtiyot bo'ling (№3240)
- bir xil manba kodi bilan tekinlashtirilgan triggerlar (#4419)

### O'zgartirildi- bump zang asboblar zanjiri 2024-04-18 tunda
- bloklarni B to'plami validatorlariga yuborish (#4387)
- quvur hodisalarini blok va tranzaksiya hodisalariga bo'lish (#4366)
- `[telemetry.dev]` konfiguratsiya bo'limi nomini `[dev_telemetry]` (#4377) ga o'zgartiring
- `Action` va `Filter` umumiy bo'lmagan turlarini yaratish (#4375)
- quruvchi naqsh bilan hodisalarni filtrlash API-ni yaxshilash (#3068)
- turli hodisalar filtri API-larini birlashtirish, ravon quruvchi API-ni joriy qilish
- `FilterBox` nomini `EventFilterBox` ga o'zgartiring
- `TriggeringFilterBox` nomini `TriggeringEventFilterBox` ga o'zgartiring
- filtr nomini yaxshilash, masalan. `AccountFilter` -> `AccountEventFilter`
- konfiguratsiyani RFC konfiguratsiyasiga muvofiq qayta yozish (#4239)
- umumiy API dan versiya tuzilmalarining ichki tuzilishini yashirish (#3887)
- ko'rinishdagi juda ko'p muvaffaqiyatsiz o'zgarishlardan keyin oldindan bashorat qilinadigan tartibni vaqtincha joriy qilish (#4263)
- `iroha_crypto` (#4181) da aniq kalit turlaridan foydalaning
- oddiy xabarlardan ajratilgan ko'rinishdagi o'zgarishlar (#4115)
- `SignedTransaction` ni o'zgarmas qilish (#4162)
- `iroha_config`ni `iroha_client` orqali eksport qilish (#4147)
- `iroha_crypto`ni `iroha_client` orqali eksport qilish (#4149)
- `data_model`ni `iroha_client` orqali eksport qilish (#4081)
- `openssl-sys` bog'liqligini `iroha_crypto` dan olib tashlang va `iroha_client` (#3422) ga sozlanishi tls backendlarini kiriting
- ta'mirlanmagan EOF `hyperledger/ursa` o'rniga `iroha_crypto` (#3422) ichki yechimi
- ijrochining ishlashini optimallashtirish (#4013)
- topologiyani yangilash (#3995)

### Tugallangan

- `Unregister<Domain>` (#4461) da tegishli triggerlarni olib tashlang
- ob'ektni ro'yxatdan o'chirish bo'yicha rollardan ruxsatlarni olib tashlash (#4242)
- genezis tranzaktsiyasi genesis pub kaliti bilan imzolanganligini tasdiqlang (#4253)
- p2p da javob bermayotgan tengdoshlar uchun vaqt tugashini kiritish (#4267)
- genezis domenini yoki hisob qaydnomasini ro'yxatdan o'tkazishni oldini olish (№4226)
- `ChaCha20Poly1305` uchun `MinSize` (#4395)
- `tokio-console` yoqilganda konsolni ishga tushirish (#4377)
- har bir elementni `\n` bilan ajratib oling va `dev-telemetry` fayl jurnallari uchun rekursiv ravishda ota-kataloglarni yarating.
- imzosiz hisobni ro'yxatdan o'tkazishning oldini olish (№4212)
- kalit juftligini yaratish endi xatosiz (#4283)
- `X25519` kalitlarini `Ed25519` (#4174) sifatida kodlashni to'xtatish
- `no_std` (#4270) da imzoni tekshirishni amalga oshiring
- asinxron kontekstda blokirovka usullarini chaqirish (#4211)
- ob'ektni ro'yxatdan o'chirishda tegishli tokenlarni bekor qilish (№3962)
- Sumeragi ishga tushirilganda asinxron blokirovka xatosi
- qattiq `(get|set)_config` 401 HTTP (#4177)
- `musl` arxivator nomi Docker (#4193)
- kontraktni tuzatish uchun aqlli chop etish (#4178)
- qayta ishga tushirilganda topologiyani yangilash (#4164)
- yangi tengdoshni ro'yxatdan o'tkazish (#4142)
- zanjirdagi bashorat qilinadigan iteratsiya tartibi (#4130)
- logger va dinamik konfiguratsiyani qayta tiklash (#4100)
- trigger atomikligi (#4106)
- so'rovlar do'koni xabarlarini buyurtma qilish muammosi (№4057)
- Norito yordamida javob beradigan so'nggi nuqtalar uchun `Content-Type: application/x-norito` ni o'rnating

### O'chirildi

- `logger.tokio_console_address` konfiguratsiya parametri (#4377)
- `NotificationEvent` (#4377)
- `Value` raqam (#4305)
- Irohadan MST yig'ilishi (#4229)
- ISI uchun klonlash va aqlli shartnomalarda so'rovlarni bajarish (#4182)
- `bridge` va `dex` xususiyatlari (#4152)
- tekislangan hodisalar (#3068)
- ifodalar (#4089)
- avtomatik yaratilgan konfiguratsiya ma'lumotnomasi
- Jurnallardagi shovqin `warp` (#4097)

### Xavfsizlik

- p2p da (#4065) pub kalitini buzishning oldini olish
- OpenSSL-dan chiqadigan `secp256k1` imzolari normallashtirilganligiga ishonch hosil qiling (#4155)

## [2.0.0-pre-rc.20] - 2023-10-17

### Qo'shilgan

- `Domain` egalik huquqini o'tkazing
- `Domain` egalariga ruxsatlar
- `owned_by` maydonini `Domain` ga qo'shing
- `iroha_client_cli` da JSON5 sifatida filtrni ajratish (#3923)
- Serde qisman teglangan enumlarda Self turidan foydalanishni qo'llab-quvvatlang
- API bloklarini standartlashtirish (#3884)
- `Fast` kura init rejimini amalga oshiring
- iroha_swarm rad etish sarlavhasini qo'shing
- WSV snapshotlari uchun dastlabki yordam

### Tugallangan

- update_configs.sh (#3990) da ijrochini yuklab olishni tuzatish
- devShell-da to'g'ri rustc
- `Trigger` rertitions kuyishini tuzatish
- `AssetDefinition` uzatishni tuzatish
- `Domain` uchun `RemoveKeyValue` tuzatish
- `Span::join` foydalanishni to'g'irlang
- Topologiyaning mos kelmasligi xatosini tuzatdi (#3903)
- `apply_blocks` va `validate_blocks` mezonlarini tuzating
- `mkdir -r` do'kon yo'li bilan, qulflash yo'li (#3908)
- Agar test_env.py da dir mavjud bo'lsa, muvaffaqiyatsiz bo'lmang
- Autentifikatsiya/avtorizatsiya docstringini tuzatish (#3876)
- So'rovni topish xatosi uchun yaxshiroq xato xabari
- Dev docker kompozitsiyasiga genezis hisobining ochiq kalitini qo'shing
- Ruxsat tokenining foydali yukini JSON (#3855) sifatida solishtiring
- `#[model]` makrosida `irrefutable_let_patterns` ni tuzatish
- Genesisga har qanday ISIni bajarishga ruxsat bering (#3850)
- Genesis tekshiruvini tuzatish (#3844)
- 3 yoki undan kam tengdoshlar uchun topologiyani tuzatish
- tx_amounts gistogrammasi qanday hisoblanganligini to'g'rilang.
- `genesis_transactions_are_validated()` testi silliqligi
- Standart validator yaratish
- Iroha oqlangan o'chirishni tuzatish

### Refaktor

- foydalanilmagan bog'liqliklarni olib tashlash (#3992)
- zarbaga bog'liqliklar (#3981)
- Validator nomini ijrochiga o'zgartirish (№3976)
- `IsAssetDefinitionOwner` (#3979) olib tashlang
- Ish maydoniga aqlli shartnoma kodini qo'shing (#3944)
- API va Telemetriya so'nggi nuqtalarini bitta serverga birlashtiring
- iborani umumiy API dan yadroga ko'chiring (#3949)
- Rollarni qidirishda klonlashdan saqlaning
- Rollar uchun diapazon so'rovlari
- Hisob rollarini `WSV` ga o'tkazing
- ISI nomini *Box dan *Expr ga o'zgartiring (#3930)
- Versiyalangan konteynerlardan "Versioned" prefiksini olib tashlang (#3913)
- `commit_topology`-ni blokning foydali yukiga o'tkazing (#3916)
- `telemetry_future` makrosini syn 2.0 ga o'tkazing
- ISI chegaralarida Identifiable bilan ro'yxatdan o'tgan (№3925)
- `derive(HasOrigin)` ga asosiy generik yordamni qo'shing
- Klipni xursand qilish uchun Emitent API hujjatlarini tozalang
- Derive(HasOrigin) makrosi uchun testlarni qo'shing, derive(IdEqOrdHash) da takrorlashni kamaytiring, barqaror xato haqida hisobotni tuzating
- Nomlashni yaxshilang, takroriy .filter_mapsni soddalashtiring va derive(Filter) dan tashqari keraksiz .
- Qisman TaggedSerialize/Deserialize foydalaning azizim
- Darling (IdEqOrdHash) dan foydalaning, testlarni qo'shing
- Darling (Filtrni) ishlating
- Syn 2.0 dan foydalanish uchun iroha_data_model_derive-ni yangilang
- Imzoni tekshirish holati birligi testlarini qo'shing
- Imzoni tekshirish shartlarining faqat qat'iy to'plamiga ruxsat bering
- ConstBytes-ni har qanday const ketma-ketligini saqlaydigan ConstVec-ga umumlashtiring
- O'zgarmas bayt qiymatlari uchun samaraliroq tasvirdan foydalaning
- Yakunlangan wsv-ni oniy rasmda saqlang
- `SnapshotMaker` aktyorini qo'shing
- proc makroslarida hosilalarni tahlil qilishning hujjat cheklanishi
- sharhlarni tozalash
- lib.rs atributlarini tahlil qilish uchun umumiy sinov yordam dasturini chiqarib oling
- parse_display dan foydalaning va Attr -> Attrs nomlarini yangilang
- ffi args funktsiyasida naqsh moslashuvidan foydalanishga ruxsat berish
- getset attrs tahlilida takrorlashni kamaytirish
- Emitter::into_token_stream nomini Emitter::finish_token_streamga o‘zgartirish
- Getset tokenlarini tahlil qilish uchun parse_display dan foydalaning
- Yozuv xatolarini tuzating va xato xabarlarini yaxshilang
- iroha_ffi_derive: atributlarni tahlil qilish uchun darling-dan foydalaning va syn 2.0 dan foydalaning
- iroha_ffi_derive: proc-makro-xatoni manyhow bilan almashtiring
- Kura lock fayl kodini soddalashtiring
- barcha raqamli qiymatlarni satr harflari sifatida ketma-ketlashtiring
- Kagami (#3841) ajrating
- `scripts/test-env.sh` ni qayta yozing
- Aqlli kontrakt va tetikli kirish nuqtalarini farqlang
- `data_model/src/block.rs` da Elide `.cloned()`
- syn 2.0 dan foydalanish uchun `iroha_schema_derive` ni yangilang

## [2.0.0-pre-rc.19] - 2023-08-14

### Qo'shilgan

- hyperledger#3309 Bump IVM ish vaqti yaxshilanishi uchun
- hyperledger#3383 Kompilyatsiya vaqtida rozetka manzillarini tahlil qilish uchun makrosni qo'llang
- hyperledger#2398 So'rov filtrlari uchun integratsiya testlarini qo'shing
- Haqiqiy xato xabarini `InternalError` ga qo'shing
- Standart asboblar zanjiri sifatida `nightly-2023-06-25` dan foydalanish
- hyperledger#3692 Validator migratsiyasi
- [DSL internship] hyperledger # 3688: asosiy arifmetikani prok makrosi sifatida amalga oshirish
- hyperledger#3371 Validatorlar endi smart-kontrakt sifatida ko'rilmasligini ta'minlash uchun `entrypoint` ajratilgan validator
- giperledger # 3651 WSV snapshotlari, bu Iroha tugunini avariyadan keyin tezda tiklashga imkon beradi
- hyperledger#3752 `MockValidator` ni barcha tranzaksiyalarni qabul qiluvchi `Initial` validator bilan almashtiring
- hyperledger#3276 Iroha tugunining asosiy jurnaliga belgilangan qatorni qayd qiluvchi `Log` nomli vaqtinchalik koʻrsatma qoʻshing
- hyperledger#3641 Ruxsat tokenining foydali yukini odamlar o'qishi mumkin bo'lsin
- hyperledger#3324 `iroha_client_cli` bilan bog'liq `burn` tekshiruvlari va refaktoring qo'shing
- hyperledger#3781 Genesis tranzaksiyalarini tasdiqlash
- hyperledger#2885 Triggerlar uchun ishlatilishi mumkin bo'lgan va mumkin bo'lmagan hodisalarni farqlang
- hyperledger#2245 `Nix` asosidagi iroha tugunining ikkilik tuzilishi `AppImage`

### Tugallangan- noto'g'ri imzolangan tranzaktsiyalarni qabul qilish imkonini beradigan giperledger#3613 regressiya
- Noto'g'ri konfiguratsiya topologiyasini erta rad etish
- hyperledger#3445 Regressiyani tuzating va `/configuration` so'nggi nuqtasida `POST` ni qaytadan ishga tushiring
- hyperledger # 3654 `iroha2` `glibc` asosidagi `Dockerfiles` ni oʻrnatish uchun tuzatish
- hyperledger # 3451 Apple silikon Mac kompyuterlarida tuzilgan `docker` tuzatmasi
- hyperledger #3741 `kagami validator` da `tempfile` xatosini tuzatish
- hyperledger#3758 regressiyani tuzatdi, bunda alohida qutilarni qurish mumkin emas, lekin ish maydonining bir qismi sifatida qurish mumkin
- hyperledger#3777 Rollarni ro'yxatdan o'tkazishda yamoq bo'shlig'i tasdiqlanmagan
- hyperledger#3805 `SIGTERM` qabul qilingandan so'ng Iroha o'chmasligini tuzatish

### Boshqa

- hyperledger # 3648 CI jarayonlariga `docker-compose.*.yml` tekshiruvini qo'shing
- `len()` ko'rsatmasini `iroha_data_model` dan `iroha_core` ga o'tkazing
- hyperledger#3672 `HashMap` ni hosila makrosida `FxHashMap` bilan almashtiring
- hyperledger # 3374 Xatoning hujjat sharhlarini birlashtirish va `fmt::Display` amalga oshirish
- hyperledger#3289 Loyiha davomida Rust 1.70 ish maydoni merosidan foydalaning
- hyperledger#3654 `GNU libc <https://www.gnu.org/software/libc/>`_ da iroha2 qurish uchun `Dockerfiles` ni qo'shing
- Proc-makroslar uchun `syn` 2.0, `manyhow` va `darling` ni joriy qilish
- hyperledger # 3802 Unicode `kagami crypto` urug'i

## [2.0.0-pre-rc.18]

### Qo'shilgan

- hyperledger # 3468: server tomoni kursori, so'rovlar kechikishiga katta ijobiy ta'sir ko'rsatishi kerak bo'lgan dangasalik bilan baholangan qayta sahifalash imkonini beradi
- hyperledger # 3624: Umumiy maqsadlar uchun ruxsat tokenlari; maxsus
  - Ruxsat tokenlari har qanday tuzilishga ega bo'lishi mumkin
  - Token tuzilishi `iroha_schema` da oʻzini oʻzi tasvirlab beradi va JSON qatori sifatida seriyalashtiriladi
  - Token qiymati `Norito` kodlangan
  - ushbu o'zgarish natijasida ruxsat tokenini nomlash konventsiyasi `snake_case` dan `UpeerCamelCase` ga ko'chirildi
- hyperledger # 3615 Tekshiruvdan so'ng wsv ni saqlang

### Tugallangan

- hyperledger#3627 Tranzaksiya atomligi endi `WorlStateView` klonlash orqali amalga oshiriladi
- hyperledger#3195 Rad etilgan genezis tranzaksiyani qabul qilishda vahima qo'zg'atishni kengaytiring
- hyperledger#3042 Noto'g'ri so'rov xabarini tuzatish
- hyperledger#3352 Boshqaruv oqimi va ma'lumotlar xabarini alohida kanallarga bo'lish
- hyperledger#3543 Ko'rsatkichlar aniqligini oshirish

## 2.0.0-pre-rc.17

### Qo'shilgan

- hyperledger#3330 `NumericValue` seriyani bekor qilishni kengaytirish
- hyperledger#2622 `u128`/`i128` FFI-da qo'llab-quvvatlash
- hyperledger#3088 DoS oldini olish uchun navbatni qisqartirishni joriy qiling
- hyperledger#2373 `kagami swarm file` va `kagami swarm dir` `docker-compose` fayllarini yaratish uchun buyruq variantlari
- hyperledger # 3597 Ruxsat token tahlili (Iroha tomoni)
- hyperledger#3353 Xato holatlarini sanash va qattiq yozilgan xatolardan foydalanish orqali `eyre` ni `block.rs` dan olib tashlang
- hyperledger#3318 Tranzaktsiyalarni qayta ishlash tartibini saqlab qolish uchun rad etilgan va qabul qilingan tranzaktsiyalarni bloklarga qo'ying.

### Tugallangan

- hyperledger # 3075 `genesis.json` da noto'g'ri tranzaksiyada vahima, yaroqsiz tranzaksiyalarni qayta ishlashning oldini olish uchun
- hyperledger#3461 Standart konfiguratsiyada standart qiymatlarni to'g'ri ishlatish
- hyperledger#3548 `IntoSchema` shaffof atributini tuzatish
- hyperledger#3552 Validator yo'l sxemasi ko'rinishini tuzatish
- hyperledger#3546 Vaqt triggerlarining tiqilib qolishini tuzatish
- hyperledger#3162 Blok oqimi so'rovlarida 0 balandlikni taqiqlash
- Konfiguratsiya makrosining dastlabki sinovi
- hyperledger#3592 `release` da yangilanadigan konfiguratsiya fayllari uchun tuzatish
- hyperledger#3246 `fault <https://en.wikipedia.org/wiki/Byzantine_fault>`_ holda `Set B validators <https://github.com/hyperledger-iroha/iroha/blob/main/docs/source/iroha_2_whitepaper.md#2-system-architecture>`_ ni jalb qilmang
- hyperledger#3570 Mijoz tomonidagi qator so'rovlari xatolarini to'g'ri ko'rsatish
- hyperledger # 3596 `iroha_client_cli` bloklar/hodisalar ko'rsatadi
- hyperledger#3473 `kagami validator` ni iroha ombori ildiz katalogidan tashqarida ishlashga majbur qiling

### Boshqa

- hyperledger #3063 `wsv` da balandlikni bloklash uchun `hash` tranzaksiyasi xaritasi
- `Value` da qattiq yozilgan `HashOf<T>`

## [2.0.0-pre-rc.16]

### Qo'shilgan

- hyperledger #2373 `kagami swarm` `docker-compose.yml` ni yaratish uchun kichik buyruq
- hyperledger#3525 Tranzaksiya API-ni standartlashtirish
- hyperledger#3376 Iroha Client CLI `pytest <https://docs.pytest.org/en/7.4.x/>`_ avtomatlashtirish ramkasini qo'shish
- hyperledger#3516 `LoadedExecutable` da asl blob xeshni saqlang

### Tugallangan

- hyperledger#3462 `burn` aktiv buyrug'ini `client_cli` ga qo'shing
- hyperledger#3233 Refaktor xato turlari
- hyperledger#3330 `partially-tagged <https://serde.rs/enum-representations.html>`_ `enums` uchun `serde::de::Deserialize` ni qo'lda amalga oshirish orqali regressiyani tuzating
- hyperledger#3487 Sxemaga etishmayotgan turlarni qaytaring
- hyperledger#3444 Diskriminantni sxemaga qaytarish
- hyperledger # 3496 `SocketAddr` maydon tahlilini tuzatish
- hyperledger#3498 Yumshoq vilkalarni aniqlashni tuzatish
- hyperledger#3396 Bloklangan hodisani chiqarishdan oldin blokni `kura` da saqlang

### Boshqa

- hyperledger#2817 `WorldStateView` dan ichki o'zgaruvchanlikni olib tashlang
- hyperledger # 3363 Genesis API refaktori
- Mavjudni qayta tiklash va topologiya uchun yangi testlar bilan to'ldirish
- Test qamrovi uchun `Codecov <https://about.codecov.io/>`_ dan `Coveralls <https://coveralls.io/>`_ ga o'tish
- hyperledger#3533 `Bool` nomini sxemada `bool` ga o'zgartiring

## [2.0.0-pre-rc.15]

### Qo'shilgan

- hyperledger # 3231 Monolitik validator
- hyperledger # 3015 FFIda niche optimallashtirishni qo'llab-quvvatlash
- hyperledger#2547 `AssetDefinition` ga logotip qo'shing
- hyperledger#3274 `kagami` ga misollar yaratuvchi kichik buyruqni qo'shing (LTS-ga aks ettirilgan)
- giperledger #3415 `Nix <https://nixos.wiki/wiki/Flakes>`_ parcha
- hyperledger#3412 Tranzaksiya g'iybatini alohida aktyorga o'tkazing
- hyperledger#3435 `Expression` mehmonini tanishtirish
- hyperledger#3168 Genesis validatorni alohida fayl sifatida taqdim eting
- hyperledger#3454 Ko'pgina Docker operatsiyalari va hujjatlari uchun LTSni birlamchi qilib belgilang
- hyperledger#3090 zanjirdagi parametrlarni blokcheyndan `sumeragi` ga ko'paytirish

### Tugallangan

- hyperledger#3330 `u128` barglari bilan yorliqsiz enum de-serializatsiyasini tuzating (RC14-ga qaytarilgan)
- hyperledger # 2581 jurnallardagi shovqinni kamaytirdi
- hyperledger # 3360 `tx/s` benchmarkini tuzatish
- hyperledger#3393 `actors` da aloqa o'chirilishini buzish
- hyperledger # 3402 `nightly` tuzilmasi tuzatildi
- hyperledger#3411 Tengdoshlarning bir vaqtda ulanishini to'g'ri boshqaring
- hyperledger#3440. O'tkazish vaqtida aktiv konvertatsiyalarini bekor qilish, aksincha, smart-kontraktlar tomonidan boshqariladi
- hyperledger # 3408: `public_keys_cannot_be_burned_to_nothing` testini tuzating

### Boshqa

- hyperledger#3362 `tokio` aktyorlariga ko'chiring
- hyperledger#3349 `EvaluateOnHost`-ni aqlli shartnomalardan olib tashlang
- hyperledger#1786 Soket manzillari uchun `iroha`-native tiplarini qo'shing
- IVM keshini o'chirib qo'ying
- IVM keshini qayta yoqing
- Ruxsat tekshiruvchisi nomini validatorga o'zgartiring
- hyperledger#3388 `model!` ni modul darajasidagi atribut makrosiga aylantiring
- hyperledger#3370 `hash` ni o'n oltilik qator sifatida seriyalash
- `maximum_transactions_in_block` konfiguratsiyasini `queue` dan `sumeragi` konfiguratsiyasiga o'tkazing
- `AssetDefinitionEntry` turini bekor qiling va olib tashlang
- `configs/client_cli` nomini `configs/client` ga o'zgartiring
- `MAINTAINERS.md` yangilanishi

## [2.0.0-pre-rc.14]

### Qo'shilgan

- hyperledger#3127 ma'lumotlar modeli `structs` sukut bo'yicha shaffof emas
- hyperledger#3122 digest funktsiyasini saqlash uchun `Algorithm` dan foydalaning (hamjamiyat ishtirokchisi)
- hyperledger # 3153 `iroha_client_cli` chiqishi mashinada o'qilishi mumkin
- hyperledger#3105 `AssetDefinition` uchun `Transfer` ni amalga oshirish
- hyperledger#3010 `Transaction` muddati tugashi hodisasi qo'shildi

### Tugallangan

- hyperledger # 3113 beqaror tarmoq testlarini qayta ko'rib chiqish
- hyperledger # 3129 `Parameter` de/seriyalashtirishni tuzatish
- hyperledger#3141 `Hash` uchun `IntoSchema` ni qo'lda amalga oshirish
- hyperledger#3155 Sinovlardagi vahima ilgagini tuzatib, boshi berk ko'chaga tushishning oldini oling
- hyperledger#3166 Ishlamasdan turib o'zgarishlarni ko'rmang, bu esa unumdorlikni oshiradi
- hyperledger#2123 Multihashdan PublicKey de/serializatsiyasiga qaytish
- hyperledger#3132 NewParameter validator qo'shing
- hyperledger#3249 Blok xeshlarini qisman va to'liq versiyalarga bo'lish
- hyperledger#3031 UI/UX etishmayotgan konfiguratsiya parametrlarini tuzating
- hyperledger#3247 `sumeragi` dan nosozlik in'ektsiyasi olib tashlandi.

### Boshqa

- Soxta nosozliklarni tuzatish uchun etishmayotgan `#[cfg(debug_assertions)]` qo'shing
- hyperledger#2133 Oq qog'ozga yaqinroq bo'lish uchun topologiyani qayta yozing
- `iroha_client` `iroha_core` ga qaramlikni olib tashlang
- hyperledger # 2943 `HasOrigin` hosil qiling
- hyperledger#3232 Ish maydoni metama'lumotlarini almashish
- hyperledger # 3254 Refactor `commit_block()` va `replace_top_block()`
- Barqaror standart ajratuvchi ishlov beruvchidan foydalaning
- hyperledger#3183 `docker-compose.yml` fayllar nomini o'zgartiring
- `Multihash` displey formati yaxshilandi
- hyperledger#3268 Global noyob element identifikatorlari
- Yangi PR shabloni

## [2.0.0-pre-rc.13]

### Qo'shilgan- hyperledger#2399 Parametrlarni ISI sifatida sozlash.
- hyperledger#3119 `dropped_messages` ko'rsatkichini qo'shing.
- hyperledger#3094 `n` tengdoshlari bilan tarmoq yarating.
- hyperledger#3082 `Created` hodisasida to'liq ma'lumotlarni taqdim eting.
- hyperledger#3021 Shaffof ko'rsatgich importi.
- hyperledger#2794 FFIda aniq diskriminantlarga ega bo'lgan maydonsiz raqamlarni rad etish.
- hyperledger#2922 `Grant<Role>` standart genezisga qo'shing.
- hyperledger#2922 `NewRole` json deserialization da `inner` maydonini o'tkazib yuboring.
- hyperledger#2922 json deserialization-da `object(_id)`-ni o'tkazib yuboring.
- hyperledger#2922 json deserialization-da `Id`-ni o'tkazib yuboring.
- hyperledger#2922 json deserialization-da `Identifiable`-ni o'tkazib yuboring.
- hyperledger#2963 Ko'rsatkichlarga `queue_size` qo'shing.
- hyperledger # 3027 Kura uchun blokirovka faylini amalga oshirish.
- hyperledger#2813 Kagami standart tengdosh konfiguratsiyasini yaratadi.
- hyperledger # 3019 JSON5-ni qo'llab-quvvatlash.
- hyperledger#2231 FFI o'rash API yaratish.
- hyperledger#2999 Blok imzolarini to'plash.
- hyperledger # 2995 Yumshoq vilkalarni aniqlash.
- hyperledger#2905 `NumericValue`-ni qo'llab-quvvatlash uchun arifmetik operatsiyalarni kengaytiring
- hyperledger#2868 Iroha versiyasini chiqaring va jurnallarda xeshni bajaring.
- hyperledger#2096 Aktivning umumiy miqdori uchun so'rov.
- hyperledger#2899 "client_cli" ga ko'p ko'rsatmalar quyi buyrug'ini qo'shing
- hyperledger#2247 Websocket aloqa shovqinini olib tashlang.
- hyperledger#2889 `iroha_client`-ga blok oqimini qo'shishni qo'shing
- hyperledger#2280 Rol berilgan/bekor qilinganida ruxsat hodisalarini ishlab chiqarish.
- hyperledger#2797 Hodisalarni boyitish.
- hyperledger#2725 `submit_transaction_blocking` ga vaqt tugashini qaytadan kiriting
- hyperledger # 2712 Konfiguratsiya ko'rsatmalari.
- hyperledger # 2491 FFi-da raqamni qo'llab-quvvatlash.
- hyperledger#2775 Sintetik genezda turli kalitlarni yarating.
- hyperledger#2627 Konfiguratsiyani yakunlash, proksi-serverga kirish nuqtasi, kagami docgen.
- hyperledger#2765 `kagami` da sintetik genezis yaratish
- hyperledger#2698 `iroha_client` da noaniq xato xabarini tuzatish
- hyperledger#2689 Ruxsat belgisini aniqlash parametrlarini qo'shing.
- hyperledger#2502 Qurilishning GIT xeshini saqlang.
- hyperledger#2672 `ipv4Addr`, `ipv6Addr` varianti va predikatlarni qo'shing.
- hyperledger # 2626 `Combine` hosilasini amalga oshirish, `config` makrolarini ajratish.
- proksi tuzilmalar uchun hyperledger#2586 `Builder` va `LoadFromEnv`.
- hyperledger#2611 Umumiy noaniq tuzilmalar uchun `TryFromReprC` va `IntoFfi` ni chiqaring.
- hyperledger#2587 `Configurable` ni ikkita xususiyatga ajrating. #2587: `Configurable` ni ikkita xususiyatga ajrating
- hyperledger#2488 `ffi_export` da belgilar belgilarini qo'shish
- hyperledger#2553 Aktiv so'rovlariga tartiblash qo'shing.
- hyperledger#2407 Triggerlarni parametrlash.
- hyperledger#2536 FFI mijozlari uchun `ffi_import` ni joriy qilish.
- hyperledger#2338 `cargo-all-features` asboblarini qo'shing.
- hyperledger#2564 Kagami asboblar algoritmi opsiyalari.
- hyperledger#2490 Mustaqil funksiyalar uchun ffi_export dasturini ishga tushiring.
- hyperledger#1891 Trigger bajarilishini tekshirish.
- hyperledger#1988 Identifikatsion, Eq, Hash, Ord uchun makroslarni oling.
- hyperledger # 2434 FFI bog'lovchi kutubxonasi.
- hyperledger#2073 Blokcheyndagi turlar uchun String o'rniga ConstStringni afzal ko'ring.
- hyperledger#1889 Domenga tegishli triggerlarni qo'shing.
- hyperledger#2098 Sarlavha so'rovlarini bloklash. # 2098: blok sarlavhasi so'rovlarini qo'shing
- hyperledger#2467 iroha_client_cli-ga hisob qaydnomasini berish kichik buyrug'ini qo'shing.
- hyperledger#2301 So'rov paytida tranzaksiya blok xeshini qo'shing.
 - hyperledger#2454 Norito dekoder vositasiga qurish skriptini qo'shing.
- hyperledger#2061 Filtrlar uchun makros hosil qiling.
- hyperledger#2228 Smartcontracts so'rovi xatosiga Ruxsatsiz variantni qo'shing.
- hyperledger#2395 Agar genezisni qo'llash mumkin bo'lmasa, vahima qo'shing.
- hyperledger#2000 Bo'sh nomlarga ruxsat bermaslik. # 2000: bo'sh nomlarga ruxsat bermang
 - hyperledger#2127 Norito kodek tomonidan dekodlangan barcha ma'lumotlar iste'mol qilinishiga ishonch hosil qilish uchun aql-idrok tekshiruvini qo'shing.
- hyperledger#2360 `genesis.json`ni yana ixtiyoriy qiling.
- hyperledger#2053 Shaxsiy blokcheyndagi qolgan barcha so'rovlarga test qo'shing.
- hyperledger # 2381 `Role` ro'yxatini unify.
- hyperledger # 2053 Shaxsiy blokcheyndagi aktivlar bilan bog'liq so'rovlarga testlar qo'shing.
- hyperledger#2053 "private_blockchain" ga test qo'shing
- hyperledger # 2302 "FindTriggersByDomainId" stub-so'rovini qo'shing.
- hyperledger#1998 So'rovlarga filtr qo'shing.
- hyperledger#2276 Joriy Blok xeshini BlockHeaderValue-ga qo'shing.
- hyperledger # 2161 identifikatorni boshqarish va umumiy FFI fns.
- tutqich identifikatorini qo'shing va umumiy xususiyatlarning FFI ekvivalentlarini amalga oshiring (Clone, Eq, Ord)
- hyperledger # 1638 `configuration` doc pastki daraxtini qaytaradi.
- hyperledger # 2132 `endpointN` proc makrosini qo'shing.
- hyperledger#2257 Revoke<Role> RoleRevoked hodisasini chiqaradi.
- hyperledger#2125 FindAssetDefinitionById so'rovini qo'shing.
- hyperledger # 1926 Signalni boshqarish va oqlangan o'chirishni qo'shing.
- hyperledger#2161 `data_model` uchun FFI funksiyalarini yaratadi
- hyperledger#1149 Bloklangan fayllar soni har bir katalog uchun 1000000 dan oshmaydi.
- hyperledger#1413 API versiyasining so'nggi nuqtasini qo'shing.
- hyperledger # 2103 bloklar va tranzaktsiyalar uchun so'rovlarni qo'llab-quvvatlaydi. `FindAllTransactions` so'rovini qo'shing
- hyperledger # 2186 `BigQuantity` va `Fixed` uchun transfer ISI qo'shing.
- hyperledger#2056 `AssetValueType` `enum` uchun derive proc makro qutisini qo'shing.
- hyperledger#2100 Aktiv bilan barcha hisoblarni topish uchun so'rov qo'shing.
- hyperledger#2179 Trigger bajarilishini optimallashtirish.
- hyperledger # 1883 O'rnatilgan konfiguratsiya fayllarini olib tashlang.
- hyperledger # 2105 mijozdagi so'rovlar xatolarini qayta ishlaydi.
- hyperledger#2050 Rol bilan bog'liq so'rovlarni qo'shing.
- hyperledger#1572 Ixtisoslashtirilgan ruxsat tokenlari.
- hyperledger#2121 Klaviaturalar juftligi tuzilganda yaroqliligini tekshiring.
 - hyperledger#2003 Norito Dekoder vositasini joriy qiling.
- hyperledger#1952 Optimallashtirish uchun standart sifatida TPS benchmarkini qo'shing.
- hyperledger#2040 Tranzaksiyani bajarish chegarasi bilan integratsiya testini qo'shing.
- hyperledger#1890 Orillion foydalanish holatlariga asoslangan integratsiya testlarini joriy qilish.
- hyperledger#2048 Asboblar zanjiri faylini qo'shing.
- hyperledger#2100 Aktiv bilan barcha hisoblarni topish uchun so'rov qo'shing.
- hyperledger#2179 Trigger bajarilishini optimallashtirish.
- hyperledger # 1883 O'rnatilgan konfiguratsiya fayllarini olib tashlang.
- hyperledger#2004 `isize` va `usize` `IntoSchema` bo'lishini taqiqlang.
- hyperledger # 2105 mijozdagi so'rovlar xatolarini qayta ishlaydi.
- hyperledger#2050 Rol bilan bog'liq so'rovlarni qo'shing.
- hyperledger#1572 Ixtisoslashtirilgan ruxsat tokenlari.
- hyperledger#2121 Klaviaturalar juftligi tuzilganda yaroqliligini tekshiring.
 - hyperledger#2003 Norito Dekoder vositasini joriy qilish.
- hyperledger#1952 Optimallashtirish uchun standart sifatida TPS benchmarkini qo'shing.
- hyperledger#2040 Tranzaksiyani bajarish chegarasi bilan integratsiya testini qo'shing.
- hyperledger#1890 Orillion foydalanish holatlariga asoslangan integratsiya testlarini joriy qilish.
- hyperledger#2048 Asboblar zanjiri faylini qo'shing.
- hyperledger#2037 Pre-commit Triggerlarni joriy qilish.
- hyperledger # 1621 Qo'ng'iroq triggerlari bilan tanishtiring.
- hyperledger#1970 ixtiyoriy sxema so'nggi nuqtasini qo'shing.
- hyperledger#1620 Vaqtga asoslangan triggerlarni joriy qilish.
- hyperledger#1918 `client` uchun asosiy autentifikatsiyani amalga oshirish
- hyperledger # 1726 Reliz PR ish jarayonini amalga oshirish.
- hyperledger # 1815 So'rov javoblarini ko'proq turdagi tizimli qiling.
- hyperledger # 1928 `gitchangelog` yordamida o'zgarishlar jurnalini yaratishni amalga oshiradi
- hyperledger # 1902 Yalang'och metall 4-peer o'rnatish skript.

  Docker-compose talab qilmaydigan va Iroha disk raskadrovka tuzilmasidan foydalanadigan setup_test_env.sh versiyasi qo‘shildi.
- hyperledger#1619 Hodisaga asoslangan triggerlarni joriy qilish.
- hyperledger#1195 Websocket ulanishini toza yoping.
- hyperledger#1606 Domen tuzilishidagi domen logotipiga ipfs havolasini qo'shing.
- hyperledger # 1754 Kura inspektor CLI qo'shing.
- hyperledger#1790 stekga asoslangan vektorlar yordamida unumdorlikni oshiring.
- hyperledger#1805 Vahima xatolari uchun ixtiyoriy terminal ranglari.
- hyperledger#1749 `no_std` `data_model` da
- hyperledger # 1179 Revoke-ruxsat yoki rol ko'rsatmasini qo'shing.
- hyperledger # 1782 iroha_crypto no_std bilan mos keladi.
- hyperledger#1172 Ko'rsatma hodisalarini amalga oshirish.
- hyperledger # 1734 Bo'shliqlarni istisno qilish uchun `Name` ni tasdiqlang.
- hyperledger#1144 Metadata joylashtirishni qo'shing.
- #1210 Oqimni bloklash (server tomoni).
- hyperledger#1331 Ko'proq `Prometheus` ko'rsatkichlarini amalga oshirish.
- hyperledger # 1689 Xususiyatlar bog'liqligini tuzatish. # 1261: Yuk ko'tarilishini qo'shing.
- versiyali elementlar uchun o'rash strukturasi o'rniga hyperledger#1675 turidan foydalaning.
- hyperledger#1643 Sinovlarda tengdoshlarning genezisni amalga oshirishini kuting.
- giperledger # 1678 `try_allocate`
- hyperledger # 1216 Prometheus oxirgi nuqtasini qo'shing. # 1216: ko'rsatkichlarning so'nggi nuqtasini dastlabki amalga oshirish.
- hyperledger # 1238 Ish vaqti jurnali darajasidagi yangilanishlar. Asosiy `connection` kirish nuqtasiga asoslangan qayta yuklash yaratildi.
- hyperledger # 1652 PR sarlavhasini formatlash.
- `Status` ga ulangan tengdoshlar sonini qo'shing

  - "Ulangan tengdoshlar soni bilan bog'liq narsalarni o'chirish" ni qaytarish

  Bu qaytarishlar b228b41dab3c035ce9973b6aa3b35d443c082544.
  - Clarify `Peer` haqiqiy ochiq kalitga faqat qoʻl siqishdan keyin ega boʻladi
  - `DisconnectPeer` sinovlarsiz
  - Ro'yxatdan o'tish peer ijrosini amalga oshirish
  - `client_cli` ga tengdosh kichik buyrug'ini qo'shing (ro'yxatdan o'chirish).
  - Ro'yxatdan o'tmagan tengdoshning manzili bo'yicha qayta ulanishni rad etingSizning tengdoshingiz boshqa tengdoshni ro'yxatdan o'tkazib, aloqani uzgandan so'ng,
  sizning tarmog'ingiz tengdoshingizdan qayta ulanish so'rovlarini eshitadi.
  Avval bilishingiz mumkin bo'lgan yagona narsa port raqami ixtiyoriy bo'lgan manzildir.
  Shunday qilib, ro'yxatdan o'tmagan tengdoshni port raqamidan boshqa qism bilan eslang
  va u yerdan qayta ulanishni rad eting
- Muayyan portga `/status` so'nggi nuqtasini qo'shing.

### Tuzatishlar- hyperledger # 3129 `Parameter` de/serializatsiyani tuzatish.
- hyperledger#3109 Rol agnostik xabaridan keyin `sumeragi` uyquni oldini olish.
- hyperledger#3046 Iroha bo'sh joydan yaxshi ishga tushishiga ishonch hosil qiling
  `./storage`
- hyperledger#2599 Bolalar bog'chasi zig'irchalarini olib tashlang.
- hyperledger#3087 Ko'rinishni o'zgartirgandan so'ng B to'plami tekshiruvchilaridan ovozlarni to'plang.
- hyperledger#3056 `tps-dev` benchmark osilganligini tuzatish.
- hyperledger#1170 Klonlash-wsv uslubidagi yumshoq vilkalar bilan ishlashni amalga oshirish.
- hyperledger#2456 Genesis blokini cheksiz qiling.
- hyperledger#3038 Multisiglarni qayta yoqing.
- hyperledger # 2894 `LOG_FILE_PATH` env o'zgaruvchisini seriyadan chiqarishni tuzatish.
- hyperledger#2803 Imzo xatolari uchun to'g'ri holat kodini qaytaring.
- hyperledger # 2963 `Queue` tranzaktsiyalarni to'g'ri olib tashlash.
- hyperledger#0000 Vergen breaking CI.
- hyperledger#2165 Asboblar zanjiri fidgetini olib tashlang.
- hyperledger#2506 Blok tekshiruvini tuzatish.
- hyperledger#3013 To'g'ri zanjir yoqish validatorlari.
- hyperledger#2998 Ishlatilmagan zanjir kodini o'chirish.
- hyperledger#2816 Bloklarga kirish mas'uliyatini kuraga o'tkazing.
- hyperledger#2384 dekodlashni decode_all bilan almashtiring.
- hyperledger # 1967 ValueNameni Ism bilan almashtiring.
- hyperledger#2980 blok qiymatini ffi turini tuzatish.
- hyperledger#2858 std o'rniga parking_lot::Mutex ni kiriting.
- hyperledger # 2850 `Fixed` seriyali dekodlanishini tuzatdi
- hyperledger#2923 `AssetDefinition` bajarilmasa, `FindError`ni qaytaring
  mavjud.
- hyperledger # 0000 tuzatish `panic_on_invalid_genesis.sh`
- hyperledger#2880 Websocket ulanishini to'g'ri yoping.
- hyperledger#2880 Blok oqimini tuzatish.
- hyperledger#2804 `iroha_client_cli` tranzaksiya blokirovkasini yuborish.
- hyperledger#2819 Muhim bo'lmagan a'zolarni WSV dan tashqariga ko'chiring.
- Ifodani ketma-ketlashtirish rekursiyasi xatosini tuzatdi.
- hyperledger#2834 Qisqartirilgan sintaksisni takomillashtirish.
- hyperledger#2379 Blocks.txt-ga yangi Kura bloklarini tashlash qobiliyatini qo'shing.
- hyperledger#2758 Sxemaga Saralash strukturasini qo'shing.
- CI.
- hyperledger#2548 Katta genezis faylida ogohlantiring.
- hyperledger#2638 `whitepaper` yangilang va o'zgarishlarni targ'ib qiling.
- hyperledger # 2678 Sting bo'limidagi testlarni tuzatish.
- hyperledger # 2678 Kura kuchini o'chirishda testlarni bekor qilishni tuzatish.
- hyperledger#2607 Ko'proq soddalik uchun sumeragi kodining refaktori va
  mustahkamlik tuzatishlar.
- hyperledger#2561 Ko'rinishdagi o'zgarishlarni konsensusga qayta kiriting.
- hyperledger#2560 Blok_sinxronlash va tengdoshlarni ajratishda qayta qo'shing.
- hyperledger # 2559 Sumeragi mavzuni o'chirishni qo'shing.
- hyperledger#2558 Kuradan wsvni yangilashdan oldin genezisni tasdiqlang.
- hyperledger#2465 Sumeragi tugunini bir tarmoqli holat sifatida qayta tiklash
  mashina.
- hyperledger # 2449 Sumeragi qayta tuzilmasining dastlabki amalga oshirilishi.
- hyperledger#2802 Konfiguratsiya uchun env yuklanishini tuzatish.
- hyperledger#2787 Har bir tinglovchini vahima paytida o'chirish haqida xabar bering.
- hyperledger # 2764 Maksimal xabar hajmi bo'yicha cheklovni olib tashlang.
- # 2571: Yaxshiroq Kura inspektori UX.
- hyperledger#2703 Orillion dev env xatolarini tuzatish.
- schema/src-dagi hujjat sharhidagi xatoni tuzating.
- hyperledger#2716 Uptime-da vaqtni hammaga ochiq qilish.
- hyperledger#2700 Docker tasvirlarida `KURA_BLOCK_STORE_PATH` eksport qiling.
- hyperledger#0 `/iroha/rust-toolchain.toml` ni quruvchidan olib tashlang
  tasvir.
- hyperledger # 0 tuzatish `docker-compose-single.yml`
- hyperledger#2554 `secp256k1` 32 dan qisqa bo'lsa xatoni ko'taring
  bayt.
- hyperledger#0 Har bir tengdosh uchun saqlashni ajratish uchun `test_env.sh` ni o'zgartiring.
- hyperledger#2457 Testlarda kurani majburan o'chirish.
- hyperledger#2623 VariantCount uchun doctestni tuzatdi.
- ui_fail testlarida kutilgan xatoni yangilang.
- Ruxsat tekshiruvchilarida noto'g'ri hujjat sharhini tuzating.
- hyperledger#2422 konfiguratsiya so'nggi nuqta javobida shaxsiy kalitlarni yashirish.
- hyperledger#2492: Hodisaga mos keluvchi bajarilmayotgan barcha triggerlarni tuzating.
- hyperledger#2504 Ishlamay qolgan tps benchmarkini tuzatish.
- hyperledger#2477 Rollardan ruxsatlar hisobga olinmagan xatolikni tuzatdi.
- hyperledger # 2416 MacOS qo'lidagi zig'irchalarni tuzatish.
- hyperledger # 2457 Vahima paytida o'chirish bilan bog'liq bo'lgan testlarni tuzatish.
  #2457: Vahima konfiguratsiyasida o'chirishni qo'shing
- hyperledger#2473 parse rustc --version o'rniga RUSTUP_TOOLCHAIN.
- hyperledger # 1480 Vahima paytida o'chiring. #1480: Vahima paytida dasturdan chiqish uchun vahima ilgagini qo'shing
- hyperledger#2376 Soddalashtirilgan Kura, asinxron emas, ikkita fayl.
- hyperledger#0000 Docker tuzilma xatosi.
- hyperledger # 1649 `spawn` ni `do_send` dan olib tashlang
- hyperledger # 2128 `MerkleTree` qurilishi va iteratsiyasini tuzatish.
- hyperledger#2137 Ko'p jarayonli kontekst uchun testlarni tayyorlang.
- hyperledger # 2227 Ro'yxatdan o'tish va aktiv uchun ro'yxatdan o'chirish.
- hyperledger#2081 Rol berish xatosini tuzatdi.
- hyperledger#2358 disk raskadrovka profili bilan nashrni qo'shing.
- hyperledger#2294 oneshot.rs-ga flamegraf avlodini qo'shing.
- hyperledger#2202 So'rov javobidagi jami maydonni tuzatish.
- hyperledger#2081 Rol berish uchun test ishini tuzating.
- hyperledger#2017 Rollarni ro'yxatdan o'chirishni tuzatish.
- hyperledger#2303 Fix docker-compose'ning tengdoshlari yaxshi o'chirilmaydi.
- hyperledger#2295 Ro'yxatdan o'tish trigger xatosini tuzatdi.
- hyperledger # 2282 FFI yaxshilash getset dasturidan kelib chiqadi.
- hyperledger # 1149 nocheckin kodini olib tashlang.
- hyperledger#2232 Iroha genezisda juda ko'p isi bo'lsa, mazmunli xabarni chop eting.
- hyperledger#2170 M1 mashinalaridagi docker konteynerida tuzatilgan tuzatma.
- hyperledger#2215 `cargo build` uchun 2022-04-20 tungi ixtiyoriy qilish
- hyperledger#1990 config.json mavjud bo'lmaganda env vars orqali tengdosh ishga tushirishni yoqing.
- hyperledger # 2081 Rollarni ro'yxatga olishni tuzatish.
- hyperledger#1640 config.json va genesis.json ni yarating.
- hyperledger#1716 konsensus xatosini f=0 holatlar bilan tuzating.
- hyperledger#1845 Zarb qilinmaydigan aktivlarni faqat bir marta zarb qilish mumkin.
- hyperledger#2005 `Client::listen_for_events()` WebSocket oqimini yopmasligini tuzatish.
- hyperledger # 1623 RawGenesisBlockBuilder yarating.
- hyperledger#1917 easy_from_str_impl makrosini qo'shing.
- hyperledger#1990 config.json mavjud bo'lmaganda env vars orqali tengdosh ishga tushirishni yoqing.
- hyperledger # 2081 Rollarni ro'yxatga olishni tuzatish.
- hyperledger#1640 config.json va genesis.json ni yarating.
- hyperledger#1716 konsensus xatosini f=0 holatlar bilan tuzating.
- hyperledger#1845 Zarb qilinmaydigan aktivlarni faqat bir marta zarb qilish mumkin.
- hyperledger#2005 `Client::listen_for_events()` WebSocket oqimini yopmasligini tuzatish.
- hyperledger # 1623 RawGenesisBlockBuilder yarating.
- hyperledger#1917 easy_from_str_impl makrosini qo'shing.
- hyperledger#1922 crypto_cli-ni asboblarga o'tkazing.
- hyperledger#1969 `roles` funksiyasini standart funksiyalar majmuasining bir qismiga aylantiring.
- hyperledger # 2013 tuzatish CLI args.
- hyperledger # 1897 serializatsiyadan foydalanish/sizeni olib tashlang.
- hyperledger # 1955 `:` ni `web_login` ichida o'tkazish imkoniyatini tuzatish
- hyperledger#1943 Sxemaga so'rov xatolarini qo'shing.
- hyperledger # 1939 `iroha_config_derive` uchun mos xususiyatlar.
- hyperledger # 1908 telemetriya tahlili skripti uchun nol qiymat bilan ishlashni tuzatdi.
- hyperledger#0000 Bilvosita e'tibor berilmagan doc-testni aniq e'tibordan chetda qoldirish.
- hyperledger # 1848 Ochiq kalitlarni hech narsaga yoqishning oldini olish.
- hyperledger # 1811 ishonchli tengdosh kalitlarini aniqlashtirish uchun testlar va tekshiruvlarni qo'shdi.
- hyperledger # 1821 MerkleTree va VersionedValidBlock uchun IntoSchema qo'shing, HashOf va SignatureOf sxemalarini tuzating.
- hyperledger#1819 Tekshiruvdagi xato hisobotidan kuzatuvni olib tashlang.
- hyperledger # 1774 tekshirish xatolarining aniq sababini qayd qiladi.
- hyperledger#1714 PeerId ni faqat kalit bilan solishtiring.
- hyperledger#1788 `Value` xotira maydonini qisqartirish.
- hyperledger # 1804 HashOf, SignatureOf uchun sxema yaratishni tuzating, hech qanday sxemalar etishmayotganligiga ishonch hosil qilish uchun test qo'shing.
- hyperledger # 1802 Jurnalni o'qish qobiliyatini yaxshilash.
  - hodisalar jurnali kuzatuv darajasiga ko'chirildi
  - ctx jurnalni yozib olishdan olib tashlandi
  - terminal ranglari ixtiyoriy qilingan (fayllarga jurnalni yaxshiroq chiqarish uchun)
- hyperledger # 1783 Ruxsat etilgan torii benchmark.
- hyperledger # 1772 # 1764 dan keyin tuzatish.
- hyperledger#1755 #1743, #1725 uchun kichik tuzatishlar.
  - JSON-larni №1743 `Domain` tuzilmasi o'zgarishiga muvofiq tuzating
- hyperledger # 1751 konsensus tuzatishlar. # 1715: Yuqori yuklarni bartaraf etish uchun konsensus tuzatildi (# 1746)
  - O'zgarishlarni qayta ishlash bo'yicha tuzatishlarni ko'ring
  - Muayyan tranzaksiya xeshlaridan qat'iy nazar o'zgartirish dalillarini ko'ring
  - Xabar uzatishning qisqarishi
  - Darhol xabar yuborish o'rniga ko'rishni o'zgartirish ovozlarini to'plang (tarmoqning chidamliligini yaxshilaydi)
  - Sumeragi da Actor ramkasidan to'liq foydalaning (topshiriqlar o'rniga o'zingizga xabarlarni rejalashtirish)
  - Sumeragi bilan sinovlar uchun nosozliklarni in'ektsiya qilishni yaxshilaydi
  - Sinov kodini ishlab chiqarish kodiga yaqinlashtiradi
  - Haddan tashqari murakkab o'ramlarni olib tashlaydi
  - Sumeragi test kodida aktyor kontekstidan foydalanishga ruxsat beradi
- hyperledger#1734 Yangi Domen tekshiruviga mos kelish uchun genezisni yangilang.
- hyperledger # 1742 `core` ko'rsatmalarida qaytarilgan aniq xatolar.
- hyperledger # 1404 Ruxsat etilganligini tekshiring.
- hyperledger#1636 `trusted_peers.json` va `structopt`ni olib tashlang
  # 1636: `trusted_peers.json`ni olib tashlang.
- hyperledger # 1706 Topologiya yangilanishi bilan `max_faults` yangilanishi.
- hyperledger#1698 Ruxsat etilgan ochiq kalitlar, hujjatlar va xato xabarlari.
- zarb masalalari (1593 va 1405) 1405-son

### Refaktor- Sumeragi asosiy tsiklidan funksiyalarni chiqarib oling.
- Refactor `ProofChain` yangi turga.
- `Mutex` ni `Metrics` dan olib tashlang
- Adt_const_generics tungi funksiyasini olib tashlang.
- hyperledger#3039 Multisigs uchun kutish buferini kiriting.
- Sumeragini soddalashtiring.
- hyperledger#3053 Kesilgan lintlarni tuzatish.
- hyperledger#2506 Blokni tekshirish bo'yicha qo'shimcha testlarni qo'shing.
- Kurada `BlockStoreTrait`ni olib tashlang.
- `nightly-2022-12-22` uchun lintlarni yangilang
- hyperledger#3022 `transaction_cache` da `Option`ni olib tashlang
- hyperledger#3008 `Hash` ga joy qiymatini qo'shing
- Lintsni 1.65 ga yangilang.
- Qoplashni kuchaytirish uchun kichik testlarni qo'shing.
- `FaultInjection` dan o'lik kodni olib tashlang
- Sumeragidan p2p ga kamroq qo'ng'iroq qiling.
- hyperledger#2675 Vec-ni ajratmasdan element nomlarini/identifikatorlarini tasdiqlang.
- hyperledger#2974 To'liq revalidatsiyasiz blokni buzishning oldini oling.
- kombinatorlarda samaraliroq `NonEmpty`.
- hyperledger#2955 BlockSigned xabaridan blokni olib tashlang.
- hyperledger # 1868 Tasdiqlangan tranzaksiyalarning yuborilishini oldini olish
  tengdoshlar o'rtasida.
- hyperledger#2458 Umumiy kombinator API-ni joriy qilish.
- Gitignore-ga saqlash papkasini qo'shing.
- hyperledger#2909 Keyingisi uchun qattiq kod portlari.
- hyperledger # 2747 O'zgartirish `LoadFromEnv` API.
- Konfiguratsiya muvaffaqiyatsizligi haqida xato xabarlarini yaxshilash.
- `genesis.json` ga qo'shimcha misollar qo'shing
- `rc9` chiqarilishidan oldin foydalanilmagan bog'liqliklarni olib tashlang.
- Yangi Sumeragi da lintingni yakunlang.
- Asosiy tsikldagi subprotseduralarni ajratib oling.
- hyperledger#2774 `kagami` genezis yaratish rejimini bayroqchaga o'zgartiring
  pastki buyruq.
- hyperledger # 2478 `SignedTransaction` qo'shing
- hyperledger#2649 `byteorder` kassasini `Kura` dan olib tashlang
- `DEFAULT_BLOCK_STORE_PATH` nomini `./blocks` dan `./storage` ga o'zgartiring
- hyperledger#2650 Iroha submodullarini o'chirish uchun `ThreadHandler` ni qo'shing.
- hyperledger#2482 `Account` ruxsat tokenlarini `Wsv` da saqlang
- 1.62 ga yangi lintlar qo'shing.
- `p2p` xato xabarlarini yaxshilash.
- hyperledger # 2001 `EvaluatesTo` statik turini tekshirish.
- hyperledger#2052 Ruxsat tokenlarini ta'rifi bilan ro'yxatdan o'tish mumkin qiling.
  № 2052: PermissionTokenDefinitionni amalga oshirish
- Barcha xususiyatlar kombinatsiyasi ishlashini ta'minlang.
- hyperledger#2468 Ruxsat tekshiruvchilaridan disk raskadrovka supertraitini olib tashlang.
- hyperledger#2419 Aniq `drop`larni olib tashlang.
- hyperledger#2253 `Registrable` xususiyatini `data_model` ga qo'shing
- Ma'lumotlar hodisalari uchun `Identifiable` o'rniga `Origin` ni qo'llang.
- hyperledger # 2369 Refaktor ruxsati tekshirgichlari.
- hyperledger#2307 `events_sender`-ni `WorldStateView`-da ixtiyoriy emas.
- hyperledger#1985 `Name` strukturasi hajmini kamaytirish.
- Yana `const fn` qo'shing.
- `default_permissions()` dan integratsiya testlarini o'tkazing
- private_blockchain-ga ruxsat tokenlarini qo'shing.
- hyperledger#2292 `WorldTrait` ni olib tashlang, `IsAllowedBoxed` dan generiklarni olib tashlang
- hyperledger#2204 Asset bilan bog'liq operatsiyalarni umumiy qilish.
- hyperledger#2233 `impl`ni `Display` va `Debug` uchun `derive` bilan almashtiring.
- Aniqlangan tuzilma yaxshilanishlari.
- hyperledger#2323 Kura init xato xabarini yaxshilash.
- hyperledger#2238 Sinovlar uchun peer builder qo'shing.
- hyperledger#2011 Batafsil tavsiflovchi konfiguratsiya parametrlari.
- hyperledger # 1896 `produce_event` dasturini soddalashtiring.
- `QueryError` atrofidagi refaktor.
- `TriggerSet` ni `data_model` ga o'tkazing.
- hyperledger # 2145 refactor mijozining `WebSocket` tomoni, sof ma'lumotlar mantiqini chiqarib oling.
- `ValueMarker` xususiyatini olib tashlang.
- hyperledger#2149 `prelude` da `Mintable` va `MintabilityError`ni ochish
- hyperledger#2144 mijozning http ish jarayonini qayta loyihalash, ichki api-ni ochish.
- `clap` ga o'ting.
- `iroha_gen` ikkilik, birlashtiruvchi hujjatlar, schema_bin yarating.
- hyperledger#2109 `integration::events::pipeline` testini barqaror qiling.
- hyperledger # 1982 `iroha_crypto` tuzilmalariga kirishni qamrab oladi.
- `AssetDefinition` quruvchisini qo'shing.
- APIdan keraksiz `&mut` ni olib tashlang.
- ma'lumotlar modeli tuzilmalariga kirishni inkapsulyatsiya qilish.
- hyperledger#2144 mijozning http ish jarayonini qayta loyihalash, ichki api-ni ochish.
- `clap` ga o'ting.
- `iroha_gen` ikkilik, birlashtiruvchi hujjatlar, schema_bin yarating.
- hyperledger#2109 `integration::events::pipeline` testini barqaror qiling.
- hyperledger # 1982 `iroha_crypto` tuzilmalariga kirishni qamrab oladi.
- `AssetDefinition` quruvchisini qo'shing.
- APIdan keraksiz `&mut` ni olib tashlang.
- ma'lumotlar modeli tuzilmalariga kirishni inkapsulyatsiya qilish.
- Yadro, `sumeragi`, misol funksiyalari, `torii`
- hyperledger # 1903 hodisa emissiyasini `modify_*` usullariga o'tkazing.
- `data_model` lib.rs faylini ajrating.
- Navbatga ws havolasini qo'shing.
- hyperledger#1210 Voqealar oqimini ajratish.
  - Tranzaksiya bilan bog'liq funksiyalarni data_model/transaction moduliga o'tkazing
- hyperledger#1725 Torii da global holatni olib tashlang.
  - `add_state macro_rules` ni amalga oshiring va `ToriiState` ni olib tashlang
- Linter xatosini tuzatish.
- hyperledger # 1661 `Cargo.toml` tozalash.
  - Yuklarga bog'liqliklarni saralash
- hyperledger # 1650 `data_model`ni tartibga soladi
  - Dunyoni wsv-ga o'tkazing, rollar xususiyatini tuzating, CommittedBlock uchun IntoSchema-ni oling
- `json` fayllari va readmeni tashkil qilish. Shablonga mos kelish uchun Readme-ni yangilang.
- 1529: tuzilgan logging.
  - Refactor jurnali xabarlari
- `iroha_p2p`
  - p2p xususiylashtirishni qo'shing.

### Hujjatlar

- Iroha Client CLI readme-ni yangilang.
- Darslik parchalarini yangilang.
- API spetsifikatsiyasiga "sort_by_metadata_key" ni qo'shing.
- Hujjatlarga havolalarni yangilang.
- O'quv qo'llanmasini aktivlar bilan bog'liq hujjatlar bilan kengaytiring.
- Eskirgan doc fayllarni olib tashlang.
- Tinish belgilarini ko'rib chiqing.
- Ba'zi hujjatlarni o'quv qo'llanmasiga ko'chiring.
- Sahnalashtirish shoxobchasi uchun xiralik hisoboti.
- Pre-rc.7 uchun o'zgarishlar jurnalini yarating.
- 30-iyul uchun mo'rtlik hisoboti.
- Bump versiyalari.
- Sinovning silliqligini yangilang.
- hyperledger#2499 client_cli xato xabarlarini tuzatish.
- hyperledger # 2344 2.0.0-pre-rc.5-lts uchun CHANGELOG yarating.
- O'quv qo'llanmasiga havolalar qo'shing.
- Git hooks haqidagi ma'lumotlarni yangilang.
- yorilish testini yozish.
- hyperledger#2193 Iroha mijoz hujjatlarini yangilash.
- hyperledger#2193 Iroha CLI hujjatlarini yangilash.
- hyperledger#2193 Ibratli quti uchun README-ni yangilang.
 - hyperledger # 2193 Norito Dekoder vositasi hujjatlarini yangilash.
- hyperledger#2193 Kagami hujjatlarini yangilash.
- hyperledger#2193 Benchmark hujjatlarini yangilash.
- hyperledger#2192 hissa qo'shish bo'yicha ko'rsatmalarni ko'rib chiqing.
- Kod ichidagi buzilgan havolalarni tuzatish.
- hyperledger # 1280 Iroha hujjati ko'rsatkichlari.
- hyperledger#2119 Iroha ni Docker konteynerida qayta yuklash bo'yicha ko'rsatmalar qo'shing.
- hyperledger#2181 README-ni ko'rib chiqing.
- hyperledger#2113 Cargo.toml fayllaridagi hujjat xususiyatlari.
- hyperledger#2177 gitchangelog chiqishini tozalang.
- hyperledger # 1991 Readmeni Kura inspektoriga qo'shing.
- hyperledger#2119 Iroha ni Docker konteynerida qayta yuklash bo'yicha ko'rsatmalar qo'shing.
- hyperledger#2181 README-ni ko'rib chiqing.
- hyperledger#2113 Cargo.toml fayllaridagi hujjat xususiyatlari.
- hyperledger#2177 gitchangelog chiqishini tozalang.
- hyperledger # 1991 Readmeni Kura inspektoriga qo'shing.
- so'nggi o'zgarishlar jurnalini yaratish.
- O'zgarishlar jurnalini yarating.
- Eskirgan README fayllarini yangilang.
- `api_spec.md` ga etishmayotgan hujjatlar qo'shildi.

### CI/CD o'zgarishlari- Yana beshta o'z-o'zidan yuguruvchi qo'shing.
- Soramitsu registriga oddiy rasm tegini qo'shing.
- libgit2-sys 0.5.0 uchun vaqtinchalik yechim. 0.4.4 ga qayting.
- Arkga asoslangan tasvirdan foydalanishga harakat qiling.
- Yangi tungi konteynerda ishlash uchun ish oqimlarini yangilang.
- Ikkilik kirish nuqtalarini qamrovdan olib tashlang.
- Dev testlarini Equinix o'z-o'zidan ishlaydigan yuguruvchilarga o'tkazing.
- hyperledger#2865 `scripts/check.sh` dan tmp faylidan foydalanishni olib tashlang
- hyperledger#2781 Qoplama ofsetlarini qo'shing.
- Sekin integratsiya testlarini o'chirib qo'ying.
- Asosiy tasvirni docker keshi bilan almashtiring.
- hyperledger#2781 Codecov commit ota-ona xususiyatini qo'shing.
- Ishlarni github runnerlariga o'tkazing.
- hyperledger # 2778 Mijoz konfiguratsiyasini tekshirish.
- hyperledger#2732 Iroha2-asos tasvirlarini yangilash va qo'shish uchun shartlar qo'shing
  PR belgilari.
- Kecha tasvirini tuzating.
- `docker/build-push-action` bilan `buildx` xatosini tuzating
- Ishlamaydigan birinchi yordam `tj-actions/changed-files`
- #2662 dan keyin rasmlarni ketma-ket nashr qilishni yoqing.
- Port registrini qo'shing.
- `api-changes` va `config-changes` avtomatik yorlig'i
- Tasvirda xeshni amalga oshiring, asboblar zanjiri fayli, UI izolyatsiyasi,
  sxemani kuzatish.
- Nashr qilish ish oqimlarini ketma-ket qiling va №2427 ni to'ldiring.
- hyperledger # 2309: CI da hujjat testlarini qayta yoqing.
- hyperledger # 2165 Codecov o'rnatilishini olib tashlang.
- Joriy foydalanuvchilar bilan ziddiyatlarni oldini olish uchun yangi konteynerga o'ting.
 - hyperledger # 2158 `parity_scale_codec` va boshqa bog'liqliklarni yangilang. (Norito kodek)
- Qurilishni tuzatish.
- hyperledger # 2461 Iroha2 CI ni yaxshilash.
- `syn` yangilanishi.
- qamrovni yangi ish oqimiga o'tkazish.
- teskari docker login ver.
- `archlinux:base-devel` versiyasi spetsifikatsiyasini olib tashlang
- Dockerfiles va Codecov hisobotlarini qayta ishlatish va bir vaqtda yangilang.
- O'zgarishlar jurnalini yarating.
- `cargo deny` faylini qo'shing.
- `iroha2` dan ko'chirilgan ish oqimi bilan `iroha2-lts` filialini qo'shing
- hyperledger#2393 Docker asosiy tasvirining versiyasini o'zgartiring.
- hyperledger # 1658 Hujjatlarni tekshirishni qo'shing.
- Sandiqlarning versiyalari va foydalanilmagan bog'liqliklarni olib tashlang.
- Keraksiz qamrov hisobotini olib tashlang.
- hyperledger#2222 Sinovlarni qamrovni o'z ichiga oladimi yoki yo'qmi bo'yicha ajrating.
- hyperledger # 2153 tuzatish # 2154.
- Versiya barcha qutilarni uradi.
- Quvurni joylashtirishni tuzatish.
- hyperledger # 2153 Qoplashni tuzatish.
- Genesis tekshiruvini qo'shing va hujjatlarni yangilang.
- Bump zang, mog'or va tungi mos ravishda 1,60, 1,2,0 va 1,62.
- load-rs triggerlari.
- hyperledger # 2153 tuzatish # 2154.
- Versiya barcha qutilarni uradi.
- Quvurni joylashtirishni tuzatish.
- hyperledger # 2153 Qoplashni tuzatish.
- Genesis tekshiruvini qo'shing va hujjatlarni yangilang.
- Bump zang, mog'or va tungi mos ravishda 1,60, 1,2,0 va 1,62.
- load-rs triggerlari.
- load-rs: ish oqimining triggerlarini chiqarish.
- Push ish jarayonini tuzatish.
- Standart xususiyatlarga telemetriya qo'shing.
- asosiy ish oqimini surish uchun tegishli teg qo'shing.
- muvaffaqiyatsiz testlarni tuzatish.
- hyperledger#1657 Rasmni zangga yangilang 1.57. # 1630: O'z-o'zidan ishlaydigan yuguruvchilarga qayting.
- CI yaxshilanishlari.
- `lld` foydalanish uchun o'zgartirilgan qamrov.
- CI qaramligini tuzatish.
- CI segmentatsiyasini yaxshilash.
- CI da qattiq Rust versiyasidan foydalanadi.
- Docker nashr qilish va iroha2-dev push CI ni tuzatish. Qamrov va dastgohni PRga o'tkazing
- CI docker testida keraksiz to'liq Iroha tuzilmasini olib tashlang.

  Iroha qurilishi endi docker tasvirining o'zida bajarilgani uchun foydasiz bo'lib qoldi. Shunday qilib, CI faqat testlarda ishlatiladigan mijoz cli-ni yaratadi.
- CI quvur liniyasida iroha2 filiali uchun yordam qo'shing.
  - uzoq sinovlar faqat iroha2 ga PR orqali o'tdi
  - docker tasvirlarini faqat iroha2 dan nashr qilish
- Qo'shimcha CI keshlari.

### Veb-Asambleya


### Versiya xatosi

- Pre-rc.13 versiyasi.
- Pre-rc.11 versiyasi.
- RC.9 versiyasi.
- RC.8 versiyasi.
- Versiyalarni RC7 ga yangilang.
- Chiqarilishdan oldingi tayyorgarlik.
- Mold 1.0 ni yangilang.
- Bump bog'liqliklari.
- api_spec.md-ni yangilash: so'rov/javob organlarini tuzatish.
- Rust versiyasini 1.56.0 ga yangilang.
- Hissa qo'shadigan qo'llanmani yangilang.
- Yangi API va URL formatiga mos kelish uchun README.md va `iroha/config.json` ni yangilang.
- Docker nashr qilish maqsadini hyperledger/iroha2 #1453 ga yangilang.
- Ish jarayonini asosiyga mos keladigan tarzda yangilaydi.
- API spetsifikatsiyasini yangilang va sog'liqning so'nggi nuqtasini tuzating.
- Rust yangilanishi 1.54.
- Hujjatlar(iroha_crypto): `Signature` hujjatlarini yangilang va `verify` arglarini tekislang
- Ursa versiyasi 0.3.5 dan 0.3.6 gacha.
- Ish oqimlarini yangi yuguruvchilarga yangilang.
- Keshlash va tezroq ci qurish uchun docker faylini yangilang.
- libssl versiyasini yangilang.
- Docker fayllari va async-std-ni yangilang.
- Yangilangan klipni tuzatish.
- Aktiv tuzilmasini yangilaydi.
  - Aktivdagi kalit-qiymat ko'rsatmalarini qo'llab-quvvatlash
  - Enum sifatida aktivlar turlari
  - ISI aktividagi to'lib-toshgan zaiflikni tuzatish
- Yangilanishlarga hissa qo'shadigan qo'llanma.
- Eskirgan kutubxonani yangilash.
- Oq qog'ozni yangilang va liniya bilan bog'liq muammolarni tuzating.
- cucumber_rust lib-ni yangilang.
- Kalit yaratish uchun README yangilanishlari.
- Github Actions ish oqimlarini yangilang.
- Github Actions ish oqimlarini yangilang.
- talablar.txt faylini yangilang.
- common.yaml-ni yangilang.
- Saradan hujjatlar yangilanishi.
- Ko'rsatma mantiqini yangilang.
- Oq qog'ozni yangilash.
- Tarmoq funksiyalari tavsifini yangilaydi.
- Sharhlar asosida oq qog'ozni yangilang.
- WSV yangilanishini ajratish va Scale-ga o'tish.
- Gitignore-ni yangilang.
- WP-da kura tavsifini biroz yangilang.
- Oq qog'ozda kura haqidagi tavsifni yangilang.

### Sxema

- hyperledger#2114 Sxemalarda tartiblangan to'plamlarni qo'llab-quvvatlash.
- hyperledger # 2108 Sahifalar qo'shing.
- hyperledger#2114 Sxemalarda tartiblangan to'plamlarni qo'llab-quvvatlash.
- hyperledger # 2108 Sahifalar qo'shing.
- Sxema, versiya va makro no_std mos kelsin.
- Sxemadagi imzolarni tuzatish.
- Sxemada `FixedPoint` ko'rinishi o'zgartirildi.
- Introspektsiya sxemasiga `RawGenesisBlock` qo'shildi.
- IR-115 sxemasini yaratish uchun ob'ekt modellari o'zgartirildi.

### Testlar

- hyperledger # 2544 O'quv qo'llanmalari.
- hyperledger#2272 "FindAssetDefinitionById" so'rovi uchun testlarni qo'shing.
- `roles` integratsiya testlarini qo'shing.
- Ui testlari formatini standartlashtiring, ui testlarini qutilarni olish uchun ko'chiring.
- Soxta testlarni tuzatish (fyuchersning tartibsiz xatosi).
- DSL qutisi olib tashlandi va testlar `data_model` ga ko'chirildi
- Barqaror tarmoq testlari haqiqiy kod uchun o'tishiga ishonch hosil qiling.
- iroha_p2p ga testlar qo'shildi.
- Sinov muvaffaqiyatsiz tugamaguncha testlardagi jurnallarni yozib oladi.
- Sinovlar uchun so'rov qo'shing va kamdan-kam hollarda buziladigan testlarni tuzating.
- Parallel o'rnatishni sinab ko'radi.
- Iroha init va iroha_client testlaridan ildizni olib tashlang.
- Sinovlar haqida ogohlantirishlarni tuzatish va ci-ga cheklar qo'shish.
- Benchmark testlari paytida `tx` tekshirish xatolarini tuzating.
- hyperledger#860: Iroha So'rovlar va testlar.
- Iroha maxsus ISI qo'llanmasi va Bodring testlari.
- No-std mijoz uchun testlarni qo'shing.
- Bridge ro'yxatga olish o'zgarishlari va testlari.
- Tarmoq masxarasi bilan konsensus testlari.
- Testlarni bajarish uchun temp dir-dan foydalanish.
- Skameykalar ijobiy holatlarni tekshiradi.
- Testlar bilan Merkle Tree boshlang'ich funksionalligi.
- Ruxsat etilgan testlar va World State View ishga tushirilishi.

### Boshqa- Parametrlashtirishni belgilarga o'tkazing va FFI IR turlarini olib tashlang.
- Kasaba uyushmalari uchun qo'llab-quvvatlash qo'shing, `non_robust_ref_mut` joriy * conststring FFI konversiyasini amalga oshirish.
- IdOrdEqHash-ni yaxshilang.
- FilterOpt::BySome-ni (de-)seriyalashdan olib tashlang.
- Shaffof emas qiling.
- ContextValue-ni shaffof qiling.
- Ifodani qiling::Raw tegi ixtiyoriy.
- Ba'zi ko'rsatmalar uchun shaffoflikni qo'shing.
- RoleId seriyasini yaxshilash (de-serializatsiya).
- Validator::Id seriyasini yaxshilash (de-)
- PermissionTokenId seriyasini yaxshilash (de-serializatsiya).
- TriggerId seriyasini yaxshilash (de-)
- Asset(-ta'rif) identifikatorlarini seriyalilashtirishni yaxshilash (de-)
- AccountId seriyasini yaxshilash (de-serializatsiya).
- Ipfs va DomainId ning seriyaliligini yaxshilash (de-).
- Mijoz konfiguratsiyasidan logger konfiguratsiyasini olib tashlang.
- FFIda shaffof tuzilmalarni qo'llab-quvvatlash.
- Refactor &Option<T> ga Variant<&T>
- Klip haqida ogohlantirishlarni tuzating.
- `Find` xato tavsifiga qo'shimcha ma'lumot qo'shing.
- `PartialOrd` va `Ord` ilovalarini tuzating.
- `cargo fmt` o'rniga `rustfmt` dan foydalaning
- `roles` xususiyatini olib tashlang.
- `cargo fmt` o'rniga `rustfmt` dan foydalaning
- Ishchi faylni ishlab chiqaruvchi docker misollari bilan jild sifatida baham ko'ring.
- Execute-da Diff bilan bog'liq turni olib tashlang.
- Multival qaytish o'rniga maxsus kodlashdan foydalaning.
- Serde_json ni iroha_crypto qaramligi sifatida olib tashlang.
- Versiya atributidagi faqat ma'lum maydonlarga ruxsat bering.
- Oxirgi nuqtalar uchun turli portlarni aniqlang.
- `Io` hosilasini olib tashlang.
- key_pairsning dastlabki hujjatlari.
- O'z-o'zidan ishlaydigan yuguruvchilarga qayting.
- Koddagi yangi kliplarni tuzatish.
- Xizmatchilardan i1i1 ni olib tashlang.
- Aktyor doc va kichik tuzatishlar qo'shing.
- Eng so'nggi bloklarni surish o'rniga so'rovnoma.
- 7 ta tengdoshning har biri uchun sinovdan o'tgan tranzaksiya holati hodisalari.
- `join_all` o'rniga `FuturesUnordered`
- GitHub Runners-ga o'tish.
- /so'rovning so'nggi nuqtasi uchun VersionedQueryResult va QueryResult dan foydalaning.
- Telemetriyani qayta ulang.
- Dependabot konfiguratsiyasini tuzatish.
- Signoffni kiritish uchun commit-msg git hook qo'shing.
- Surish quvurini mahkamlang.
- Dependabotni yangilang.
- Navbatni surishda kelajakdagi vaqt tamg'asini aniqlang.
- hyperledger # 1197: Kura xatolarni boshqaradi.
- Registerni bekor qilish ko'rsatmasini qo'shing.
- Tranzaktsiyalarni farqlash uchun ixtiyoriy nonce qo'shing. Yopish №1493.
- Keraksiz `sudo` olib tashlandi.
- Domenlar uchun metama'lumotlar.
- `create-docker` ish jarayonida tasodifiy sakrashlarni tuzating.
- Ishlamay qolgan quvur liniyasi taklif qilganidek, `buildx` qo'shildi.
- hyperledger # 1454: Muayyan holat kodi va maslahatlar bilan so'rov xatosi javobini tuzating.
- hyperledger # 1533: tranzaksiyani xesh orqali toping.
- `configure` oxirgi nuqtasini tuzating.
- Boolean asosidagi aktivlarning sotilishi tekshiruvini qo'shing.
- Terilgan kripto-primitivlarni qo'shish va turdagi xavfsiz kriptografiyaga o'tish.
- Jurnalni yaxshilash.
- hyperledger # 1458: `mailbox` sifatida sozlash uchun aktyor kanali hajmini qo'shing.
- hyperledger # 1451: `faulty_peers = 0` va `trusted peers count > 1` bo'lsa, noto'g'ri konfiguratsiya haqida ogohlantirish qo'shing
- Muayyan blok xeshini olish uchun ishlov beruvchini qo'shing.
- FindTransactionByHash yangi so'rovi qo'shildi.
- hyperledger # 1185: qutilar nomi va yo'lini o'zgartiring.
- Jurnallarni tuzatish va umumiy yaxshilanishlar.
- hyperledger # 1150: Har bir faylga 1000 ta blokni guruhlang
- Navbatdagi stress testi.
- Jurnal darajasini tuzatish.
- Mijoz kutubxonasiga sarlavha spetsifikatsiyasini qo'shing.
- Navbatdagi vahima nosozliklarini tuzatish.
- Tuzatish navbati.
- Docker-fayl relizlar tuzilishini tuzatish.
- Https mijozini tuzatish.
- Tezlashtirish ci.
- 1. Iroha_cryptodan tashqari barcha ursa bog'liqliklari olib tashlandi.
- Davomiyliklarni ayirishda to'lib ketishni tuzating.
- Mijozda maydonlarni ommaviy qilish.
- Iroha2-ni Dockerhub-ga tungidek suring.
- http holat kodlarini tuzatish.
- iroha_error ni thiserror, eyre va color-eyre bilan almashtiring.
- Navbatni ko'ndalang chiziq bilan almashtiring.
- Ba'zi keraksiz tuklar to'lovlarini olib tashlang.
- Aktiv ta'riflari uchun metama'lumotlarni kiritadi.
- test_tarmoq qutisidan argumentlarni olib tashlash.
- Keraksiz bog'liqliklarni olib tashlang.
- iroha_client_cli :: hodisalarini tuzatish.
- hyperledger # 1382: Eski tarmoq ilovasini olib tashlang.
- hyperledger # 1169: aktivlar uchun qo'shimcha aniqlik.
- Tengdoshlarni ishga tushirishda yaxshilanishlar:
  - Genesis ochiq kalitini faqat env dan yuklashga ruxsat beradi
  - konfiguratsiya, genesis va ishonchli_peers yo'li endi cli parametrlarida ko'rsatilishi mumkin
- hyperledger # 1134: Iroha P2P integratsiyasi.
- So'rovning so'nggi nuqtasini GET o'rniga POST ga o'zgartiring.
- On_startni aktyorda sinxron tarzda bajaring.
- Warpga o'tish.
- Broker xatoliklarini tuzatish bilan qayta ishlash.
- "Bir nechta broker tuzatishlarini kiritadi" majburiyatini qaytarish (9c148c33826067585b5868d297dcdd17c0efe246)
- Bir nechta broker tuzatishlarini kiritadi:
  - Aktyor to'xtash joyidagi broker obunasini bekor qiling
  - Bitta aktyor turidan bir nechta obunalarni qo'llab-quvvatlash (ilgari TODO)
  - Broker har doim o'zini aktyor identifikatori sifatida qo'yadigan xatoni tuzating.
- Broker xatosi (test vitrini).
- Ma'lumotlar modeli uchun hosilalarni qo'shing.
- Torii dan rwlockni olib tashlang.
- OOB so'rovi ruxsatini tekshirish.
- hyperledger # 1272: tengdoshlarni hisoblashni amalga oshirish,
- Ko'rsatmalar ichidagi so'rovlar uchun ruxsatlarni rekursiv tekshirish.
- To'xtash aktyorlarini rejalashtirish.
- hyperledger # 1165: Tengdoshlarni hisoblashni amalga oshirish.
- Torii so'nggi nuqtasida hisob qaydnomasi bo'yicha so'rovlar ruxsatlarini tekshiring.
- Tizim ko'rsatkichlarida protsessor va xotiradan foydalanishni oshkor qilish olib tashlandi.
 - WS xabarlari uchun JSONni Norito bilan almashtiring.
- Ko'rinishdagi o'zgarishlarni tasdiqlovchi hujjatlarni saqlang.
- hyperledger # 1168: Agar tranzaktsiya imzoni tekshirish shartidan o'tmasa, jurnal qo'shildi.
- Kichik muammolar tuzatildi, ulanishni tinglash kodi qo'shildi.
- Tarmoq topologiyasi yaratuvchisini joriy qilish.
- Iroha uchun P2P tarmog'ini joriy qiling.
- Blok o'lchami ko'rsatkichini qo'shadi.
- PermissionValidator xususiyati IsAllowed deb o'zgartirildi. va tegishli nomdagi boshqa o'zgarishlar
- API spetsifikatsiyasi veb-rozetkasi tuzatishlari.
- Docker tasviridan keraksiz bog'liqliklarni olib tashlaydi.
- Fmt Crate import_granularity-dan foydalanadi.
- Umumiy ruxsatni tekshirgichni taqdim etadi.
- Aktyor doirasiga o'tish.
- Broker dizaynini o'zgartiring va aktyorlarga ba'zi funksiyalarni qo'shing.
- Codecov holatini tekshirishni sozlaydi.
- Grcov bilan manbaga asoslangan qamrovdan foydalanadi.
- Bir nechta qurilish-arg formatlari tuzatildi va oraliq qurilish konteynerlari uchun ARG qayta e'lon qilindi.
- SubscriptionAccepted xabarini taqdim etadi.
- Operatsiyadan so'ng hisoblardan nol qiymatli aktivlarni olib tashlang.
- Ruxsat etilgan docker qurish argumentlari formati.
- Agar bolalar bloki topilmasa, xato xabari tuzatildi.
- Qurilish uchun ishlab chiqarilgan OpenSSL qo'shildi, pkg-config bog'liqligini tuzatdi.
- Dockerhub va qamrov farqlari uchun ombor nomini tuzatish.
- TrustedPeers-ni yuklab bo'lmasa, aniq xato matni va fayl nomi qo'shildi.
- Matn ob'ektlari hujjatlardagi havolalarga o'zgartirildi.
- Docker nashrida noto'g'ri foydalanuvchi nomi sirini tuzating.
- Oq qog'ozdagi kichik xatoni tuzating.
- Yaxshiroq fayl tuzilishi uchun mod.rs dan foydalanishga ruxsat beradi.
- main.rs-ni alohida qutiga o'tkazing va ommaviy blokcheynga ruxsat bering.
- Mijoz cli ichiga so'rov qo'shing.
- Cli uchun clapdan structoptsga o'tish.
- Telemetriyani beqaror tarmoq sinovi bilan cheklang.
- Xususiyatlarni smartcontracts moduliga o'tkazing.
- Sed -i "s/world_state_view/wsv/g"
- Aqlli shartnomalarni alohida modulga o'tkazing.
- Iroha tarmoq tarkibi uzunligi xatosi tuzatildi.
- Aktyor identifikatori uchun mahalliy vazifa xotirasini qo'shadi. Tugallanishni aniqlash uchun foydalidir.
- CI ga qulfni aniqlash testini qo'shing
- Introspect makrosini qo'shing.
- Ish jarayoni nomlarini, shuningdek, formatlash tuzatishlarini ham ajratadi
- Api so'rovini o'zgartirish.
- Async-std dan tokioga ko'chish.
- ci ga telemetriya tahlilini qo'shing.
- Iroha uchun fyuchers telemetriyasini qo'shing.
- Har bir asinx funksiyasiga iroha fyucherslarini qo'shing.
- So'rovlar sonini kuzatish uchun iroha fyucherslarini qo'shing.
- README-ga qo'lda joylashtirish va sozlash qo'shildi.
- Muxbirni tuzatish.
- Olingan xabar makrosini qo'shing.
- Oddiy aktyor ramkasini qo'shing.
- Dependabot konfiguratsiyasini qo'shing.
- Yaxshi vahima va xato muxbirlarini qo'shing.
- Rust versiyasini 1.52.1 ga ko'chirish va tegishli tuzatishlar.
- Alohida mavzularda protsessorni talab qiladigan vazifalarni bloklaydigan spawn.
- Crates.io dan unique_port va cargo-lints dan foydalaning.
- Qulfsiz WSV uchun tuzatish:
  - APIdagi keraksiz Dashmaplarni va qulflarni olib tashlaydi
  - yaratilgan bloklar soni ko'p bo'lgan xatolikni tuzatadi (rad etilgan tranzaktsiyalar qayd etilmagan)
  - Xatolar uchun to'liq xato sababini ko'rsatadi
- Telemetriya obunachisini qo'shing.
- Rollar va ruxsatlar uchun so'rovlar.
- Bloklarni kuradan wsvga o'tkazing.
- WSV ichidagi blokirovkasiz ma'lumotlar tuzilmalariga o'zgartirish.
- Tarmoq vaqti tugashini tuzatish.
- Sog'lik holatini tuzatish.
- Rollarni tanishtiradi.
- Dev bo'limidan push docker tasvirlarini qo'shing.
- Ko'proq tajovuzkor linting qo'shing va koddan vahimalarni olib tashlang.
- Ko'rsatmalar uchun Execute xususiyatini qayta ishlash.
- Eski kodni iroha_config dan olib tashlang.
- IR-1060 barcha mavjud ruxsatlar uchun grant tekshiruvlarini qo'shadi.
- Iroha_network uchun ulimit va vaqt tugashini tuzating.
- Ci vaqt tugashi testini tuzatish.
- Ularning ta'rifi olib tashlanganidan keyin barcha aktivlarni olib tashlang.
- Aktiv qo'shishda wsv vahimasini tuzating.
- Kanallar uchun Arc va Rwlock-ni olib tashlang.
- Iroha tarmoqni tuzatish.
- Ruxsat tekshiruvchilari tekshiruvlarda havolalardan foydalanadilar.
- Grant bo'yicha ko'rsatma.
- NewAccount, Domain va AssetDefinition IR-1036 uchun qator uzunligi chegaralari va identifikatorlarni tekshirish uchun qo'shilgan konfiguratsiya.
- Jurnalni tracing lib bilan almashtiring.
- Hujjatlarni tekshirishni qo'shing va dbg makrosini rad eting.
- Beriladigan ruxsatnomalarni joriy qiladi.
- Iroha_config qutisini qo'shing.
- Barcha kiruvchi birlashma soʻrovlarini tasdiqlash uchun kod egasi sifatida @alerdenisov ni qoʻshing.
- Konsensus paytida tranzaksiya hajmini tekshirishni tuzatish.
- Async-std yangilanishini qaytarish.
- Ba'zi konstlarni 2 IR-1035 quvvatiga almashtiring.
- IR-1024 tranzaktsiyalar tarixini olish uchun so'rov qo'shing.- Saqlash uchun ruxsatnomalarni tekshirishni qo'shing va ruxsat tasdiqlovchilarini qayta tuzing.
- Hisob qaydnomasini ro'yxatdan o'tkazish uchun NewAccount qo'shing.
- Aktivni aniqlash uchun turlarni qo'shing.
- Konfiguratsiya qilinadigan metama'lumotlar chegaralarini joriy qiladi.
- Tranzaksiya metama'lumotlarini joriy qiladi.
- So'rovlar ichiga iboralar qo'shing.
- Lints.toml qo'shing va ogohlantirishlarni tuzating.
- config.json dan ishonchli_peersni ajrating.
- Telegramdagi Iroha 2 hamjamiyatiga URL manzilidagi xatoni tuzatdi.
- Klip haqida ogohlantirishlarni tuzating.
- Hisob uchun kalit-qiymat meta-ma'lumotlarini qo'llab-quvvatlashni joriy qiladi.
- Bloklarning versiyalarini qo'shing.
- Qayta takrorlashni tuzatish.
- mul,div,mod,raise_to ifodalarini qo'shing.
- Versiyalash uchun into_v* qo'shing.
- Xato :: msg ni xato makrosi bilan almashtiring.
- iroha_http_serverni qayta yozing va torii xatolarini qayta ishlang.
 - Norito versiyasini 2 ga yangilaydi.
- Oq qog'oz versiyalarining tavsifi.
- O'zgarmas sahifalash. Xatolar tufayli sahifalash keraksiz bo'lishi mumkin bo'lgan holatlarni tuzating, buning o'rniga bo'sh to'plamlarni qaytarmang.
- Enumlar uchun derive (xato) qo'shing.
- Kecha versiyasini tuzating.
- Iroha_error qutisini qo'shing.
- Versiyalangan xabarlar.
- Konteyner versiyalarining ibtidoiylarini kiritadi.
- Benchmarklarni tuzatish.
- Sahifani qo'shing.
- Varint kodlash dekodlashni qo'shing.
- So'rov vaqt tamg'asini u128 ga o'zgartiring.
- Quvur hodisalari uchun RejectionReason raqamini qo'shing.
- Genesis fayllaridan eskirgan chiziqlarni olib tashlaydi. Belgilangan manzil avvalgi majburiyatlarda ISI reestridan olib tashlangan.
- ISIni ro'yxatdan o'tkazish va ro'yxatdan chiqarishni soddalashtiradi.
- 4 peer tarmog'ida yuborilmasligini to'g'irlash.
- Topologiyani o'zgartirish ko'rinishida aralashtiriladi.
- FromVariant makrosi uchun boshqa konteynerlarni qo'shing.
- Mijoz cli uchun MST yordamini qo'shing.
- FromVariant makrosini qo'shing va kodlar bazasini tozalang.
- Kod egalariga i1i1 qo'shing.
- G'iybat operatsiyalari.
- Ko'rsatmalar va ifodalar uchun uzunlik qo'shing.
- Vaqtni bloklash va vaqt parametrlarini bajarish uchun hujjatlarni qo'shing.
- Verify and Accept xususiyatlari TryFrom bilan almashtirildi.
- Faqat tengdoshlarning minimal sonini kutishni joriy qiling.
- Api-ni iroha2-java bilan sinab ko'rish uchun github amalini qo'shing.
- Docker-compose-single.yml uchun genezis qo'shing.
- Hisob uchun standart imzoni tekshirish sharti.
- Bir nechta imzo qo'ygan hisob uchun test qo'shing.
- MST uchun mijoz API yordamini qo'shing.
- Dockerda qurish.
- Docker kompozitsiyasiga genezis qo'shing.
- Shartli MSTni joriy qilish.
- wait_for_active_peers impl qo'shing.
- iroha_http_serverda isahc mijozi uchun test qo'shing.
- Client API spetsifikatsiyasi.
- Ifodalarda so'rovlar bajarilishi.
- Ifodalar va ISIlarni birlashtiradi.
- ISI uchun ifodalar.
- Hisob konfiguratsiyasi mezonlarini tuzatish.
- Mijoz uchun hisob konfiguratsiyasini qo'shing.
- `submit_blocking` ni tuzatish.
- Quvur hodisalari yuboriladi.
- Iroha mijoz veb-rozetkasi ulanishi.
- Quvur va ma'lumotlar hodisalari uchun hodisalarni ajratish.
- Ruxsatlar uchun integratsiya testi.
- Kuyish va yalpiz uchun ruxsat tekshiruvlarini qo'shing.
- ISI ruxsatini ro'yxatdan o'chirish.
- Jahon PR tuzilmasi uchun mezonlarni tuzatish.
- Dunyo tuzilishi bilan tanishtiring.
- Genezis blokini yuklash komponentini amalga oshirish.
- Genesis hisobini kiriting.
- Ruxsatlar validator quruvchisini tanishtiring.
- Github Actions yordamida Iroha2 PRlariga teglar qo'shing.
- Ruxsatlar ramkasini joriy qilish.
- Queue tx tx raqam chegarasi va Iroha ishga tushirish tuzatishlari.
- Xeshni strukturaga o'rash.
- Jurnal darajasini yaxshilash:
  - Konsensusga ma'lumot darajasi jurnallarini qo'shing.
  - Tarmoq aloqa jurnallarini kuzatuv darajasi sifatida belgilang.
  - WSV dan blok vektorni olib tashlang, chunki u takrorlanadi va u barcha blokcheynni jurnallarda ko'rsatdi.
  - Ma'lumot jurnali darajasini sukut bo'yicha o'rnating.
- Tekshirish uchun o'zgaruvchan WSV havolalarini olib tashlang.
- Heim versiyasi o'sishi.
- Konfiguratsiyaga standart ishonchli tengdoshlarni qo'shing.
- Client API-ni http-ga o'tkazish.
- CLI ga uzatish IS ni qo'shing.
- Iroha Tengdosh ko'rsatmalarining konfiguratsiyasi.
- etishmayotgan ISI bajarish usullari va testlarini amalga oshirish.
- URL so'rovi parametrlarini tahlil qilish
- `HttpResponse::ok()`, `HttpResponse::upgrade_required(..)` qo'shing
- Eski Instruction va Query modellarini Iroha DSL yondashuviga almashtirish.
- BLS imzolarini qo'llab-quvvatlashni qo'shing.
- http server qutisini tanishtiring.
- Symlink bilan yamalgan libssl.so.1.0.0.
- Tranzaksiya uchun hisob imzosini tasdiqlaydi.
- Refactor tranzaksiya bosqichlari.
- Dastlabki domenlar yaxshilandi.
- DSL prototipini joriy qilish.
- Torii ko'rsatkichlarini yaxshilang: benchmarklarda tizimga kirishni o'chirib qo'ying, muvaffaqiyat nisbati tasdiqini qo'shing.
- Sinov qamrovini yaxshilash: `tarpaulin` ni `grcov` bilan almashtiring, `codecov.io` ga test qamrovi hisobotini e'lon qiling.
- RTD mavzusini tuzatish.
- Iroha kichik loyihalari uchun yetkazib berish artefaktlari.
- `SignedQueryRequest` bilan tanishtiring.
- Imzoni tekshirish bilan xatoni tuzating.
- Orqaga qaytarish tranzaksiyalarini qo'llab-quvvatlash.
- Yaratilgan kalit-juftni json sifatida chop eting.
- `Secp256k1` kalit juftligini qo'llab-quvvatlash.
- Turli kripto algoritmlari uchun dastlabki yordam.
- DEX xususiyatlari.
- Qattiq kodlangan konfiguratsiya yo'lini cli param bilan almashtiring.
- Dastgoh ustasi ish jarayonini tuzatish.
- Docker voqea ulanish testi.
- Iroha Monitor qo'llanmasi va CLI.
- Voqealar muhitini yaxshilash.
- Voqealar filtri.
- Tadbir aloqalari.
- Asosiy ish oqimida tuzatish.
- iroha2 uchun Rtd.
- Blok tranzaksiyalari uchun Merkle daraxt ildizi xeshi.
- Docker hubga nashr qilish.
- Maintenance Connect uchun CLI funksionalligi.
- Maintenance Connect uchun CLI funksionalligi.
- Makro jurnali uchun Eprintln.
- Jurnalni yaxshilash.
- IR-802 blokirovkalari holatiga obuna bo'lish.
- Tranzaktsiyalar va bloklarni yuborish voqealari.
- Sumeragi xabarni qayta ishlashni xabarga ko'chiradi.
- Umumiy ulanish mexanizmi.
- No-std mijozi uchun Iroha domen ob'ektlarini chiqarib oling.
- TTL operatsiyalari.
- Blok konfiguratsiyasi uchun maksimal tranzaktsiyalar.
- Yaroqsiz bloklar xeshlarini saqlang.
- Bloklarni to'plamlarda sinxronlash.
- Ulanish funksiyasining konfiguratsiyasi.
- Iroha funksiyasiga ulaning.
- Tasdiqlash tuzatishlarini bloklash.
- Blok sinxronizatsiyasi: diagrammalar.
- Iroha funksiyasiga ulaning.
- Ko'prik: mijozlarni olib tashlash.
- Sinxronizatsiyani bloklash.
- AddPeer ISI.
- Ko'rsatmalar nomini o'zgartirish uchun buyruqlar.
- Oddiy ko'rsatkichlarning yakuniy nuqtasi.
- Ko'prik: ro'yxatdan o'tgan ko'priklar va tashqi aktivlarni oling.
- Docker quvur liniyasida test tuzadi.
- Sumeragi testida ovozlar yetarli emas.
- Blok zanjiri.
- Ko'prik: tashqi o'tkazmalarni qo'lda boshqarish.
- Oddiy texnik xizmat ko'rsatishning so'nggi nuqtasi.
- Serde-jsonga ko'chish.
- ISIni zararsizlantirish.
- Ko'prik mijozlari, AddSignatory ISI va CanAddSignatory ruxsatini qo'shing.
- Sumeragi: b to'plamidagi tengdoshlar TODO bilan bog'liq tuzatishlar.
- Sumeragi da imzolashdan oldin blokni tasdiqlaydi.
- Tashqi aktivlarni birlashtirish.
- Sumeragi xabarlarida imzoni tekshirish.
- Ikkilik aktivlar do'koni.
- PublicKey taxallusni turi bilan almashtiring.
- Nashr qilish uchun qutilarni tayyorlang.
- NetworkTopology ichidagi minimal ovozlar mantig'i.
- TransactionReceipt tekshiruvi refaktoringi.
- OnWorldStateViewChange tetik o'zgarishi: Yo'riqnoma o'rniga IrohaQuery.
- NetworkTopology-da initsializatsiyadan alohida qurilish.
- Iroha hodisalariga tegishli Iroha Maxsus ko'rsatmalarini qo'shing.
- Blok yaratish vaqti tugashini boshqarish.
- Lug'at va Iroha moduli hujjatlarini qanday qo'shish kerak.
- Qattiq kodlangan ko'prik modelini Iroha modeli bilan almashtiring.
- NetworkTopology tuzilishi bilan tanishing.
- Ko'rsatmalardan o'zgartirish bilan Ruxsat ob'ektini qo'shing.
- Sumeragi Xabarlar modulidagi xabarlar.
- Kura uchun Genesis Block funksiyasi.
- Iroha qutilari uchun README fayllarini qo'shing.
- Bridge va RegisterBridge ISI.
- Iroha bilan dastlabki ish tinglovchilarni o'zgartiradi.
- OOB ISI ga ruxsat cheklarini kiritish.
- Docker bir nechta tengdoshlarni tuzatish.
- Peer to peer docker misoli.
- Tranzaksiya kvitansiyasi bilan ishlash.
- Iroha ruxsatnomalari.
- Dex moduli va ko'priklar uchun qutilar.
- Bir nechta tengdoshlar bilan aktiv yaratish bilan integratsiya testini tuzating.
- Asset modelini EC-S-ga qayta tatbiq etish.
- Taymerni qayta ishlashga ruxsat bering.
- Blok sarlavhasi.
- Domen ob'ektlari uchun ISI bilan bog'liq usullar.
- Kura rejimini ro'yxatga olish va ishonchli tengdoshlar konfiguratsiyasi.
- Hujjatlarni qo'yish qoidasi.
- CommittedBlock qo'shing.
- `sumeragi` dan kura ajratish.
- Blok yaratishdan oldin tranzaktsiyalar bo'sh emasligini tekshiring.
- Iroha maxsus ko'rsatmalarini qaytadan amalga oshiring.
- Tranzaktsiyalar uchun mezonlar va o'tishlarni bloklaydi.
- Tranzaktsiyalarning hayot aylanishi va qayta ishlangan holatlar.
- Hayot aylanishi va holatlarni bloklaydi.
- Tekshirish xatosini tuzatdi, blok_build_time_ms konfiguratsiya parametri bilan sinxronlangan `sumeragi` tsikli.
- `sumeragi` moduli ichidagi Sumeragi algoritmining inkapsulyatsiyasi.
- Kanallar orqali amalga oshirilgan Iroha tarmoq qutisi uchun masxara moduli.
- Async-std API ga o'tish.
- Tarmoqni soxtalashtirish xususiyati.
- Asinxron bilan bog'liq kodni tozalash.
- Tranzaktsiyalarni qayta ishlash siklida ishlashni optimallashtirish.
- Kalit juftlarini yaratish Iroha startidan olingan.
- Docker paketi Iroha bajariladi.
- Sumeragi asosiy stsenariysi bilan tanishtiring.
- Iroha CLI mijozi.
- dastgoh guruhi ijrosidan keyin iroha tushishi.
- `sumeragi` ni integratsiyalash.
- `sort_peers` ilovasini oldingi blok xesh bilan urugʻlangan rand aralashtirishga oʻzgartiring.
- Peer modulidagi xabar o'ramini olib tashlang.
- `torii::uri` va `iroha_network` ichidagi tarmoq bilan bog'liq ma'lumotlarni inkapsulyatsiya qiling.
- Qattiq kod bilan ishlash o'rniga amalga oshirilgan Peer ko'rsatmalarini qo'shing.
- Ishonchli tengdoshlar ro'yxati orqali tengdoshlar bilan muloqot qilish.
- Torii ichida tarmoq so'rovlarini qayta ishlash inkapsulyatsiyasi.
- Kripto modul ichida kriptologikaning inkapsulyatsiyasi.- Vaqt tamg'asi va foydali yuk sifatida oldingi blok xeshi bilan bloklash belgisi.
- Kripto funksiyalar modulning tepasida joylashgan va Imzo ichiga inkapsullangan ursa imzolovchi bilan ishlaydi.
- Sumeragi boshlang'ich.
- Saqlash majburiyatini olishdan oldin dunyo holati ko'rinishidagi klon bo'yicha tranzaksiya ko'rsatmalarini tekshirish.
- tranzaktsiyani qabul qilish to'g'risidagi imzolarni tekshirish.
- Seriyadan chiqarishni so'rashdagi xatoni tuzatdi.
- Iroha imzosini amalga oshirish.
- Blokcheyn ob'ekti kod bazasini tozalash uchun olib tashlandi.
- Transactions API-dagi o'zgarishlar: yaxshiroq yaratish va so'rovlar bilan ishlash.
- Tranzaktsiyaning bo'sh vektori bilan bloklarni yaratadigan xatoni tuzating
- Forvard kutilayotgan operatsiyalar.
 - U128 Norito kodlangan TCP paketidagi etishmayotgan bayt bilan xatolikni tuzating.
- Usullarni kuzatish uchun atribut makroslari.
- P2p moduli.
- Torii va mijozda iroha_network dan foydalanish.
- Yangi ISI ma'lumotlarini qo'shing.
- Tarmoq holati uchun maxsus turdagi taxallus.
- Box<dyn Error> String bilan almashtirildi.
- Tarmoqni tinglash holati.
- tranzaktsiyalar uchun dastlabki tekshirish mantig'i.
- Iroha_tarmoq qutisi.
- Io, IntoContract va IntoQuery belgilari uchun makroslarni oling.
- Iroha-mijoz uchun so'rovlarni amalga oshirish.
- Buyruqlarni ISI shartnomalariga aylantirish.
- Shartli multisig uchun taklif qilingan dizaynni qo'shing.
- Yuk ish joylariga o'tish.
- Modullar migratsiyasi.
- Atrof-muhit o'zgaruvchilari orqali tashqi konfiguratsiya.
- Torii uchun so'rovlarni qabul qilish va joylashtirish.
- Github ci tuzatish.
- Yuk pardasi sinovdan keyin bloklarni tozalaydi.
- Katalogni bloklar bilan tozalash funksiyasi bilan `test_helper_fns` modulini kiriting.
- Merkle daraxti orqali tekshirishni amalga oshiring.
- Ishlatilmagan hosilani olib tashlang.
- Asink/kutishni tarqating va kutilmagan `wsv::put`ni tuzating.
- `futures` kassasidan ulanishdan foydalaning.
- Parallel saqlashni amalga oshirish: diskka yozish va WSVni yangilash parallel ravishda amalga oshiriladi.
- Seriyalashtirish uchun egalik o'rniga havolalardan foydalaning.
- Fayllardan kod chiqarish.
- Ursa::blake2 dan foydalaning.
- Hissa qo‘shish bo‘yicha qo‘llanmada mod.rs haqidagi qoida.
- Xesh 32 bayt.
- Bleyk2 xesh.
- Disk bloklash uchun havolalarni qabul qiladi.
- Buyruqlar moduli va boshlang'ich Merkle daraxtini qayta tiklash.
- Qayta tiklangan modullar tuzilishi.
- To'g'ri formatlash.
- Read_all ga hujjat sharhlarini qo'shing.
- `read_all` ni amalga oshiring, saqlash testlarini qayta tashkil qiling va asinxron funksiyalari bilan testlarni asinxron testlarga aylantiring.
- Keraksiz o'zgaruvchan tasvirni olib tashlang.
- Muammoni ko'rib chiqing, klipni tuzating.
- Chiziqni olib tashlang.
- Format tekshiruvini qo'shing.
- Token qo'shing.
- Github harakatlari uchun rust.yml yarating.
- Diskni saqlash prototipini tanishtiring.
- O'tkazish aktivlari sinovi va funksionalligi.
- Strukturalarga standart ishga tushirgich qo'shing.
- MSTCache strukturasi nomini o'zgartirish.
- Unutilgan qarzni qo'shing.
- Iroha2 kodining dastlabki konturi.
- Dastlabki Kura API.
- Ba'zi asosiy fayllarni qo'shing va iroha v2 uchun ko'rinishni tavsiflovchi oq qog'ozning birinchi loyihasini chiqaring.
- Asosiy iroha v2 filiali.

## [1.5.0] - 2022-04-08

### CI/CD o'zgarishlari
- Jenkinsfile va JenkinsCI-ni olib tashlang.

### Qo'shilgan

- Burrow uchun RocksDB saqlash dasturini qo'shing.
- Bloom-filtr yordamida trafikni optimallashtirishni joriy qiling
- `batches_cache` da `OS` modulida joylashgan `MST` modul tarmog'ini yangilang.
- Trafikni optimallashtirishni taklif qilish.

### Hujjatlar

- Qurilishni tuzatish. JB farqlari, migratsiya amaliyoti, sog'liqni tekshirish so'nggi nuqtasi, iroha-swarm vositasi haqida ma'lumot qo'shing.

### Boshqa

- Hujjatni yaratish uchun talabni tuzatish.
- Qolgan muhim kuzatuv elementiga e'tibor berish uchun chiqarish hujjatlarini qisqartiring.
- "Docker tasviri mavjudligini tekshirish" ni tuzating /barcha skip_testing tuzing.
- /barcha skip_testingni qurish.
- / skip_testing qurish; Va boshqa hujjatlar.
- `.github/_README.md` qo'shing.
- `.packer` ni olib tashlang.
- Sinov parametridagi o'zgarishlarni olib tashlang.
- Sinov bosqichini o'tkazib yuborish uchun yangi parametrdan foydalaning.
- Ish jarayoniga qo'shish.
- Repozitoriy jo'natishni olib tashlang.
- Repository jo'natmasini qo'shing.
- Testerlar uchun parametr qo'shing.
- `proposal_delay` vaqt tugashini olib tashlang.

## [1.4.0] - 2022-01-31

### Qo'shilgan

- Sinxronlash tugun holatini qo'shing
- RocksDB uchun ko'rsatkichlarni qo'shadi
- http va ko'rsatkichlar orqali sog'liqni tekshirish interfeyslarini qo'shing.

### Tuzatishlar

- Iroha v1.4-rc.2 da ustunlar oilalarini tuzating
- Iroha v1.4-rc.1 da 10 bitli gullash filtrini qo'shing

### Hujjatlar

- Qurilish ma'lumotlari ro'yxatiga zip va pkg-config qo'shing.
- Readmeni yangilash: status yaratish, qoʻllanma yaratish va hokazolar uchun buzilgan havolalarni tuzating.
- Konfiguratsiya va Docker ko'rsatkichlarini tuzatish.

### Boshqa

- GHA docker tegini yangilang.
- g++11 bilan kompilyatsiya qilishda Iroha 1 kompilyatsiya xatolarini tuzatish.
- `max_rounds_delay` ni `proposal_creation_timeout` bilan almashtiring.
- Eski JB ulanish parametrlarini olib tashlash uchun namuna konfiguratsiya faylini yangilang.