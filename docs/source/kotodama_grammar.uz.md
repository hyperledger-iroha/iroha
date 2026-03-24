---
lang: uz
direction: ltr
source: docs/source/kotodama_grammar.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ac9b1fa221c6de46c139ee3a3c280957adad4910b49015fbb746259a4af22659
source_last_modified: "2026-01-30T12:29:10.190473+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama Til grammatikasi va semantikasi

Ushbu hujjat Kotodama til sintaksisini (lekslash, grammatika), yozish qoidalarini, deterministik semantikani va Norito ko'rsatkich-ABI konventsiyalari bilan IVM bayt kodiga (.to) pastroq bo'lgan dasturlarni ko'rsatadi. Kotodama manbalari .ko kengaytmasidan foydalanadi. Kompilyator IVM bayt kodini (.to) chiqaradi va ixtiyoriy ravishda manifestni qaytarishi mumkin.

Tarkib
- Umumiy ko'rinish va maqsadlar
- Leksik tarkibi
- Turlar va harflar
- Deklaratsiyalar va modullar
- Shartnoma konteyneri va metama'lumotlar
- Funksiyalar va parametrlar
- Bayonotlar
- Ifodalar
- Builtins va Pointer-ABI konstruktorlari
- To'plamlar va xaritalar
- Deterministik iteratsiya va chegaralar
- Xatolar va diagnostika
- IVM ga Codegen Mapping
- ABI, Sarlavha va Manifest
- Yo'l xaritasi

## Umumiy ko'rinish va maqsadlar

- Deterministik: Dasturlar apparat bo'ylab bir xil natijalarni berishi kerak; suzuvchi nuqta yoki deterministik bo'lmagan manbalar yo'q. Barcha xost o'zaro ta'sirlari Norito kodlangan argumentlar bilan tizimli qo'ng'iroqlar orqali amalga oshiriladi.
- Portativ: Jismoniy ISA emas, Iroha virtual mashinasi (IVM) bayt-kodini maqsad qiladi. Omborda ko'rinadigan RISC‑V-ga o'xshash kodlashlar IVM dekodlashning amalga oshirish tafsilotlari bo'lib, kuzatilishi mumkin bo'lgan xatti-harakatlarni o'zgartirmasligi kerak.
- Auditable: Kichik, aniq semantika; sintaksisni IVM opkodlariga va tizim chaqiruvlariga aniq xaritalash.
- Cheklanganlik: Cheklanmagan ma'lumotlar ustidagi tsikllar aniq chegaralarga ega bo'lishi kerak. Xarita iteratsiyasi determinizmni kafolatlaydigan qat'iy qoidalarga ega.

## Leksik tuzilmasi

Bo'shliq va sharhlar
- Bo'shliq tokenlarni ajratib turadi va boshqa hollarda ahamiyatsiz.
- Satr sharhlari `//` bilan boshlanadi va satr oxirigacha ishlaydi.
- `/* ... */` sharhlarini bloklash uyaga joylashmaydi.

Identifikatorlar
- Boshlash: `[A-Za-z_]`, keyin `[A-Za-z0-9_]*` davom eting.
- Harflar katta-kichikligiga sezgir; `_` haqiqiy identifikator, lekin tavsiya etilmaydi.

Kalit so'zlar (zahiralangan)
- `seiyaku`, `hajimari`, `kotoage`, `kaizen`, `state`, `struct`, `struct`, Kotodama, Kotodama `const`, `return`, `if`, `else`, `while`, `for`, Kotodama, Kotodama `continue`, `true`, `false`, `permission`, `kotoba`.

Operatorlar va tinish belgilari
- Arifmetik: `+ - * / %`
- Bit bo'yicha: `& | ^ ~`, siljishlar `<< >>`
- Taqqoslang: `== != < <= > >=`
- Mantiqiy: `&& || !`
- Belgilang: `= += -= *= /= %= &= |= ^= <<= >>=`
- Boshqa: `: , ; . :: ->`
- Qavslar: `() [] {}`Harflar
- Butun son: kasrli (`123`), olti burchakli (`0x2A`), ikkilik (`0b1010`). Barcha butun sonlar ish vaqtida 64 bitli imzolanadi; qo'shimchasiz harflar xulosa orqali yoki sukut bo'yicha `int` sifatida kiritiladi.
- Satr: ikki tirnoqli qoʻshtirnoqli (`\n`, `\r`, `\t`, `\0`, `\xNN`, `\xNN`, Kotodama, Kotodama, `\r` `\\`); UTF‑8. `r"..."` yoki `r#"..."#` xom satrlari qochishlarni o'chiradi va yangi qatorlarga ruxsat beradi.
- Baytlar: qochish bilan `b"..."` yoki xom `br"..."` / `rb"..."`; `bytes` harfini beradi.
- Mantiqiy: `true`, `false`.

## Turlar va harflar

Skalyar turlari
- `int`: 64 bitli ikkita to'ldiruvchi; add/sub/mul uchun arifmetik o'rash moduli 2^64; bo'linma IVM da imzolangan/imzosiz variantlarni aniqladi; kompilyator semantika uchun mos variantni tanlaydi.
- `fixed_u128`, `Amount`, `Balance`: raqamli taxalluslar Norito `Numeric` tomonidan qo'llab-quvvatlanadi (512 bitli mantis va masshtabgacha imzolangan o'nlik). Kotodama bu taxalluslarni manfiy bo'lmagan miqdorlar sifatida ko'radi; arifmetika tekshiriladi, taxallusni saqlaydi va ortiqcha yoki nolga bo'linishda tuzoqqa tushadi. `int` dan yaratilgan qiymatlar 0 shkalasidan foydalanadi; `int` ga/dan konvertatsiyalar ish vaqtida diapazonda tekshiriladi (salbiy emas, integral, i64-ga mos keladi).
- `bool`: mantiqiy haqiqat qiymati; `0`/`1` gacha tushirildi.
- `string`: o'zgarmas UTF‑8 qatori; tizim chaqiruvlariga o'tkazilganda Norito TLV sifatida ifodalanadi; VM ichidagi operatsiyalar bayt bo'laklari va uzunligidan foydalanadi.
- `bytes`: xom Norito foydali yuk; Xesh/kripto/proof kirishlar va bardoshli qoplamalar uchun ABI `Blob` tipidagi ko'rsatkichga taxallus qo'yadi.

Kompozit turlari
- `struct Name { field: Type, ... }` foydalanuvchi tomonidan belgilangan mahsulot turlari. Konstruktorlar ifodalarda `Name(a, b, ...)` chaqiruv sintaksisidan foydalanadilar. Maydonga kirish `obj.field` qo'llab-quvvatlanadi va ichki ravishda kortej uslubidagi pozitsion maydonlarga tushiriladi. Chidamli holat ABI zanjirida Norito kodlangan; kompilyator tuzilish tartibini aks ettiruvchi qoplamalarni chiqaradi va oxirgi testlar (`crates/iroha_core/tests/kotodama_struct_overlay.rs`) tartibni relizlar bo'ylab qulflangan holda saqlaydi.
- `Map<K, V>`: deterministik assotsiativ xarita; semantika iteratsiya paytida iteratsiya va mutatsiyalarni cheklaydi (pastga qarang).
- `Tuple (T1, T2, ...)`: pozitsion maydonlar bilan anonim mahsulot turi; ko'p qaytish uchun ishlatiladi.

Maxsus ko'rsatgich-ABI turlari (xostga qaragan)
- `AccountId`, `AssetDefinitionId`, `Name`, `Json`, `NftId`, `Blob` va shunga o'xshashlar birinchi darajali ish vaqti turlari emas. Ular INPUT mintaqasiga (Norito TLV konvertlari) terilgan, o'zgarmas ko'rsatgichlarni beradigan konstruktorlar bo'lib, ular faqat tizim chaqiruvi argumentlari sifatida ishlatilishi yoki mutatsiyasiz o'zgaruvchilar o'rtasida ko'chirilishi mumkin.

Xulosa yozing
- Mahalliy `let` ulanishlari boshlang'ichdan turni aniqlaydi. Funktsiya parametrlari aniq yozilishi kerak. Qaytish turlari, agar noaniq bo'lmasa, xulosa chiqarish mumkin.

## Deklaratsiyalar va modullarYuqori darajadagi elementlar
- Shartnomalar: `seiyaku Name { ... }` funksiyalar, holat, tuzilmalar va metamaʼlumotlarni oʻz ichiga oladi.
- Bitta faylga bir nechta shartnomalar tuzishga ruxsat beriladi, lekin tavsiya etilmaydi; manifestda standart yozuv sifatida bitta asosiy `seiyaku` ishlatiladi.
- `struct` deklaratsiyasi shartnoma doirasidagi foydalanuvchi turlarini belgilaydi.

Ko'rinish
- `kotoage fn` umumiy kirish nuqtasini bildiradi; ko'rinish kodek emas, balki dispetcher ruxsatlariga ta'sir qiladi.
- Qo'shimcha ruxsat bo'yicha maslahatlar: `#[access(read=..., write=...)]` manifestni o'qish/yozish kalitlarini taqdim etish uchun `fn`/`kotoage fn` dan oldin bo'lishi mumkin. Kompilyator avtomatik ravishda maslahat ko'rsatmalarini ham chiqaradi; noaniq xost qo'ng'iroqlari konservativ joker kalitlarga (`*`) qaytadi va agar ochiq kirish bo'yicha maslahatlar berilmasa, diagnostikani yuzaga keltiradi, shuning uchun rejalashtiruvchilar nozikroq kalitlar uchun dinamik oldindan o'tishni tanlashlari mumkin.

## Shartnoma konteyneri va metamaʼlumotlar

Sintaksis
```
seiyaku Name {
  meta {
    abi_version: 1,
    vector_length: 0,
    max_cycles: 0,
    features: ["zk", "simd"],
  }

  state int counter;

  hajimari() { counter = 0; }

  kotoage fn inc() { counter = counter + 1; }
}
```

Semantika
- `meta { ... }` maydonlari chiqarilgan IVM sarlavhasi uchun kompilyator sukutini bekor qiladi: `abi_version`, `vector_length` (0 o'rnatilmagan degan ma'noni anglatadi), `max_cycles` (`max_cycles`) (`max_cycles`), Kotodama uchun kompilyator standarti030 xususiyat bitlari (ZK tracing, vektor e'lon qilish). Kompilyator `max_cycles: 0` ni “standart foydalanish” deb hisoblaydi va qabul talablarini qondirish uchun sozlangan noldan farqli standartni chiqaradi. Qo'llab-quvvatlanmaydigan xususiyatlar ogohlantirish bilan e'tiborga olinmaydi. `meta {}` o'tkazib yuborilsa, kompilyator `abi_version = 1` ni chiqaradi va qolgan sarlavha maydonlari uchun standart parametrlardan foydalanadi.
- `features: ["zk", "simd"]` (taxalluslar: `"vector"`) mos keladigan sarlavha bitlarini aniq so'raydi. Noma'lum xususiyatlar qatorlari endi e'tiborga olinmaslik o'rniga tahlil qilish xatosini keltirib chiqaradi.
- `state` uzoq muddatli shartnoma o'zgaruvchilarini e'lon qiladi. Kompilyator `STATE_GET/STATE_SET/STATE_DEL` tizimli qoʻngʻiroqlariga kirishni pasaytiradi va xost ularni har bir tranzaksiyaning qoplamasida bosqichma-bosqich (nazorat punkti/qayta tiklash, WSV-ga oʻchirishda) bosqichma-bosqich qoʻyadi. Kirish bo'yicha maslahatlar literal holat yo'llari uchun chiqariladi; dinamik kalitlar xarita darajasidagi ziddiyatli kalitlarga qaytadi. Xost tomonidan qo'llab-quvvatlanadigan aniq o'qish/yozish uchun `state_get/state_set/state_del` va `get_or_insert_default` xarita yordamchilaridan foydalaning; bu marshrutni Norito TLVs orqali o'tkazing va nomlar/maydon tartibini barqaror saqlang.
- davlat identifikatorlari zahiraga qo'yilgan; parametrlarda `state` nomini soya qilish yoki `let` bog'lash rad etiladi (`E_STATE_SHADOWED`).
- Davlat xaritasi qiymatlari birinchi darajali emas: xarita operatsiyalari va iteratsiya uchun davlat identifikatoridan bevosita foydalaning. Davlat xaritalarini foydalanuvchi tomonidan belgilangan funksiyalarga ulash yoki uzatish rad etiladi (`E_STATE_MAP_ALIAS`).
- Bardoshli holat xaritalari hozirda faqat `int` va pointer-ABI kalit turlarini qo'llab-quvvatlaydi; boshqa kalit turlari kompilyatsiya vaqtida rad etiladi.
- Bardoshli holat maydonlari `int`, `bool`, `Json`, `Blob`/`bytes` yoki koʻrsatkich-ABI turlari boʻlishi kerak (jumladan, ushbu tuzilmalar/maydonlar komplektlari); `string` bardoshli holat uchun qo'llab-quvvatlanmaydi.

### Kotoba mahalliylashtirish
Sintaksis
```
kotoba {
  "E_UNBOUNDED_ITERATION": { en: "Loop over map lacks a bound." }
}
```Semantika
- `kotoba` yozuvlari shartnoma manifestiga tarjima jadvallarini biriktiradi (`kotoba` maydoni).
- Xabar identifikatorlari va til teglari identifikatorlar yoki satr harflarini qabul qiladi; yozuvlar bo'sh bo'lmasligi kerak.
- Ikki nusxadagi `msg_id` + til teg juftlari kompilyatsiya vaqtida rad etiladi.

## Trigger deklaratsiyasi

Trigger deklaratsiyasi rejalashtirish metamaʼlumotlarini kirish nuqtasi manifestlariga biriktiradi va avtomatik roʻyxatga olinadi
shartnoma nusxasi faollashtirilganda (o'chirishda o'chiriladi). Ular a ichida tahlil qilinadi
`seiyaku` bloki.

Sintaksis
```
register_trigger wake {
  call run;
  on time pre_commit;
  repeats 2;
  authority "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";
  metadata { tag: "alpha"; count: 1; enabled: true; }
}
```

Eslatmalar
- `call` xuddi shu shartnomada ochiq `kotoage fn` kirish nuqtasiga havola qilishi kerak; ixtiyoriy
  `namespace::entrypoint` manifestda qayd etilgan, ammo o'zaro shartnomalar bo'yicha qo'ng'iroqlar rad etilgan
  ish vaqti bo'yicha (faqat mahalliy qayta qo'ng'iroqlar).
- Qo'llab-quvvatlanadigan filtrlar: `time pre_commit` va `time schedule(start_ms, period_ms?)`, shuningdek,
  `execute trigger <name>` qo'ng'iroqlar uchun triggerlar, `data any` va quvur liniyasi filtrlari
  (`pipeline transaction`, `pipeline block`, `pipeline merge`, `pipeline witness`).
- `authority` ixtiyoriy ravishda trigger vakolatini bekor qiladi (AccountId string literal). Agar o'tkazib yuborilsa,
  ish vaqti kontraktni faollashtirish vakolatidan foydalanadi.
- Metadata qiymatlari JSON harflari (`string`, `number`, `bool`, `null`) yoki `json!(...)` boʻlishi kerak.
- Runtime injected trigger metadata kalitlari: `contract_namespace`, `contract_id`,
  `contract_entrypoint`, `contract_code_hash`, `contract_trigger_id`.

## Funksiyalar va parametrlar

Sintaksis
- Deklaratsiya: `fn name(param1: Type, param2: Type, ...) -> Ret { ... }`
- Umumiy: `kotoage fn name(...) { ... }`
- Initializer: `hajimari() { ... }` (VMning o'zi tomonidan emas, balki ish vaqti tomonidan tarqatilganda chaqiriladi).
- Yangilash kancasi: `kaizen(args...) permission(Role) { ... }`.

Parametrlar va qaytishlar
- Argumentlar `r10..r22` registrlarida qiymatlar yoki ABI uchun INPUT ko'rsatkichlari (Norito TLV) sifatida uzatiladi; qo'shimcha args to'kiladi stack.
- Funksiyalar nol yoki bitta skalyar yoki kortejni qaytaradi. Birlamchi qaytish qiymati skaler uchun `r10` da; kortejlar konventsiya bo'yicha stek/OUTPUT da materiallashtiriladi.

## Bayonotlar- O'zgaruvchan bog'lanishlar: `let x = expr;`, `let mut x = expr;` (o'zgaruvchanlik kompilyatsiya vaqtini tekshirish; ish vaqti mutatsiyasiga faqat mahalliy aholi uchun ruxsat beriladi).
- Topshiriq: `x = expr;` va aralash shakllar `x += 1;` va boshqalar. Maqsadlar o'zgaruvchilar yoki xarita indekslari bo'lishi kerak; Tuple/struct maydonlari o'zgarmasdir.
- Raqamli taxalluslar (`fixed_u128`, `Amount`, `Balance`) `Numeric` tomonidan qo'llab-quvvatlanadigan alohida turlar; arifmetik taxallusni saqlaydi va taxalluslarni aralashtirish `int` bog'lash orqali konvertatsiya qilishni talab qiladi. `int` ga/dan konvertatsiyalar ish vaqtida tekshiriladi (salbiy bo'lmagan, integral, diapazon cheklangan).
- Nazorat: `if (cond) { ... } else { ... }`, `while (cond) { ... }`, C-uslubi `for (init; cond; step) { ... }`.
  - `for` initsializatorlari va qadamlari oddiy `let name = expr` yoki ifoda bayonotlari bo'lishi kerak; murakkab tuzilmani buzish rad etilgan (`E0005`, `E0006`).
  - `for` qamrovi: init bandidagi bog'lanishlar tsiklda va undan keyin ko'rinadi; tanada yoki qadamda yaratilgan bog'lamlar pastadirdan qochib qutula olmaydi.
- Tenglik (`==`, `!=`) `int`, `bool`, `string`, koʻrsatkich-ABI skayarlari (masalan, I10210X, I18210X) uchun qoʻllab-quvvatlanadi. `Name`, `Blob`/`bytes`, `Json`); kortejlar, tuzilmalar va xaritalarni solishtirish mumkin emas.
- Xarita halqasi: `for (k, v) in map { ... }` (deterministik; pastga qarang).
- Oqim: `return expr;`, `break;`, `continue;`.
- Qo'ng'iroq: `name(args...);` yoki `call name(args...);` (ikkalasi ham qabul qilinadi; kompilyator qo'ng'iroq bayonotlarini normallashtiradi).
- Tasdiqlar: `assert(cond);`, `assert_eq(a, b);` ZK bo'lmagan tuzilmalarda IVM `ASSERT*` xaritasi yoki ZK rejimida ZK cheklovlari.

## Ifodalar

Ustuvorlik (yuqori → past)
1. Aʼzo/indeks: `a.b`, `a[b]`
2. Birlik: `! ~ -`
3. Multiplikativ: `* / %`
4. Qo'shimcha: `+ -`
5. Shiftlar: `<< >>`
6. Aloqaviy: `< <= > >=`
7. Tenglik: `== !=`
8. Bitli VA/XOR/OR: `& ^ |`
9. Mantiqiy VA/YOKI: `&& ||`
10. Uchlik: `cond ? a : b`

Qo'ng'iroqlar va kortejlar
- Qo'ng'iroqlar pozitsion argumentlardan foydalanadi: `f(a, b, c)`.
- Tuple literal: `(a, b, c)` va buzilish: `let (x, y) = pair;`.
- Tuple tuzilmasini buzish mos keladigan aritmaga ega kortej/struktura turlarini talab qiladi; nomuvofiqliklar rad etiladi.

Satrlar va baytlar
- satrlar UTF‑8; manbada raw string va bayt literal shakllari qabul qilinadi.
- bayt literallari (`b"..."`, `br"..."`, `rb"..."`) `bytes` (Blob) ko'rsatkichlaridan past; tizim qoʻngʻirogʻi NoritoBytes TLV yuklarini kutayotganda `norito_bytes(...)` bilan oʻrash.

## Builtins va Pointer-ABI konstruktorlari

Pointer konstruktorlari (INPUT ga Norito TLV chiqaring va yozilgan ko'rsatgichni qaytaring)
- `account_id(string) -> AccountId*`
- `asset_definition(string) -> AssetDefinitionId*`
- `asset_id(string) -> AssetId*`
- `domain(string) | domain_id(string) -> DomainId*`
- `name(string) -> Name*`
- `json(string) -> Json*`
- `nft_id(string) -> NftId*`
- `blob(bytes|string) -> Blob*`
- `norito_bytes(bytes|string) -> NoritoBytes*`
- `dataspace_id(string|0xhex) -> DataSpaceId*`
- `axt_descriptor(string|0xhex) -> AxtDescriptor*`
- `asset_handle(string|0xhex) -> AssetHandle*`
- `proof_blob(string|0xhex) -> ProofBlob*`Prelude makroslari ushbu konstruktorlar uchun qisqaroq taxalluslar va inline tekshiruvini ta'minlaydi:
- `account!("i105...")`, `account_id!("i105...")`
- `asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")`, `asset_id!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")`
- `domain!("wonderland")`, `domain_id!("wonderland")`
- `name!("example")`
- `json!("{\"hello\":\"world\"}")` yoki tuzilgan harflar, masalan, `json!{ hello: "world" }`
- `nft_id!("dragon$demo")`, `blob!("bytes")`, `norito_bytes!("...")`

Makroslar yuqoridagi konstruktorlargacha kengaytiriladi va kompilyatsiya vaqtida yaroqsiz harflarni rad etadi.

Amalga oshirish holati
- Amalga oshirildi: yuqoridagi konstruktorlar string literal argumentlarni qabul qiladi va INPUT mintaqasida joylashtirilgan Norito dan past bo'lgan TLV konvertlarini qabul qiladi. Ular tizim chaqiruvi argumentlari sifatida foydalanish mumkin bo'lgan o'zgarmas yozilgan ko'rsatkichlarni qaytaradi. Literal bo'lmagan satr ifodalari rad etiladi; dinamik kirishlar uchun `Blob`/`bytes` dan foydalaning. `blob`/`norito_bytes`, shuningdek, `bytes`-yozilgan qiymatlarni ish vaqtida so'l shimlarsiz qabul qiladi.
- kengaytirilgan shakllar:
  - `json(Blob[NoritoBytes]) -> Json*` `JSON_DECODE` tizimi orqali.
  - `name(Blob[NoritoBytes]) -> Name*` `NAME_DECODE` tizimi orqali.
  - Blob/NoritoBytes-dan ko'rsatgich dekodlash: har qanday ko'rsatgich konstruktori (AXT turlarini o'z ichiga olgan holda) `Blob`/`NoritoBytes` foydali yukini qabul qiladi va kutilgan tur identifikatori bilan `POINTER_FROM_NORITO` ga tushiradi.
  - Pointer shakllari uchun o'tish: `name(Name) -> Name*`, `blob(Blob) -> Blob*`, `norito_bytes(Blob) -> Blob*`.
  - Shakar usuli qo'llab-quvvatlanadi: `s.name()`, `s.json()`, `b.blob()`, `b.norito_bytes()`.

Xost/syscall o'rnatilgan qurilmalar (SCALL xaritasi; ivm.md da aniq raqamlar)
- `mint_asset(AccountId*, AssetDefinitionId*, numeric)`
- `burn_asset(AccountId*, AssetDefinitionId*, numeric)`
- `transfer_asset(AccountId*, AccountId*, AssetDefinitionId*, numeric)`
- `set_account_detail(AccountId*, Name*, Json*)`
- `execute_instruction(Blob[NoritoBytes])`
- `execute_query(Blob[NoritoBytes]) -> Blob`
- `subscription_bill()`
- `subscription_record_usage()`
- `nft_mint_asset(NftId*, AccountId*)`
- `nft_transfer_asset(AccountId*, NftId*, AccountId*)`
- `nft_set_metadata(NftId*, Json*)`
- `nft_burn_asset(NftId*)`
- `authority() -> AccountId*`
- `register_domain(DomainId*)`
- `unregister_domain(DomainId*)`
- `transfer_domain(AccountId*, DomainId*, AccountId*)`
- `vrf_verify(Blob, Blob, Blob, int variant) -> Blob`
- `vrf_verify_batch(Blob) -> Blob`
- `axt_begin(AxtDescriptor*)`
- `axt_touch(DataSpaceId*, Blob[NoritoBytes]? manifest)`
- `verify_ds_proof(DataSpaceId*, ProofBlob?)`
- `use_asset_handle(AssetHandle*, Blob[NoritoBytes], ProofBlob?)`
- `axt_commit()`
- `contains(Map<K,V>, K) -> bool`

Yordamchi dasturlar
- `info(string|int)`: OUTPUT orqali tuzilgan hodisa/xabarni chiqaradi.
- `hash(blob) -> Blob*`: Blob sifatida Norito kodlangan xeshni qaytaradi.
- `build_submit_ballot_inline(election_id, ciphertext, nullifier32, backend, proof, vk) -> Blob*` va `build_unshield_inline(asset, to, amount, inputs32, backend, proof, vk) -> Blob*`: inline ISI quruvchilari; barcha argumentlar kompilyatsiya vaqti literallari bo'lishi kerak (string literallari yoki literallardan ko'rsatgich konstruktorlari). `nullifier32` va `inputs32` aniq 32 bayt (xom qator yoki `0x` hex) va `amount` manfiy bo'lmasligi kerak.
- `schema_info(Name*) -> Json* { "id": "<hex>", "version": N }`
- `encode_schema(Name*, Json*) -> Blob`: xost sxemasi registridan foydalangan holda JSON-ni kodlaydi (DefaultRegistry Buyurtma/savdo namunalariga qo'shimcha ravishda `QueryRequest` va `QueryResponse`-ni qo'llab-quvvatlaydi).
- `decode_schema(Name*, Blob|bytes) -> Json*`: xost sxemasi registridan foydalanib, Norito baytlarini dekodlaydi.
- `pointer_to_norito(ptr) -> NoritoBytes*`: saqlash yoki tashish uchun mavjud ABI TLV ko'rsatkichini NoritoBytes sifatida o'rab oladi.
- `isqrt(int) -> int`: butun kvadrat ildiz (`floor(sqrt(x))`) IVM operatsiya kodi sifatida amalga oshiriladi.
- `min(int, int) -> int`, `max(int, int) -> int`, `abs(int) -> int`, `div_ceil(int, int) -> int`, `gcd(int, int) -> int`, `mean(int, int) -> int` — birlashtirilgan arifmetik yordamchilar I1NT030des tomonidan qo'llab-quvvatlandi (nolga bo'linishda shiftni bo'linish tuzoqlari).Eslatmalar
- O'rnatilgan qismlar yupqa shimlardir; kompilyator harakatlarni ro'yxatdan o'tkazish uchun ularni tushiradi va `SCALL`.
- Pointer konstruktorlari toza: VM INPUTdagi Norito TLV qo'ng'iroq davomiyligi uchun o'zgarmasligini ta'minlaydi.
 - Pointer-ABI maydonlari bo'lgan tuzilmalar (masalan, `DomainId`, `AccountId`) tizim chaqiruvi argumentlarini ergonomik tarzda guruhlash uchun ishlatilishi mumkin. Kompilyator `obj.field` ni qo'shimcha ajratmalarsiz to'g'ri registrga/qiymatga moslashtiradi.

## To'plamlar va Xaritalar

Turi: `Map<K, V>`
- Xotira ichidagi xaritalar (`Map::new()` orqali yig'ilgan yoki parametr sifatida uzatilgan) bitta kalit/qiymat juftligini saqlaydi; kalitlar va qiymatlar so'z o'lchamidagi turdagi bo'lishi kerak: `int`, `bool`, `string`, `Blob`, `bytes`, `Json`, yoki `AccountId`, `Name`).
- Bardoshli holat xaritalari (`state Map<...>`) Norito kodlangan kalit/qiymatlardan foydalanadi. Qo'llab-quvvatlanadigan kalitlar: `int` yoki ko'rsatgich turlari. Qo'llab-quvvatlanadigan qiymatlar: `int`, `bool`, `Json`, `Blob`/`bytes` yoki ko'rsatkich turlari.
- `Map::new()` xotiradagi yagona yozuvni ajratadi va nol bilan ishga tushiradi (kalit/qiymat = 0); `Map<int,int>` bo'lmagan xaritalar uchun aniq turdagi izoh yoki qaytish turini taqdim eting.
- Davlat xaritalari birinchi darajali qiymatlar emas: ularni qayta tayinlay olmaysiz (masalan, `M = Map::new()`); indekslash orqali yozuvlarni yangilash (`M[key] = value`).
- Operatsiyalar:
  - Indekslash: `map[key]` qiymatini olish/o'rnatish (xost tizimi orqali amalga oshirilgan o'rnatish; ish vaqti API xaritasini ko'ring).
  - Mavjudligi: `contains(map, key) -> bool` (pastga tushirilgan yordamchi; ichki tizim chaqiruvi bo'lishi mumkin).
  - Iteratsiya: deterministik tartib va ​​mutatsiya qoidalari bilan `for (k, v) in map { ... }`.

Deterministik iteratsiya qoidalari
- Takrorlash to'plami - tsiklga kirishda tugmachalarning oniy tasviri.
- Buyurtma Norito kodli kalitlarning qat'iy ravishda o'sib boruvchi bayt-leksikografik tartibidir.
- Loop davomida takrorlangan xaritaga tuzilmaviy o'zgartirishlar (qo'shish/o'chirish/tozalash) deterministik `E_ITER_MUTATION` tuzoqqa sabab bo'ladi.
- Cheklanganlik talab qilinadi: xaritada e'lon qilingan maksimal (`@max_len`), aniq atribut `#[bounded(n)]` yoki `.take(n)`/`.range(..)` yordamida aniq chegara; aks holda kompilyator `E_UNBOUNDED_ITERATION` chiqaradi.

Chegara yordamchilari
- `#[bounded(n)]`: xarita ifodasidagi ixtiyoriy atribut, masalan. `for (k, v) in my_map #[bounded(2)] { ... }`.
- `.take(n)`: boshidan birinchi `n` yozuvlarini takrorlang.
- `.range(start, end)`: `[start, end)` yarim ochiq oraliqda yozuvlarni takrorlash. Semantika `start` va `n = end - start` ga teng.Dinamik chegaralar bo'yicha eslatmalar
- Literal chegaralar: `n`, `start` va `end`, chunki butun sonli harflar to'liq qo'llab-quvvatlanadi va ma'lum miqdordagi iteratsiyaga kompilyatsiya qilinadi.
- To'liq bo'lmagan chegaralar: `kotodama_dynamic_bounds` funksiyasi `ivm` kassasida yoqilgan bo'lsa, kompilyator dinamik `n`, `start` va `end` ish vaqti, qo'shimchalar va xavfsizlik ifodalari sifatida qabul qiladi. `end >= start`). Qo'shimcha tana ijrosini oldini olish uchun `if (i < n)` tekshiruvlari bilan K ga qadar himoyalangan iteratsiyalarni kamaytirish (standart K=2). K ni `CompilerOptions { dynamic_iter_cap, .. }` orqali dasturiy tarzda sozlashingiz mumkin.
- Kompilyatsiya qilishdan oldin Kotodama tuklar haqida ogohlantirishlarni tekshirish uchun `koto_lint` ni ishga tushiring; asosiy kompilyator har doim tahlil qilish va tipni tekshirishdan keyin pasaytirish bilan davom etadi.
- Xato kodlari [Kotodama Compiler Error Codes](./kotodama_error_codes.md) da hujjatlashtirilgan; tezkor tushuntirishlar uchun `koto_compile --explain <code>` dan foydalaning.

## Xatolar va diagnostika

Kompilyatsiya vaqti diagnostikasi (misollar)
- `E_UNBOUNDED_ITERATION`: xaritada aylanish chegarasi yo'q.
- `E_MUT_DURING_ITER`: tsikl tanasida takrorlangan xaritaning tizimli mutatsiyasi.
- `E_STATE_SHADOWED`: mahalliy ulanishlar `state` deklaratsiyasini soya qila olmaydi.
- `E_BREAK_OUTSIDE_LOOP`: `break` tsikldan tashqarida ishlatiladi.
- `E_CONTINUE_OUTSIDE_LOOP`: `continue` tsikldan tashqarida ishlatiladi.
- `E0005`: for-loop ishga tushirgich qo'llab-quvvatlanadiganidan ancha murakkab.
- `E0006`: for-loop qadam bandi qo'llab-quvvatlanadigandan ko'ra murakkabroq.
- `E_BAD_POINTER_USE`: birinchi darajali tur talab qilinadigan ko'rsatkich-ABI konstruktor natijasidan foydalanish.
- `E_UNRESOLVED_NAME`, `E_TYPE_MISMATCH`, `E_ARITY_MISMATCH`, `E_DUP_SYMBOL`.
- Asboblar: `koto_compile` bayt-kodni chiqarishdan oldin lint passni ishga tushiradi; o'tkazib yuborish uchun `--no-lint` dan foydalaning yoki lint chiqishi bo'yicha qurishni muvaffaqiyatsiz qilish uchun `--deny-lint-warnings` dan foydalaning.

Ish vaqti VM xatolari (tanlangan; toʻliq roʻyxat ivm.md da)
- `E_NORITO_INVALID`, `E_OOB`, `E_UNALIGNED`, `E_SCALL_UNKNOWN`, `E_ASSERT`, `E_ASSERT_EQ`, Norito.

Xato xabarlari
- Diagnostika mavjud bo'lganda, `kotoba {}` tarjima jadvallaridagi yozuvlarga mos keladigan barqaror `msg_id` larni olib yuradi.

## IVM ga Codegen xaritalash

Quvur liniyasi
1. Lexer/Parser AST hosil qiladi.
2. Semantik tahlil nomlarni hal qiladi, turlarini tekshiradi va belgilar jadvallarini to'ldiradi.
3. IQni oddiy SSAga o'xshash shaklga tushirish.
4. IVM GPRs (har bir chaqiruv konventsiyasi bo'yicha args/ret uchun `r10+`) uchun ajratishni ro'yxatdan o'tkazish; to'kiladi.
5. Baytkod emissiyasi: ruxsat etilgan IVM-native va RV-mos kodlashlar aralashmasi; `abi_version`, xususiyatlar, vektor uzunligi va `max_cycles` bilan chiqarilgan metamaʼlumotlar sarlavhasi.Asosiy nuqtalarni xaritalash
- IVM ALU operatsiyalariga arifmetik va mantiqiy xarita.
- shartli shoxlar va sakrashlarga tarmoqlanish va nazorat qilish xaritasi; kompilyator foydali bo'lganda siqilgan shakllardan foydalanadi.
- Mahalliy aholi uchun xotira VM stekiga to'kiladi; moslashtirish amalga oshiriladi.
- Harakatlarni ro'yxatga olish uchun pastki o'rnatilgan va 8 bitli `SCALL`.
- Pointer konstruktorlari Norito TLV larni INPUT mintaqasiga joylashtiradi va ularning manzillarini ishlab chiqaradi.
- `ASSERT`/`ASSERT_EQ` ga tasdiqlar xaritasi ZK bo'lmagan ijroda tuzoqqa tushadi va ZK tuzilmalarida cheklovlar chiqaradi.

Determinizm cheklovlari
- FP yo'q; deterministik bo'lmagan tizimlar yo'q.
- SIMD/GPU tezlashuvi bayt-kodga ko'rinmaydi va bit bilan bir xil bo'lishi kerak; kompilyator apparatga xos operatsiyalarni chiqarmaydi.

## ABI, sarlavha va manifest

IVM sarlavha maydonlari kompilyator tomonidan o'rnatilgan
- `version`: IVM bayt-kod formati versiyasi (major.minor).
- `abi_version`: tizim qo'ng'irog'i jadvali va ko'rsatgich-ABI sxemasi versiyasi.
- `feature_bits`: xususiyat bayroqlari (masalan, `ZK`, `VECTOR`).
- `vector_len`: mantiqiy vektor uzunligi (0 → o'rnatilmagan).
- `max_cycles`: kirish majburiy va ZK to'ldirish bo'yicha maslahat.

Manifest (ixtiyoriy yonbosh)
- `code_hash`, `abi_hash`, `meta {}` blokidagi metamaʼlumotlar, kompilyator versiyasi va takror ishlab chiqarishga oid maslahatlar.

## Yo'l xaritasi

- **KD-231 (aprel 2026):** takrorlash chegaralari uchun kompilyatsiya vaqti diapazoni tahlilini qo'shing, shunda tsikllar rejalashtiruvchiga cheklangan kirish to'plamlarini ko'rsatadi.
- **KD-235 (may 2026):** ko'rsatgich konstruktorlari va ABI ravshanligi uchun `string` dan farq qiluvchi birinchi toifali `bytes` skalyarni taqdim eting.
- **KD-242 (iyun 2026):** deterministik zaxiralar bilan xususiyat bayroqlari orqasida o'rnatilgan operatsion kodlar to'plamini (xesh/imzoni tekshirish) kengaytiring.
- **KD-247 (iyun 2026):** `msg_id`s xatosini barqarorlashtiring va mahalliy diagnostika uchun `kotoba {}` jadvallarida xaritalashni saqlang.
### Manifest emissiya

- Kotodama kompilyator API `ContractManifest` ni kompilyatsiya qilingan `.to` bilan birga `ivm::kotodama::compiler::Compiler::compile_source_with_manifest` orqali qaytarishi mumkin.
- Maydonlar:
  - `code_hash`: artefaktni bog'lash uchun kompilyator tomonidan hisoblangan kod baytlari xeshi (IVM sarlavhasi va harflar bundan mustasno).
  - `abi_hash`: dasturning `abi_version` uchun ruxsat etilgan tizimli qoʻngʻiroq yuzasining barqaror dayjesti (qarang: `ivm.md` va `ivm::syscalls::compute_abi_hash`).
- Ixtiyoriy `compiler_fingerprint` va `features_bitmap` asboblar zanjiri uchun ajratilgan.
- `entrypoints`: eksport qilinadigan kirish nuqtalarining tartiblangan roʻyxati (ommaviy, `hajimari`, `kaizen`), shu jumladan ularning talab qilinadigan `permission(...)` satrlari va kompilyatorning eng yaxshi kuchga ega boʻlgan oʻqish/yozish logiklari va kirish sabablari haqida kutilgan kirish jadvali.
- manifest kirish vaqtini tekshirish va registrlar uchun mo'ljallangan; hayot aylanishi uchun `docs/source/new_pipeline.md` ga qarang.