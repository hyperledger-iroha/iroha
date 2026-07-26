<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
direction: ltr
source: docs/source/soracloud/uploaded_private_models.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 97d6a421ce93a0e85be6cc99e828f965c9d8617d0ee27a772a2c9f2f646e77b7
source_last_modified: "2026-03-24T18:59:46.535846+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud foydalanuvchisi tomonidan yuklangan modellar va shaxsiy ish vaqti

Ushbu eslatma So Ra ning foydalanuvchi tomonidan yuklangan model oqimi qanday tushishi kerakligini belgilaydi
parallel ish vaqtini ixtiro qilmasdan mavjud Soracloud model samolyoti.

## Dizayn maqsadi

Mijozlarga faqat Soracloud uchun yuklangan model tizimini qo'shing:

- o'zlarining namunaviy omborlarini yuklash;
- qadalgan model versiyasini agent kvartirasiga yoki yopiq arena jamoasiga bog'lash;
- shifrlangan kirishlar va shifrlangan model/holat bilan shaxsiy xulosa chiqarish; va
- davlat majburiyatlari, tushumlar, narxlar va audit ma'lumotlarini olish.

Bu `ram_lfe` xususiyati emas. `ram_lfe` umumiy yashirin funksiya bo'lib qoladi
quyi tizim `../universal_accounts_guide.md` da hujjatlashtirilgan. Yuklangan model
shaxsiy xulosa o'rniga Soracloudning mavjud model registrini kengaytirishi kerak,
artefakt, kvartira qobiliyati, FHE va shifrni ochish siyosati sirtlari.

## Qayta foydalanish uchun mavjud Soracloud sirtlari

Joriy Soracloud stekida allaqachon to'g'ri asosiy ob'ektlar mavjud:- `SoraModelRegistryV1`
  - har bir xizmat uchun vakolatli model nomi va targ'ib qilingan versiya holati.
- `SoraModelWeightVersionRecordV1`
  - versiyaning nasl-nasabi, ko'tarilishi, orqaga qaytishi, kelib chiqishi va takrorlanishi
    xeshlar.
- `SoraModelArtifactRecordV1`
  - deterministik artefakt metama'lumotlari allaqachon modelga/og'irlik quvuriga bog'langan.
- `SoraCapabilityPolicyV1.allow_model_inference`
  - majburiy bo'lishi kerak bo'lgan kvartira/xizmat qobiliyati bayrog'i
    yuklangan modellarga bog'langan kvartiralar.
- `SecretEnvelopeV1` va `CiphertextStateRecordV1`
  - deterministik shifrlangan bayt va shifrlangan matn holati tashuvchilar.
- `FheParamSetV1`, `FheExecutionPolicyV1`, `FheGovernanceBundleV1`,
  `DecryptionAuthorityPolicyV1` va `DecryptionRequestV1`
  - shifrlangan ijro va boshqariladigan chiqish uchun siyosat/boshqaruv qatlami
    ozod qilish.
- joriy Torii modeli marshrutlari:
  - `/v1/soracloud/model/weight/{register,promote,rollback,status}`
  - `/v1/soracloud/model/artifact/{register,status}`
- joriy Torii HF umumiy ijara marshrutlari:
  - `/v1/soracloud/hf/{deploy,status,lease/leave,lease/renew}`

Yuklangan model yo'li bu sirtlarni kengaytirishi kerak. U ortiqcha yuklamasligi kerak
HF birgalikda ijaraga oladi va u `ram_lfe`-ni modelga xizmat qiluvchi ish vaqti sifatida qayta ishlatmasligi kerak.

## Kanonik yuklash shartnomasi

Soracloud-ning shaxsiy yuklangan namunaviy shartnomasi faqat kanonik bo'lishi kerak
Hugging Face uslubidagi modellar ombori:- kerakli asosiy fayllar:
  - `config.json`
  - tokenizer fayllari
  - oila talab qilganda protsessor/preprotsessor fayllari
  - `*.safetensors`
- ushbu bosqichda qabul qilingan oila guruhlari:
  - RoPE/RMSNorm/SwiGLU/GQA semantikasi bilan faqat dekoder uchun sabab LMlar
  - LLaVA uslubidagi matn+tasvir modellari
  - Qwen2-VL uslubidagi matn+tasvir modellari
- ushbu bosqichda rad etilgan:
  - Yuklangan shaxsiy ish vaqti shartnomasi sifatida GGUF
  - ONNX
  - etishmayotgan tokenizer/protsessor aktivlari
  - qo'llab-quvvatlanmaydigan arxitekturalar/shakllar
  - audio/video multimodal paketlar

Nima uchun bu shartnoma:

- u allaqachon mavjud bo'lgan dominant server tomonidagi model tartibiga mos keladi
  seyftensorlar va Hugging Face repo atrofidagi ekotizim;
- bu model tekisligiga bitta deterministik normalizatsiya yo'lini bo'lishish imkonini beradi
  Shunday qilib, Ra, Torii va ish vaqti kompilyatsiyasi; va
- bu GGUF kabi mahalliy ish vaqti import formatlarini bilan birlashtirishdan qochadi
  Soracloud shaxsiy ish vaqti shartnomasi.

HF umumiy lizinglari umumiy/ommaviy manba import ish oqimlari uchun foydali bo'lib qoladi, ammo
Shaxsiy yuklangan model yo'li shifrlangan kompilyatsiya qilingan baytlarni zanjirda saqlaydi
umumiy manba hovuzidan model baytlarini ijaraga olishdan ko'ra.

## Qatlamli model-tekislik dizayni

### 1. Provenance va registr qatlami

Joriy ro'yxatga olish kitobi dizaynini almashtirish o'rniga kengaytiring:- `SoraModelProvenanceKindV1::UserUpload` qo'shing
  - joriy turlar (`TrainingJob`, `HfImport`) farqlash uchun etarli emas.
    model to'g'ridan-to'g'ri So Ra kabi mijoz tomonidan yuklangan va normallashtirilgan.
- `SoraModelRegistryV1` ni targ'ib qilingan versiya indeksi sifatida saqlang.
- `SoraModelWeightVersionRecordV1`ni nasl-nasab/ko'tarilish/orqaga qaytarish yozuvi sifatida saqlang.
- `SoraModelArtifactRecordV1` ni ixtiyoriy yuklangan shaxsiy ish vaqti bilan kengaytiring
  havolalar:
  - `private_bundle_root`
  - `chunk_manifest_root`
  - `compile_profile_hash`
  - `privacy_mode`

Artefakt yozuvi kelib chiqishni bog'laydigan deterministik langar bo'lib qolmoqda,
takrorlanuvchanlik metama'lumotlari va to'plam identifikatori birgalikda. Og'irlik rekordi
targ'ib qilingan versiyaning chiziqli ob'ekti bo'lib qoladi.

### 2. To'plam/bo'lak saqlash qatlami

Shifrlangan yuklangan namunaviy materiallar uchun birinchi darajali Soracloud yozuvlarini qo'shing:

- `SoraUploadedModelBundleV1`
  - `model_id`
  - `weight_version`
  - `family`
  - `modalities`
  - `runtime_format`
  - `bundle_root`
  - `chunk_count`
  - `plaintext_bytes`
  - `ciphertext_bytes`
  - `compile_profile_hash`
  - `pricing_policy`
  - `decryption_policy_ref`
- `SoraUploadedModelChunkV1`
  - `model_id`
  - `bundle_root`
  - `ordinal`
  - `offset_bytes`
  - `plaintext_len`
  - `ciphertext_len`
  - `ciphertext_hash`
  - shifrlangan foydali yuk (`SecretEnvelopeV1`)

Deterministik qoidalar:- ochiq matn baytlari shifrlashdan oldin qattiq 4 MiB bo'laklarga bo'linadi;
- bo'laklarni tartiblash qat'iy va tartib bilan boshqariladi;
- chunk/root digests takrorlash davomida barqaror; va
- har bir shifrlangan parcha joriy `SecretEnvelopeV1` ostida qolishi kerak
  `33,554,432` baytdan iborat shifrlangan matn shifti.

Ushbu bosqich to'liq shifrlangan baytlarni zanjir holatida bo'lak orqali saqlaydi
yozuvlar. U shaxsiy yuklangan model baytlarini SoraFS ga yuklamaydi.

Zanjir ommaviy bo'lganligi sababli, yuklash maxfiyligi haqiqiydan kelib chiqishi kerak
Ochiqdan olingan deterministik kalitlardan emas, balki Soracloud-da saqlanadigan qabul qiluvchi kaliti
metadata. Ish stoli reklama qilingan yuklash-shifrlash qabul qiluvchisini olishi kerak,
Tasodifiy har bir yuklash to'plami kaliti ostida bo'laklarni shifrlash va faqat nashr qilish
oluvchining metamaʼlumotlari va shifrlangan matn bilan birga oʻralgan kalitli konvert.

### 3. Kompilyatsiya/ish vaqti qatlami

Soracloud ostida maxsus transformator kompilyatori/ish vaqti qatlamini qo'shing:- uchun BFV tomonidan qo'llab-quvvatlangan deterministik past aniqlikdagi kompilyatsiya xulosasi bo'yicha standartlashtirish
  endi, chunki CKKS sxema muhokamasida mavjud, lekin amalga oshirilganda emas
  mahalliy ish vaqti;
- Qabul qilingan modellarni qamrab oluvchi deterministik Soracloud xususiy IRga kompilyatsiya qilish
  o'rnatishlar, chiziqli/proyektor qatlamlari, diqqat matmullari, RoPE, RMSNorm /
  LayerNorm taxminlari, MLP bloklari, ko'rish patchi proyeksiyasi va
  tasvirdan dekoderga proyektor yo'llari;
- deterministik sobit nuqtali xulosadan foydalaning:
  - int8 og'irliklari
  - int16 faollashtirish
  - int32 to'planishi
  - chiziqli bo'lmaganlar uchun tasdiqlangan ko'phadli yaqinlashuvlar

Ushbu kompilyator/ish vaqti `ram_lfe` dan alohida. U BFV primitivlarini qayta ishlatishi mumkin
va Soracloud FHE boshqaruv ob'ektlari, lekin u bir xil ijro mexanizmi emas
yoki marshrut oilasi.

### 4. Xulosa/sessiya qatlami

Shaxsiy yugurishlar uchun sessiya va nazorat nuqtasi yozuvlarini qo'shing:- `SoraPrivateCompileProfileV1`
  - `family`
  - `quantization`
  - `opset_version`
  - `max_context`
  - `max_images`
  - `vision_patch_policy`
  - `fhe_param_set`
  - `execution_policy`
- `SoraPrivateInferenceSessionV1`
  - `session_id`
  - `apartment`
  - `model_id`
  - `weight_version`
  - `bundle_root`
  - `input_commitments`
  - `token_budget`
  - `image_budget`
  - `status`
  - `receipt_root`
  - `xor_cost_nanos`
- `SoraPrivateInferenceCheckpointV1`
  - `session_id`
  - `step`
  - `ciphertext_state_root`
  - `receipt_hash`
  - `decrypt_request_id`
  - `released_token`
  - `compute_units`
  - `updated_at_ms`

Xususiy ijro quyidagilarni anglatadi:

- shifrlangan so'rov/tasvir kiritishlari;
- shifrlangan model og'irliklari va faollashtirish;
- chiqish uchun aniq dekodlash siyosati;
- umumiy ish vaqti tushumlari va xarajatlar hisobi.

Bu majburiyatsiz yoki tekshirilmasdan yashirin ijroni anglatmaydi.

## Mijoz mas'uliyati

Shunday qilib, Ra yoki boshqa mijoz avval deterministik mahalliy oldindan ishlov berishni amalga oshirishi kerak
yuklash Soracloud-ga yetib boradi:

- tokenizer ilovasi;
- qabul qilingan matn+tasvirlar oilalari uchun patch tensorlarida tasvirni oldindan qayta ishlash;
- deterministik to'plamni normallashtirish;
- token identifikatorlari va tasvir patch tensorlarining mijoz tomonidan shifrlanishi.

Torii shifrlangan ma'lumotlar va ochiq majburiyatlarni olishi kerak, xom ashyo emas
shaxsiy yo'l uchun matn yoki xom rasmlar.

## API va ISI rejasiMavjud model ro'yxatga olish kitobi marshrutlarini kanonik ro'yxatga olish kitobi qatlami sifatida saqlang va qo'shing
tepada yangi yuklash/ish vaqti marshrutlari:

- `POST /v1/soracloud/model/upload/init`
- `POST /v1/soracloud/model/upload/chunk`
- `POST /v1/soracloud/model/upload/finalize`
- `GET /v1/soracloud/model/upload/encryption-recipient`
- `POST /v1/soracloud/model/compile`
- `POST /v1/soracloud/model/allow`
- `POST /v1/soracloud/model/run-private`
- `GET /v1/soracloud/model/run-status`
- `POST /v1/soracloud/model/decrypt-output`

Ularni mos keladigan Soracloud ISI bilan qo'llab-quvvatlang:

- to'plamni ro'yxatdan o'tkazish
- parcha qo'shish/yakunlash
- qabulni tuzish
- shaxsiy yugurish boshlanishi
- nazorat punkti yozuvi
- chiqish chiqarish

Oqim quyidagicha bo'lishi kerak:

1. upload/init deterministik to'plam seansini va kutilgan ildizni o'rnatadi;
2. upload/chunk shifrlangan parchalarni tartibli tartibda qo‘shadi;
3. to'plam ildizi va manifestini yuklash/yakuniy muhrlash;
4. kompilyatsiya bilan bog'langan deterministik shaxsiy kompilyatsiya profilini hosil qiladi
   qabul qilingan to'plam;
5. model/artefakt + model/vazn registridagi yozuvlar yuklangan paketga havola qiladi
   yolg'iz o'qitish ishi emas;
6. ruxsat-model yuklangan modelni allaqachon tan olgan kvartiraga bog'laydi
   `allow_model_inference`;
7. run-private sessiyani yozib oladi va nazorat punktlari/kvitansiyalarini chiqaradi;
8. shifrini ochish-chiqish boshqariladigan chiqish materialini chiqaradi.

## Narxlar va boshqaruv tekisligi siyosati

Joriy Soracloud zaryadlash/boshqaruv tekisligi harakatini kengaytiring:- Yuklangan modellar bilan ishlaydigan kvartiralar uchun `allow_model_inference` talab qilinadi;
- XOR-da narxlarni saqlash, kompilyatsiya qilish, ish vaqti bosqichlari va shifrni ochish;
- ushbu bosqichda yuklangan modellar uchun hikoyani tarqatishni o'chirib qo'ying;
- Yuklangan modellarni So Raning yopiq arenasida va eksportga ochiq oqimlarda saqlang.

## Avtorizatsiya va majburiy semantika

Yuklash, kompilyatsiya qilish va ishga tushirish alohida imkoniyatlardir va ular alohida qolishi kerak
samolyot modeli.

- namunaviy to'plamni yuklash kvartirani boshqarish uchun bilvosita ruxsat bermasligi kerak;
- kompilyatsiya muvaffaqiyati model versiyasini joriy qilish uchun bilvosita targ'ib qilmasligi kerak;
- kvartirani bog'lash `allow-model` uslubidagi mutatsiya orqali aniq bo'lishi kerak
  qayd etadi:
  - kvartira,
  - model identifikatori,
  - vaznli versiya,
  - to'plam ildizi,
  - maxfiylik rejimi,
  - imzolovchi / audit ketma-ketligi;
- yuklangan modellarga bog'langan kvartiralar allaqachon tan olinishi kerak
  `allow_model_inference`;
- mutatsiya yo'llari bir xil Soracloud imzolangan so'rovni talab qilishda davom etishi kerak
  mavjud model/artifakt/o'quv yo'nalishlari tomonidan qo'llaniladigan intizom va bo'lishi kerak
  `CanManageSoracloud` yoki shunga o'xshash aniq vakolat berilgan organ tomonidan qo'riqlanadi
  model.

Bu "Men uni yukladim, shuning uchun har bir xususiy kvartira uni boshqarishi mumkin" ni oldini oladi.
drift va kvartirani ijro etish siyosatini aniq saqlaydi.

## Holat va audit modeli

Yangi yozuvlar faqat mutatsiyaga emas, balki vakolatli o'qish va tekshirish sirtlariga muhtoj
marshrutlar.

Tavsiya etilgan qo'shimchalar:- yuklash holati
  - `service_name + model_name + weight_version` yoki tomonidan so'rov
    `model_id + bundle_root`;
- kompilyatsiya holati
  - `model_id + bundle_root + compile_profile_hash` tomonidan so'rov;
- shaxsiy yugurish holati
  - `session_id` tomonidan so'rov, kvartira/model/versiya konteksti bilan
    javob berish;
- shifrni ochish-chiqish holati
  - `decrypt_request_id` tomonidan so'rov.

Audit emas, balki mavjud Soracloud global ketma-ketligida qolishi kerak
har bir xususiyat uchun ikkinchi hisoblagich yaratish. Birinchi darajali audit tadbirlarini qo'shing:

- yuklashni boshlash / yakunlash
- bo'lak qo'shish / muhr
- kompilyatsiya qabul qilindi / kompilyatsiya rad etildi
- kvartira modeli ruxsat berish / bekor qilish
- shaxsiy ishga tushirish / nazorat nuqtasi / tugatish / muvaffaqiyatsizlik
- chiqishni chiqarish / rad etish

Bu yuklangan modeldagi faoliyatni bir xil nufuzli takrorlashda ko'rinadigan qilib qo'yadi va
joriy xizmat, trening, model-og'irligi, model-artifakt sifatida operatsiyalar hikoyasi,
HF birgalikda-lizing, va kvartiralar audit oqimlari.

## Qabul kvotalari va davlat o'sishi chegaralari

Zanjirdagi tom ma'noda shifrlangan model baytlari faqat kirish cheklangan bo'lsa, amal qiladi
agressiv tarzda.

Amalga oshirish kamida quyidagilar uchun deterministik chegaralarni belgilashi kerak:

- yuklangan to'plam uchun maksimal ochiq matn baytlari;
- har bir paket uchun maksimal shifrlangan baytlar;
- har bir to'plam uchun maksimal bo'laklar soni;
- har bir organ/xizmat uchun maksimal bir vaqtning o'zida parvozda yuklash seanslari;
- har bir xizmat/kvartira oynasi uchun maksimal kompilyatsiya ishlari;
- har bir shaxsiy sessiya uchun maksimal ushlab turilgan nazorat punktlari soni;
- har bir sessiya uchun maksimal chiqish so'rovlari.Torii va yadro avval e'lon qilingan chegaralardan oshib ketadigan yuklamalarni rad qilishi kerak
holatning kuchayishi sodir bo'ladi. Cheklovlar konfiguratsiyaga asoslangan bo'lishi kerak
muvofiq, ammo tasdiqlash natijalari tengdoshlar orasida deterministik bo'lib qolishi kerak.

## Takrorlash va kompilyator determinizmi

Shaxsiy kompilyator/ish vaqti yo'li odatdagidan ko'ra ko'proq determinizm yukiga ega
xizmatni joylashtirish.

Kerakli invariantlar:

- oilani aniqlash va normallashtirish barqaror kanonik to'plamni yaratishi kerak
  har qanday kompilyatsiya xeshi chiqarilishidan oldin;
- kompilyatsiya profili xeshlari ulanishi kerak:
  - normallashtirilgan to'plam ildizi,
  - oila,
  - kvantlash retsepti,
  - opset versiyasi,
  - FHE parametrlari to'plami,
  - ijro siyosati;
- ish vaqti deterministik bo'lmagan yadrolardan, suzuvchi nuqta driftidan va
  chiqishlar yoki tushumlarni o'zgartirishi mumkin bo'lgan apparat-maxsus qisqartirishlar
  tengdoshlar.

Qabul qilingan oila to'plamini kengaytirishdan oldin, kichik deterministik moslamani qo'ying
har bir oila sinfi va qulf kompilyatsiya natijalari va oltin bilan ish vaqti tushumlari
testlar.

## Kod oldidan qolgan dizayn bo'shliqlari

Eng katta hal etilmagan amalga oshirish masalalari endi aniq bo'ldi
orqa tomon qarorlari:- aniq yuklash/bo'lak/so'rov DTO shakllari va Norito sxemalari;
- to'plam/bo'lak/sessiya/tekshiruv nuqtasini qidirish uchun dunyo-davlat indekslash kalitlari;
- `iroha_config` da kvota/standart konfiguratsiyani joylashtirish;
- model/artifakt holati versiyaga yo'naltirilgan bo'lishi kerakmi
  `UserUpload` mavjud bo'lganda o'qitishga yo'naltirilgan;
- kvartirani yo'qotganda aniq bekor qilish harakati
  `allow_model_inference` yoki qadalgan model versiyasi orqaga qaytarildi.

Bular dizayndan kodlash uchun keyingi ko'prik elementlari. ning me'moriy joylashuvi
xususiyat endi barqaror bo'lishi kerak.

## Test matritsasi- yuklash tekshiruvi:
  - kanonik HF seyftensorlari repolarini qabul qiling
  - GGUF, ONNX, etishmayotgan tokenizer/protsessor aktivlarini rad etish, qo‘llab-quvvatlanmaydi
    arxitekturalar va audio/video multimodal paketlar
- bo'laklash:
  - deterministik to'plam ildizlari
  - barqaror bo'laklarni buyurtma qilish
  - aniq qayta qurish
  - konvertning shiftini qo'llash
- ro'yxatga olish kitobining izchilligi:
  - to'plam / bo'lak / artefakt / og'irlikni ko'tarish to'g'riligi
- kompilyator:
  - faqat dekoder, LLaVA uslubi va Qwen2-VL uslubi uchun har bir kichik moslama
  - qo'llab-quvvatlanmaydigan operatsiyalar va shakllar uchun rad etish
- shaxsiy ish vaqti:
  - barqaror tushumlar bilan shifrlangan mayda-uchun tutun sinovi va
    chiqish chegarasi
- narxlash:
  - XOR yuklash, kompilyatsiya qilish, ish vaqti bosqichlari va shifrni ochish uchun haq oladi
- Shunday qilib, Ra integratsiyasi:
  - yuklash, kompilyatsiya qilish, nashr qilish, jamoaga ulash, yopiq arenani ishga tushirish, kvitansiyalarni tekshirish,
    loyihani saqlash, qayta ochish, deterministik tarzda qayta ishga tushirish
- xavfsizlik:
  - eksport darvozasini aylanib o'tish yo'q
  - hikoyaning avtomatik tarqalishi yo'q
  - kvartirani ulash `allow_model_inference` holda bajarilmaydi

## Amalga oshirish bo'laklari1. Ma'lumotlar modelining etishmayotgan maydonlarini va yangi yozuv turlarini qo'shing.
2. Yangi Torii so'rov/javob turlari va marshrut ishlovchilarni qo'shing.
3. Mos Soracloud ISI va dunyo miqyosidagi xotirani qo'shing.
4. Deterministik to'plam/bo'lak tekshiruvi va zanjirdagi shifrlangan baytni qo'shing
   saqlash.
5. Kichkina BFV tomonidan qo'llab-quvvatlanadigan shaxsiy transformator armatura/ish vaqti yo'lini qo'shing.
6. Yuklash/kompilyatsiya/xususiy ishga tushirish oqimlarini qoplash uchun CLI modeli buyruqlarini kengaytiring.
7. Backend yo'li vakolatli bo'lganidan keyin Land So Ra integratsiyasi.