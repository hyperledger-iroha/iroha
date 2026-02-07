---
lang: uz
direction: ltr
source: docs/source/finance/repo_ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 42c328443065e102a65180421d515e4e3040a35175c348ea25fd83edab1236b4
source_last_modified: "2026-01-22T16:26:46.567961+00:00"
translation_last_reviewed: 2026-02-07
title: Repo Operations & Evidence Guide
summary: Governance, lifecycle, and audit requirements for repo/reverse-repo flows (roadmap F1).
translator: machine-google-reviewed
---

# Repo operatsiyalari va dalillar bo'yicha qo'llanma (F1 yo'l xaritasi)

Repo dasturi ikki tomonlama va uch tomonlama moliyalashtirishni deterministik bilan ochadi
Norito ko'rsatmalari, CLI/SDK yordamchilari va ISO 20022 pariteti. Bu eslatma yozib oladi
yo'l xaritasining muhim bosqichini qondirish uchun zarur bo'lgan operatsion shartnoma **F1 - repo
hayot aylanishi hujjatlari va asboblar**. Bu ish jarayoniga yo'naltirilganlikni to'ldiradi
[`repo_runbook.md`](./repo_runbook.md) ifodalash orqali:

- CLI/SDK/ish vaqti (`crates/iroha_cli/src/main.rs:3821`,
  `python/iroha_python/iroha_python_rs/src/lib.rs:2216`,
  `crates/iroha_core/src/smartcontracts/isi/repo.rs:1`);
- deterministik dalil/dalil ushlash (`integration_tests/tests/repo.rs:1`);
- uch tomonli vasiylik va garovni almashtirish harakati; va
- boshqaruv kutilmalari (dual-nazorat, audit yo'llari, rollback playbooks).

## 1. Qo'llash doirasi va qabul qilish mezonlari

Yo'l xaritasi F1 elementi to'rtta mavzu bo'yicha yopiqligicha qolmoqda; ushbu hujjat sanab o'tadi
zarur artefaktlar va ularni qondiradigan kod/testlarga havolalar:

| Talab | Dalil |
|-------------|----------|
| Repo → teskari repo → almashtirishni qamrab oluvchi deterministik hisob-kitob dalillari `integration_tests/tests/repo.rs` oxirigacha oqimlarni, takroriy identifikatorni himoya qilishni, marja kadansini tekshirishni va garovni almashtirish muvaffaqiyatli/muvaffaqiyatsiz holatlarini qamrab oladi. To'plam `cargo test --workspace` ning bir qismi sifatida ishlaydi. `crates/iroha_core/src/smartcontracts/isi/repo.rs` (`repo_deterministic_lifecycle_proof_matches_fixture`) snapshotlarni boshlash → chegara → almashtirish ramkalaridagi deterministik hayot tsikli dayjest jabduqlari auditorlar kanonik foydali yuklarni farqlashi mumkin. |
| Uch tomonli qamrov | Ish vaqti saqlovchiga tegishli oqimlarni amalga oshiradi: `RepoAgreement::custodian` + `RepoAccountRole::Custodian` hodisalar (`crates/iroha_data_model/src/repo.rs:74`, `crates/iroha_data_model/src/events/data/events.rs:742`). |
| Garovni almashtirish testlari | Teskari oyoq invariantlari garovga qo'yilmagan almashtirishlarni rad etadi (`crates/iroha_core/src/smartcontracts/isi/repo.rs:417`) va integratsiya testlari almashtirish aylanmasidan keyin (`integration_tests/tests/repo.rs:261`) daftarning to'g'ri tozalanishini tasdiqlaydi. |
| Chaqiruv chegarasi va ishtirokchilarni qo'llash | `integration_tests/tests/repo.rs::repo_margin_call_enforces_cadence_and_participant_rules` `RepoMarginCallIsi` mashqlari, kadansga moslashtirilgan rejalashtirish, muddatidan oldin qoʻngʻiroqlarni rad etish va faqat ishtirokchilarga ruxsat berishni isbotlaydi. |
| Boshqaruv tomonidan tasdiqlangan runbooks | Ushbu qo'llanma va `repo_runbook.md` audit uchun CLI/SDK protseduralari, firibgarlik/orqaga qaytarish bosqichlari va dalillarni to'plash ko'rsatmalarini taqdim etadi. |

## 2. Hayotiy aylanish sirtlari

### 2.1 CLI va Norito quruvchilari

- `iroha app repo initiate|unwind|margin|margin-call` o'rash `RepoIsi`,
  `ReverseRepoIsi` va `RepoMarginCallIsi`
  (`crates/iroha_cli/src/main.rs:3821`). Har bir kichik buyruq `--input` / ni qo'llab-quvvatlaydi.
  `--output`, shuning uchun stollar ikki tomonlama tasdiqlash uchun ko'rsatmalar yuklarini tayyorlashi mumkin
  topshirish. Kastodiy marshrutlash `--custodian` orqali ifodalanadi.
- `repo query list|get` kelishuvlarni suratga olish uchun `FindRepoAgreements` dan foydalanadi va mumkin
  dalillar to'plamlari uchun JSON artefaktlariga yo'naltirilishi mumkin.
- `crates/iroha_cli/tests/cli_smoke.rs:2637` ostida CLI tutun sinovlari ta'minlaydi
  emit-to-fayl yo'li auditorlar uchun barqaror bo'lib qoladi.

### 2.2 SDK va avtomatlashtirish ilgaklari- Python ulanishlari `RepoAgreementRecord`, `RepoCashLeg`,
  `RepoCollateralLeg` va qulaylik yaratuvchilar
  (`python/iroha_python/iroha_python_rs/src/lib.rs:2216`) shuning uchun avtomatlashtirish mumkin
  tranzaktsiyalarni yig'ing va `next_margin_check_after` ni mahalliy sifatida baholang.
- JS/Swift yordamchilari bir xil Norito sxemalarini qayta ishlatishadi
  `javascript/iroha_js/src/instructionBuilders.js` va
  Eslatma uchun `IrohaSwift/Sources/IrohaSwift/ConfidentialEncryptedPayload.swift`
  ishlov berish; Repo boshqaruv tugmachalarini ulashda SDKlar ushbu hujjatga murojaat qilishlari kerak.

### 2.3 Hisobot voqealari va telemetriya

Har bir hayot tsikli harakati o'z ichiga olgan `AccountEvent::Repo(...)` yozuvlarini chiqaradi
`RepoAccountEvent::{Initiated,Settled,MarginCalled}` foydali yuklar qamrovi
ishtirokchi roli (`crates/iroha_data_model/src/events/data/events.rs:742`). Bosish
bu hodisalarni SIEM/log agregatoringizga kiritib, oʻzgartirishlar aniqlangan audit jurnalini oling
stol harakatlari, marja qo'ng'iroqlari va saqlovchi bildirishnomalari uchun.

### 2.4 Konfiguratsiyani tarqatish va tekshirish

Tugunlar `[settlement.repo]` bandidagi repo boshqaruv tugmalarini qabul qiladi.
`iroha_config` (`crates/iroha_config/src/parameters/user.rs:4071`). Buni davolang
Snippet boshqaruv dalillari shartnomasining bir qismi sifatida - uni versiya nazoratida bosqichma-bosqich ko'rsating
repo paketi bilan birga va o'zgartirishni o'zingiz orqali surishdan oldin uni xeshlang
avtomatlashtirish yoki ConfigMap. Minimal profil quyidagicha ko'rinadi:

```toml
[settlement.repo]
default_haircut_bps = 1500
margin_frequency_secs = 86400
eligible_collateral = ["bond#wonderland", "note#wonderland"]

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["note#wonderland", "bill#wonderland"]
```

Operatsion nazorat ro'yxati:

1. Yuqoridagi parchani (yoki ishlab chiqarish variantingiz) repo konfiguratsiyasiga kiriting
   u `irohad` ni oziqlantiradi va boshqaruv paketidagi SHA-256 ni yozib oladi
   sharhlovchilar siz joylashtirishni rejalashtirgan baytlarni farq qilishi mumkin.
2. Oʻzgarishlarni park boʻylab oʻtkazing (tizim birligi, Kubernetes ConfigMap va boshqalar).
   va har bir tugunni qayta ishga tushiring. Chiqarilgandan so'ng darhol Torii ni oling
   kelib chiqishi uchun konfiguratsiya surati:

   ```bash
   curl -sS "${TORII_URL}/v1/configuration" \
     -H "Authorization: Bearer ${TOKEN}" | jq .
   ```

   `ToriiClient.get_configuration()` xuddi shu maqsadda Python SDK da mavjud
   avtomatlashtirish uchun terilgan dalillar kerak bo'lganda maqsad.【python/iroha_python/src/iroha_python/client.py:5791】
3. So‘rov orqali ish vaqti talab qilingan kadans/soch kesishni amalga oshirishini isbotlang.
   `FindRepoAgreements` (yoki `iroha app repo margin --agreement-id ...`) va
   o'rnatilgan `RepoGovernance` qiymatlarini tekshirish. JSON javoblarini saqlang
   `artifacts/finance/repo/<agreement>/agreements_after.json` ostida; bu qadriyatlar
   `[settlement.repo]` dan olingan, shuning uchun ular ikkinchi darajali guvoh sifatida ishlaydi
   Torii ning `/v1/configuration` surati yetarli emas.
4. Har ikkala artefaktni - TOML parchasi va Torii/CLI snapshotlarini saqlang.
   boshqaruv so'rovini topshirishdan oldin dalillar to'plami. Auditorlar buni bilishlari kerak
   parchani takrorlang, uning xeshini tekshiring va uni ish vaqti ko'rinishi bilan bog'lang.

Ushbu ish jarayoni repo stollari hech qachon vaqtinchalik muhit o'zgaruvchilariga tayanmasligini ta'minlaydi
konfiguratsiya yo'li deterministik bo'lib qoladi va har bir boshqaruv chiptasi o'z ichiga oladi
F1 yo'l xaritasida kutilgan bir xil `iroha_config` dalillar to'plami.

### 2.5 Deterministik dalil jabduqlar

Birlik testi `repo_deterministic_lifecycle_proof_matches_fixture` (qarang
`crates/iroha_core/src/smartcontracts/isi/repo.rs`) har bir bosqichni ketma-ketlashtiradi
Norito JSON ramkasiga repo hayot aylanishi, uni kanonik moslama bilan solishtiradi
`crates/iroha_core/tests/fixtures/repo_lifecycle_proof.json` va xeshlaydi
bundle (fiksator dayjestida kuzatilgan
`crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`). Uni mahalliy sifatida quyidagi orqali ishga tushiring:

```bash
cargo test -p iroha_core \
  -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```Ushbu test endi standart `cargo test -p iroha_core` to'plamining bir qismi sifatida ishlaydi, shuning uchun CI
oniy tasvirni avtomatik ravishda himoya qiladi. Har doim repo semantikasi yoki moslamalari o'zgarganda,
JSONni ham yangilang, ham hazm qiling:

```bash
scripts/regen_repo_proof_fixture.sh
```

Yordamchi mahkamlangan `rust-toolchain.toml` kanalidan foydalanadi, moslamalarni qayta yozadi
`crates/iroha_core/tests/fixtures/` ostida va deterministik jabduqni qayta ishga tushiradi
shuning uchun tekshirilgan surat/dijest ish vaqti harakati bilan hamohang qoladi
auditorlar takrorlanadi.

### 2.4 Torii API sirtlari

- `GET /v1/repo/agreements` faol kelishuvlarni ixtiyoriy sahifalash, filtrlash bilan qaytaradi
  (`filter={...}`), saralash va manzilni formatlash parametrlari. Buni tezkor tekshiruvlar uchun foydalaning yoki
  xom JSON foydali yuklari etarli bo'lganda asboblar paneli.
- `POST /v1/repo/agreements/query` tuzilgan so'rov konvertini qabul qiladi (sahifalash, saralash,
  `FilterExpr`, `fetch_size`) shuning uchun quyi oqim xizmatlari buxgalteriya daftarini aniq varaqlashi mumkin.
- JavaScript SDK endi `listRepoAgreements`, `queryRepoAgreements` va iteratorni ochib beradi
  yordamchilar, shuning uchun brauzer/Node.js asboblari Rust/Python bilan bir xil yozilgan DTOlarni oladi.

### 2.4 Standart konfiguratsiya

Tugunlar `[settlement.repo]` ni o'qiydi
Ishga tushirish vaqtida `iroha_config::parameters::actual::Repo`; har qanday repo ko'rsatmasi
Parametrni nolga teng qoldiradigan bo'lsa, o'zidan oldingi ko'rsatuvlarga nisbatan normallashtiriladi
zanjirda qayd etilgan.【crates/iroha_core/src/smartcontracts/isi/repo.rs:40】 Bu
boshqaruvga har bir SDKga tegmasdan asosiy siyosatni oshirish (yoki tushirish) imkonini beradi
qo'ng'iroq-sayt, siyosat o'zgarishi to'liq hujjatlashtirilgan bo'lsa.

- `default_haircut_bps` – `RepoGovernance::haircut_bps()` da soch turmagi
  nolga teng. Ish vaqti uni ushlab turish uchun qattiq 10000bps shiftiga mahkamlaydi
  configs sane.【crates/iroha_core/src/smartcontracts/isi/repo.rs:44】
- `margin_frequency_secs` – `RepoMarginCallIsi` uchun kadans. Nollangan so'rovlar
  bu qiymatni meros qilib oladi, shuning uchun kadansni qisqartirish stollarni ko'proq chegaralashga majbur qiladi
  sukut bo'yicha tez-tez.【crates/iroha_core/src/smartcontracts/isi/repo.rs:49】
- `eligible_collateral` - `AssetDefinitionId`larning ixtiyoriy ruxsat etilgan ro'yxati. Qachon
  ro'yxat bo'sh emas `RepoIsi` to'plamdan tashqari har qanday garovni rad etadi va buning oldini oladi
  tekshirilmagan obligatsiyalarning tasodifiy kirishi.【crates/iroha_core/src/smartcontracts/isi/repo.rs:57】
- `collateral_substitution_matrix` – asl garovning xaritasi →
  ruxsat etilgan almashtirishlar. `ReverseRepoIsi` faqat bo'lganda almashtirishni qabul qiladi
  matritsa kalit sifatida qayd etilgan ta'rifni va uning o'rnini o'zgartirishni o'z ichiga oladi
  qiymat massivi; aks holda bo'shashish muvaffaqiyatsiz tugadi, bu boshqaruv tomonidan tasdiqlanganligini isbotlaydi
  narvon.【crates/iroha_core/src/smartcontracts/isi/repo.rs:74】

Ushbu tugmalar tugun konfiguratsiyasida `[settlement.repo]` ostida ishlaydi va
`iroha_config::parameters::user::Repo` orqali tahlil qilingan, shuning uchun ular qo'lga olinishi kerak
har bir boshqaruv dalillari to'plami.【crates/iroha_config/src/parameters/user.rs:3956】

```toml
[settlement.repo]
default_haircut_bps = 1750
margin_frequency_secs = 43200
eligible_collateral = ["bond#wonderland", "note#wonderland"]

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["note#wonderland", "bill#wonderland"]
```

**O'zgarishlarni boshqarish bo'yicha nazorat ro'yxati**1. Taklif etilgan TOML snippetini (jumladan, almashtirish matritsasi deltalarini) bosqichma-bosqich, xesh
   uni SHA-256 bilan bog'lang va parchani ham, xeshni ham boshqaruvga biriktiring
   chipta, shuning uchun sharhlovchilar baytlarni so'zma-so'z takrorlashlari mumkin.
2. Taklif/referendum ichidagi parchaga havola qiling (masalan,
   CLI boshqaruvidagi `--notes` maydoni) va kerakli tasdiqlarni to'plang
   F1 uchun. Imzolangan tasdiqlash paketini parcha biriktirilgan holda saqlang.
3. O'zgarishlarni butun park bo'ylab o'tkazing: `[settlement.repo]` yangilang, har birini qayta ishga tushiring
   tugunni oching, so'ngra `GET /v1/configuration` suratini oling (yoki
   `ToriiClient.getConfiguration`) har bir tengdosh uchun qo'llaniladigan qiymatlarni isbotlash.
4. `integration_tests/tests/repo.rs` plus-ni qayta ishga tushiring
   `repo_deterministic_lifecycle_proof_matches_fixture` va keyingi jurnallarni saqlang
   Auditorlar yangi standart sozlamalar saqlanib qolganligini ko'rishlari uchun konfiguratsiya farqiga o'ting
   determinizm.

Matritsali yozuvsiz ish vaqti aktivni o'zgartiradigan almashtirishlarni rad etadi
ta'rif, hatto umumiy `eligible_collateral` ro'yxati ruxsat bergan bo'lsa ham; topshirmoq
repo dalillari bilan bir qatorda oniy tasvirlarni sozlang, shunda auditorlar aniq ma'lumotlarni takrorlashlari mumkin
repo bron qilinganda qo'llaniladigan siyosat.

### 2.5 Konfiguratsiya dalillari va driftni aniqlash

Norito/`iroha_config` sanitariya-tesisat endi hal qilingan repo siyosatini ochib beradi.
`iroha_config::parameters::actual::Repo`, shuning uchun boshqaruv paketlari buni isbotlashi kerak
har bir tengdosh uchun qo'llaniladigan qiymatlar - faqat taklif qilingan TOML emas. Yechilganlarni qo'lga oling
konfiguratsiya va uning har bir chiqarilishidan keyin hazm qilish:

1. Konfiguratsiyani har bir tengdoshdan oling (`GET /v1/configuration` yoki
   `ToriiClient.getConfiguration`) va repo stanzasini ajratib oling:

   ```bash
   curl -s http://<torii-host>/v1/configuration \
     | jq -cS '.settlement.repo' \
     > artifacts/finance/repo/<agreement-id>/config/repo_config_actual.json
   ```

2. Kanonik JSON-ni xeshlang va uni dalillar manifestiga yozing. Qachon
   flot sog'lom, hash tengdoshlar orasida mos kelishi kerak, chunki `actual`
   standart sozlamalarni bosqichli `[settlement.repo]` parchasi bilan birlashtiradi:

   ```bash
   shasum -a 256 artifacts/finance/repo/<agreement-id>/config/repo_config_actual.json
   ```

3. Boshqaruv paketiga JSON + xesh-ni biriktiring va kirishni aks ettiring
   manifest boshqaruv DAGga yuklangan. Agar birorta hamkasbi divergent haqida xabar bersa
   hazm qiling, tarqatishni to'xtating va avval konfiguratsiya/holat driftini yarashtiring
   davom etmoqda.

### 2.6 Boshqaruvni tasdiqlash va dalillar to'plami

F1 yo'l xaritasi faqat repo stollari deterministik paketni yuborganda yopiladi
boshqaruv DAG, shuning uchun har bir o'zgarish (yangi soch turmagi, vasiylik siyosati yoki garov
matritsa) ovoz berish rejalashtirilganidan oldin bir xil artefaktlarni yuborishi kerak.【docs/source/governance_playbook.md:1】

**Qabul qilish paketi**1. **Kuzatuv shabloni** – nusxalash
   Sizning dalilingizga `docs/examples/finance/repo_governance_packet_template.md`
   katalog (masalan
   `artifacts/finance/repo/<agreement-id>/packet.md`) va metama'lumotlarni to'ldiring
   artefaktlarni xeshlashni boshlashdan oldin blokirovka qiling. Shablon boshqaruvni saqlaydi
   fayl yo'llari, SHA-256 dayjestlari va ro'yxati orqali kengashning kadans deterministik
   sharhlovchining tasdig'i bir joyda.
2. **Ko‘rsatma yuklamalari** – boshlash, bo‘shatish va qo‘ng‘iroqni cheklash bosqichi
   `iroha app repo ... --output` bilan ko'rsatmalar, shuning uchun ikki tomonlama boshqaruvni tasdiqlovchilar ko'rib chiqadi
   baytga o'xshash foydali yuklar. Har bir faylni xeshlang va ostida saqlang
   Ish stolining dalillar to'plami yonida `artifacts/finance/repo/<agreement-id>/`
   ushbu eslatmaning boshqa joyida havola qilingan.【crates/iroha_cli/src/main.rs:3821】
3. **Konfiguratsiya farqi** – aniq `[settlement.repo]` TOML snippetini kiriting
   (standart va almashtirish matritsasi) va uning SHA-256. Bu qaysi ekanligini isbotlaydi
   `iroha_config` tugmalari ovoz berishdan keyin faol bo'ladi va ovozni aks ettiradi.
   qabul qilish vaqtida repo ko'rsatmalarini normallashtiradigan ish vaqti maydonlari.【crates/iroha_config/src/parameters/user.rs:3956】
4. **Deterministik testlar** – oxirgisini ilova qiling
   `integration_tests/tests/repo.rs` jurnali va dan chiqishi
   `repo_deterministic_lifecycle_proof_matches_fixture` shuning uchun sharhlovchilar ko'radi
   bosqichma-bosqich ko'rsatmalarga mos keladigan hayot aylanishini isbotlovchi xesh.【integration_tests/tests/repo.rs:1】【crates/iroha_core/src/smartcontracts/isi/repo.rs:1450】
5. **Hodisa/temetriya snapshot** – oxirgi `AccountEvent::Repo(*)` eksporti
   miqyosdagi stollar uchun oqim va kengashga kerak bo'lgan har qanday asboblar paneli/ko'rsatkichlari
   tavakkalchilikni baholash (masalan, marjaning siljishi). Bu auditorlarga bir xil narsani beradi
   soxtalashtirilgan jurnal ular keyinchalik Torii dan qayta tiklanadi.【crates/iroha_data_model/src/events/data/events.rs:742】

**Tasdiqlash va ro‘yxatga olish**

- Boshqaruv chiptasi yoki referendum va ichidagi artefakt xeshlariga murojaat qiling
  bosqichli paketga havola, shuning uchun kengash standart marosimni kuzatishi mumkin
  boshqaruv kitobida maxsus yo'llarni quvmasdan tasvirlangan.【docs/source/governance_playbook.md:8】
- Qaysi qo'sh boshqaruvli imzolovchilar bosqichli ko'rsatmalar fayllarini ko'rib chiqqanini va
  o'z minnatdorchiliklarini xeshlar yonida saqlang; bu zanjirdagi dalildir
  Ushbu repo stollari ish vaqtiga qaramay, "ikki kishilik qoida" ni qondirdi
  faqat ishtirokchilar uchun bajarilishini ta'minlaydi.
- Kengash boshqaruvni tasdiqlash yozuvini (GAR) nashr qilganda, uni aks ettiring
  dalillar katalog ichida imzolangan daqiqa shunday kelajakda almashtirishlar yoki
  Sartaroshlik yangilanishlari uni qayta ko'rsatish o'rniga aniq qaror paketini keltirishi mumkin
  mantiqiy asos.

**Tasdiqlangandan keyingi tarqatishlar**1. Tasdiqlangan `[settlement.repo]` konfiguratsiyasini qo'llang va har bir tugunni qayta ishga tushiring (yoki aylantiring)
   bu sizning avtomatlashtirishingiz orqali). Darhol `GET /v1/configuration` raqamiga qo'ng'iroq qiling va arxivlang
   har bir tugun uchun javob, shuning uchun boshqaruv to'plami qaysi tengdoshlar qabul qilganligini ko'rsatadi
   o'zgartirish.【crates/iroha_torii/src/lib.rs:3225】
2. Deterministik repo testlarini qayta ishga tushiring va yangi jurnallarni biriktiring va tuzing
   metadata (git commit, toolchain), shuning uchun auditorlar hisob-kitoblarni takrorlashlari mumkin
   ishga tushirilgandan keyin dalil.
3. Boshqaruv kuzatuvchisini dalillar arxivi yo'li, xeshlar va
   kuzatuvchi bilan aloqa qilish, shuning uchun keyinchalik repo stollari o'rniga bir xil jarayonni meros qilib olishi mumkin
   nazorat ro'yxatini qaytadan chiqarish.

**Governance DAG nashri (kerak)**

1. Dalillar katalogini torting (konfiguratsiya parchasi, ko'rsatmalarning foydali yuklari, isbot jurnallari,
   GAR/daqiqa) va uni boshqaruv DAG quvur liniyasiga a sifatida topshiring
   `GovernancePayloadKind::PolicyUpdate` uchun izohlar bilan foydali yuk
   `agreement_id`, `iso_week` va tavsiya etilgan soch kesish/chekka qiymatlari; the
   quvur liniyasi xususiyatlari va CLI sirtlari yashaydi
   `docs/source/sorafs_governance_dag_plan.md`.
2. Noshir IPNS boshini yangilagandan so'ng, blok CID va bosh CIDni yozib oling
   boshqaruv kuzatuvchisida va GAR-da har kim o'zgarmasni olishi mumkin
   paket keyinroq. `sorafs governance dag head` va `sorafs governance dag list`
   ovoz berish ochilishidan oldin tugunning mahkamlanganligini tasdiqlashga ruxsat bering.
3. Repo dalillar arxivi yonida CAR faylini yoki blokli yukni saqlang
   auditorlar zanjirdagi boshqaruv qarorini aniq bilan kelishib olishlari mumkin
   tasdiqlangan zanjirdan tashqari paket.

### 2.7 Lifecycle oniy tasvirni yangilash

Har safar repo semantikasi o'zgarganda (stavkalar, hisob-kitoblar matematikasi, saqlash mantig'i yoki
sukut bo'yicha konfiguratsiya), deterministik hayot tsiklining oniy tasvirini yangilang, shunda boshqaruv mumkin
dalil jabduqlar teskari muhandislik holda yangi dayjest keltiring.

1. O'rnatilgan asboblar zanjiri ostidagi armaturalarni yangilang:

   ```bash
   scripts/regen_repo_proof_fixture.sh --toolchain <toolchain> \
     --bundle-dir artifacts/finance/repo/<agreement>
   ```

   Yordamchi vaqtinchalik katalogdagi chiqishlarni bosqichma-bosqich qiladi, kuzatilgan moslamalarni yangilaydi
   `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.{json,digest}` da,
   tekshirish uchun isbot testini qayta ishga tushiradi va (`--bundle-dir` o'rnatilganda)
   `repo_proof_snapshot.json` va `repo_proof_digest.txt` to'plamga tushadi
   auditorlar uchun ma'lumotnoma.
2. Kuzatiladigan moslamalarga tegmasdan artefaktlarni eksport qilish (masalan, quruq yugurish
   dalil), env yordamchilarini to'g'ridan-to'g'ri o'rnating:

   ```bash
   REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<agreement>/repo_proof_snapshot.json \
   REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<agreement>/repo_proof_digest.txt \
   cargo test -p iroha_core \
     -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
   ```

   `REPO_PROOF_SNAPSHOT_OUT` isbotdan yaxshilangan Norito JSONni oladi
   jabduqlar, `REPO_PROOF_DIGEST_OUT` esa katta olti burchakli dayjestni saqlaydi (bir
   Qulaylik uchun keyingi yangi qator). Yordamchi qachon fayllarni qayta yozishni rad etadi
   asosiy katalog mavjud emas, shuning uchun birinchi navbatda `artifacts/...` daraxtini yarating.
3. Eksport qilingan ikkala faylni ham kelishuv to‘plamiga biriktiring (3-§ ga qarang) va qayta tiklang
   `scripts/repo_evidence_manifest.py` orqali manifest, shuning uchun boshqaruv paketi
   yangilangan isbot artefaktlariga aniq havola qiladi. In-repo qurilmalari
   CI uchun haqiqat manbai bo'lib qoladi.

### 2.8 Foizlarni hisoblash va to'lash muddatini boshqarish**Deterministik foizlar matematikasi.** `RepoIsi` va `ReverseRepoIsi` naqd pul chiqaradi
ACT/360 yordamchisidan bo'shash vaqtida qarzdor
`compute_accrued_interest()`【crates/iroha_core/src/smartcontracts/isi/repo.rs:100】
va to'lov oyoqlarini rad qiluvchi `expected_cash_settlement()` ichidagi qo'riqchi
Bular *asosiy + foiz* dan kam qaytaradi.【crates/iroha_core/src/smartcontracts/isi/repo.rs:132】
Yordamchi `rate_bps`ni to'rt o'nlik kasrga normallashtiradi, uni ko'paytiradi
18 kasrdan foydalangan holda `elapsed_ms / (360 * 24h)` va nihoyat oraliqlarga yaxlitlash
naqd pul oyog'ining `NumericSpec` tomonidan e'lon qilingan shkalasi. Boshqaruv paketini saqlash uchun
takrorlanishi mumkin, yordamchini oziqlantiradigan to'rtta qiymatni oling:

1. `cash_leg.quantity` (asosiy),
2. `rate_bps`,
3. `initiated_timestamp_ms`, va
4. siz foydalanmoqchi bo'lgan vaqt tamg'asi (rejalashtirilgan GL yozuvlari uchun bu
   odatda `maturity_timestamp_ms`, lekin favqulodda vaziyatni bo'shatish haqiqiyni qayd etadi
   `ReverseRepoIsi::settlement_timestamp_ms`).

Tupleni bosqichma-bosqich ochish yo'riqnomasi bilan birga saqlang va qisqa isbotni ilova qiling
parcha, masalan:

```python
from decimal import Decimal
ACT_360_YEAR_MS = 24 * 60 * 60 * 1000 * 360

principal = Decimal("1000")
rate_bps = Decimal("1500")  # 150 bps
elapsed_ms = Decimal(maturity_ms - initiated_ms)
interest = principal * (rate_bps / Decimal(10_000)) * (elapsed_ms / Decimal(ACT_360_YEAR_MS))
expected_cash = principal + interest.quantize(Decimal("0.01"))
```

Dumaloq `expected_cash` teskari tomonda kodlangan `quantity` ga mos kelishi kerak
repo ko'rsatmasi. Skript chiqishini (yoki kalkulyator ish varag'ini) ichida saqlang
Auditorlar qayta hisoblashlari uchun `artifacts/finance/repo/<agreement>/interest.json`
savdo jadvalingizni talqin qilmasdan raqam. Integratsiya to'plami
allaqachon bir xil invariantni amalga oshiradi
(`repo_roundtrip_transfers_balances_and_clears_agreement`), lekin ops dalillari
ochiladigan aniq qiymatlarni keltirishi kerak.【integration_tests/tests/repo.rs:1】

**Marja va hisoblash kadansi.** Har bir kelishuv kadans yordamchilarini ochib beradi
`RepoAgreement::next_margin_check_after()` va keshlangan
`last_margin_check_timestamp_ms`, stollarga chekka o'tishini isbotlash imkonini beradi
`RepoMarginCallIsi` taqdim etishdan oldin ham siyosatga muvofiq rejalashtirilgan edi
tranzaksiya.【crates/iroha_data_model/src/repo.rs:113】【crates/iroha_core/src/smartcontracts/isi/repo.rs:557】
Har bir marja chaqiruvi dalillar to'plamida uchta artefaktni o'z ichiga olishi kerak:

1. `repo margin-call --agreement <id>` JSON chiqishi (yoki ekvivalent SDK
   foydali yuk), shartnoma identifikatori, blok vaqt tamg'asi uchun foydalanilgan
   tekshiring va uni qo'zg'atgan hokimiyat.【crates/iroha_cli/src/main.rs:3821】
2. Kelishuvning surati (`repo query get --agreement-id <id>`) olindi
   Qo'ng'iroqdan oldin darhol tekshiruvchilar kadansning to'g'ri kelishini tasdiqlashlari uchun
   (`current_timestamp_ms` `next_margin_check_after()` bilan solishtiring).
3. Har bir rol uchun `AccountEvent::Repo::MarginCalled` SSE/NDJSON tasmasi chiqariladi
   (tashabbuschi, kontragent va ixtiyoriy ravishda saqlovchi) chunki ish vaqti
   har bir ishtirokchi uchun tadbirni takrorlaydi.【crates/iroha_data_model/src/events/data/events.rs:742】

CI allaqachon ushbu qoidalarni orqali amalga oshiradi
`repo_margin_call_enforces_cadence_and_participant_rules`, bu qo'ng'iroqlarni rad etadi
erta yoki ruxsatsiz hisoblardan kelganlar.【integration_tests/tests/repo.rs:395】
Dalillar arxivida ushbu manbani takrorlash F1 yo'l xaritasini yopadi
Hujjatlar bo'shlig'i: boshqaruvni tekshiruvchilar bir xil vaqt belgilarini ko'rishlari mumkin
§2.7 da olingan deterministik isbot xesh bilan birga ishlash vaqtiga tayangan
va §3.2 da muhokama qilingan manifest.

### 2.8 Uch tomonlama saqlashni tasdiqlash va monitoring qilish“Yoʻl xaritasi” **F1** shuningdek, garov toʻxtatilgan joyda uch tomonlama repolarni chaqiradi
kontragent emas, balki qo'riqchi. Ish vaqti qo'riqchi yo'lini amalga oshiradi
`RepoAgreement::custodian` ni davom ettirish orqali garovga qo'yilgan aktivlarni
ishga tushirish paytida va emitentning hisobi
Auditorlar ko'rishi uchun hayot tsiklining har bir bosqichi uchun `RepoAccountRole::Custodian` hodisalari
har bir vaqt tamg'asida garovga ega bo'lganlar.【crates/iroha_data_model/src/repo.rs:74】【crates/iroha_core/src/smartcontracts/isi/repo.rs:252】【integration_tests/tests/repo.rs:951】
Yuqorida sanab o'tilgan ikki tomonlama dalillarga qo'shimcha ravishda, har bir uch tomonlama repo bo'lishi kerak
boshqaruv paketi tugallangan deb hisoblanishidan oldin quyidagi artefaktlarni qo'lga kiriting.

**Qo'shimcha qabul qilish talablari**

1. **Kastodiyan tasdiqnomasi.** Stollarda imzolangan tasdiqnoma saqlanishi kerak.
   repo identifikatorini, saqlash oynasini, marshrutni tasdiqlovchi har bir saqlovchi
   hisob va hisob-kitob SLAlari. Imzolangan hujjatni ilova qiling
   (`artifacts/finance/repo/<agreement>/custodian_ack_<custodian>.md`)
   va uni boshqaruv paketiga havola qiling, shunda sharhlovchilar buni ko'rishlari mumkin
   uchinchi tomon tashabbuskor/kontragent ma'qullagan bir xil baytlarga rozi bo'ldi.
2. **Kastodian daftarining snapshoti.** Boshlanish garovni kastodianga o'tkazadi
   hisob va unwind uni tashabbuskorga qaytaradi; capture the relevant
   `FindAssets` saqlovchi uchun har bir oyoqdan oldin va keyin auditorlar chiqishi
   balanslar bosqichli ko'rsatmalarga mos kelishini tasdiqlashi mumkin.【crates/iroha_core/src/smartcontracts/isi/repo.rs:252】【crates/iroha_core/src/smartcontracts/isi/repo.rs:1641】
3. **Tadbir kvitansiyalari.** Barcha rollar va rollar uchun `RepoAccountEvent` oqimini aks ettiring.
   saqlovchining foydali yukini tashabbuskor/kontragent yozuvlari bilan birga saqlang.
   Ish vaqti har bir rol uchun alohida hodisalarni chiqaradi
   `RepoAccountRole::{Initiator,Counterparty,Custodian}`, shuning uchun xom ashyoni biriktiring
   SSE tasmasi har uch tomonning bir xil vaqt belgilarini ko'rganligini isbotlaydi va
   hisob-kitob summalari.【crates/iroha_data_model/src/events/data/events.rs:742】【integration_tests/tests/repo.rs:1508】
4. **Kastodiyning tayyorligini tekshirish ro'yxati.** Repo ma'lumotnomalari ishga tushganda
   shimlar (masalan, depozitni yarashtirish yoki doimiy ko'rsatmalar), yozib olish
   avtomatlashtirish kontakti va ish jarayonini takrorlash uchun ishlatiladigan buyruq (masalan
   `iroha app repo initiate --custodian ... --dry-run` sifatida) sharhlovchilar erishishlari mumkin
   mashg'ulotlar paytida saqlovchi operatorlar.

| Dalil | Buyruq / yo'l | Maqsad |
|----------|----------------|---------|
| Vasiylik guvohnomasi (`custodian_ack_<custodian>.md`) | `docs/examples/finance/repo_governance_packet_template.md` da havola qilingan imzolangan eslatmaga havola (urug' sifatida `docs/examples/finance/repo_custodian_ack_template.md` dan foydalaning). | Aktivlar ko‘chirilishidan oldin uchinchi tomon repo identifikatori, saqlash SLA va hisob-kitob kanalini qabul qilganligini ko‘rsatadi. |
| Saqlash ob'ektining surati | `iroha json --query FindAssets '{ "id": "...#<custodian>" }' > artifacts/.../assets/custodian_<ts>.json` | Garovni `RepoIsi` kodlaganidek, chap/qaytarilganligini isbotlaydi. |
| Custodian `RepoAccountEvent` tasmasi | `torii-events --account <custodian> --event-type repo > artifacts/.../events/custodian.ndjson` | `RepoAccountRole::Custodian` yuklamalarini ishga tushirish, chegara qo'ng'iroqlari va bo'shash uchun chiqarilgan ish vaqtini oladi. |
| Saqlash burg'ulash jurnali | `artifacts/.../governance/drills/<timestamp>-custodian.log` | Hujjatlar saqlovchi orqaga qaytarish yoki hisob-kitob skriptlarini amalga oshirgan joyda quruq ishlaydi. |uchun bir xil xeshlash ish oqimini (`scripts/repo_evidence_manifest.py`) qayta ishlatish
kastodian tasdiqi, aktivlar snapshotlari va voqealar tasmasi uch tomonli boʻlib qoladi
paketlarni takrorlash mumkin. Bir nechta qo'riqchilar kitobda qatnashganda, yarating
har bir saqlovchi uchun kichik kataloglar, shuning uchun manifest qaysi fayllarga tegishli ekanligini ta'kidlaydi
har bir partiya; boshqaruv chiptasi har bir manifest xesh va ga havola qilishi kerak
mos keladigan tasdiq fayli. O'z ichiga olgan integratsiya testlari
`repo_initiation_with_custodian_routes_collateral` va
`reverse_repo_with_custodian_emits_events_for_all_parties` allaqachon amal qiladi
ish vaqtining xatti-harakati - dalillar to'plami ichida ularning artefaktlarini aks ettirish - bu nima
**F1** yo'l xaritasi uch tomonlama stsenariy uchun GA-tayyor hujjatlarni jo'natish imkonini beradi.【integration_tests/tests/repo.rs:951】【integration_tests/tests/repo.rs:1508】

### 2.9 Tasdiqlashdan keyingi konfiguratsiya suratlari

Boshqaruv o'zgarishlarni ma'qullagandan so'ng va `[settlement.repo]` bandi boshlanadi.
klaster, har bir tengdoshdan autentifikatsiya qilingan konfiguratsiya oniy rasmini oling
auditorlar tasdiqlangan qiymatlarning jonli ekanligini isbotlashlari mumkin. Torii ni ochib beradi
Shu maqsadda `/v1/configuration` marshruti va barcha SDK sirt yordamchilari, masalan
`ToriiClient.getConfiguration`, shuning uchun suratga olish ish jarayoni stol skriptlari uchun ishlaydi,
CI yoki qo'lda operator ishlaydi.【crates/iroha_torii/src/lib.rs:3225】【javascript/iroha_js/src/toriiClient.js:2115】【IrohaSwift/Sources/IrohaSwift/Toriift:68】

1. `GET /v1/configuration` (yoki SDK yordamchisi) ga darhol qo'ng'iroq qiling.
   tarqatish. Toʻliq JSON ostida saqlang
   `artifacts/finance/repo/<agreement>/config/peers/<peer-id>.json` va yozib oling
   `config/config_snapshot_index.md` da blok balandligi/klaster vaqt tamg'asi.
   ```bash
   mkdir -p artifacts/finance/repo/<slug>/config/peers
   curl -fsSL https://peer01.example/v1/configuration \
     | jq '.' \
     > artifacts/finance/repo/<slug>/config/peers/peer01.json
   ```
2. Har bir suratni (`sha256sum config/peers/*.json`) xeshlang va keyingi dayjestni qayd qiling
   boshqaruv paketi shablonidagi peer identifikatoriga. Bu qaysi tengdosh ekanligini isbotlaydi
   siyosatni qabul qildi va qaysi commit/toolchain oniy tasvirni yaratdi.
3. Har bir suratdagi `.settlement.repo` blokini bosqichli blok bilan solishtiring.
   `[settlement.repo]` TOML parchasi; har qanday drift va qayta ishlashni yozib oling
   `repo query get --agreement-id <id> --pretty` shuning uchun dalillar to'plami ko'rsatilgan
   ish vaqti konfiguratsiyasi va normallashtirilgan `RepoGovernance` qiymatlari
   kelishuv bilan saqlanadi.【crates/iroha_cli/src/main.rs:3821】
4. Snapshot fayllari va xulosa indeksini dalillar manifestiga biriktiring (qarang.
   §3.2) shuning uchun boshqaruv yozuvi tasdiqlangan o'zgarishlarni haqiqiy tengdosh bilan bog'laydi
   konfiguratsiya baytlari. Boshqaruv shabloni buni o'z ichiga olgan holda yangilandi
   jadval, shuning uchun har bir kelajakdagi repo paketi bir xil dalilga ega.

Ushbu suratlarni olish `iroha_config` hujjatidagi bo'shliqni yopadi.
yo'l xaritasida: sharhlovchilar endi bosqichli TOMLni baytlardan farqlashlari mumkin
tengdoshlar hisobotlari va auditorlar har doim repo o'zgartirilganda taqqoslashni qayta boshlashlari mumkin
tergov ostida.

## 3. Deterministik dalillar ish jarayoni1. **Ko'rsatma kelib chiqishini yozib oling**
   - `iroha app repo ... --output` orqali repo/ochish yukini yarating.
   - `InstructionBox` JSON ostida saqlang
     `artifacts/finance/repo/<agreement-id>/initiation.json`.
2. **Capture ledger state**
   - Oldin `iroha app repo query list --pretty > artifacts/.../agreements.json` ni ishga tushiring
     va hisob-kitobdan so'ng hisob-kitob qilingandan so'ng hisob-kitob qilingan qoldiqlarni isbotlash.
   - Ixtiyoriy ravishda arxivlash uchun `iroha json` yoki SDK yordamchilari orqali `FindAssets` soʻrovi
     aktiv qoldiqlari repo oyog'ida tegdi.
3. **Doimiy voqealar oqimi**
   - `AccountEvent::Repo` ga Torii SSE orqali obuna bo'ling yoki torting va biriktiring
     dalillar katalogiga JSON yubordi. Bu soxtalashtirishni qondiradi
     logging bandi, chunki voqealar kuzatilgan tengdoshlar tomonidan imzolangan
     har bir o'zgarish.
4. **Deterministik testlarni o‘tkazing**
   - CI allaqachon `integration_tests/tests/repo.rs` ishlaydi; qo'lda ro'yxatdan o'tish uchun,
     `cargo test -p integration_tests repo::` ni bajaring va log plusni arxivlang
     `target/debug/deps/repo-*` JUnit chiqishi.
5. **Boshqaruvni ketma-ketlashtirish va sozlash**
   - Ushbu davr uchun foydalanilgan `[settlement.repo]` konfiguratsiyasini tekshiring (yoki biriktiring),
     shu jumladan, soch turmagi / mos ro'yxatlar. Bu audit takrorlashlariga mos kelishiga imkon beradi
     `RepoAgreement` da qayd etilgan ish vaqti bilan normallashtirilgan boshqaruv.

### 3.1 Dalillar to'plamining joylashuvi

Ushbu bo'limda ko'rsatilgan har bir artefaktni bitta kelishuv ostida saqlang
katalog, shuning uchun boshqaruv bitta daraxtni arxivlashi yoki xeshlashi mumkin. Tavsiya etilgan tartib:

```
artifacts/finance/repo/<agreement-id>/
├── agreements_before.json
├── agreements_after.json
├── initiation.json
├── unwind.json
├── margin/
│   └── 2026-04-30.json
├── events/
│   └── repo-events.ndjson
├── config/
│   ├── settlement_repo.toml
│   └── peers/
│       ├── peer01.json
│       └── peer02.json
├── repo_proof_snapshot.json
├── repo_proof_digest.txt
└── tests/
    └── repo_lifecycle.log
```

- `agreements_before/after.json` `repo query list` chiqishini ushlaydi, shunda auditorlar
  daftar shartnomani bekor qilganligini isbotlash.
- `initiation.json`, `unwind.json` va `margin/*.json` aniq Norito
  foydali yuklar `iroha app repo ... --output` bilan bosqichli.
- `events/repo-events.ndjson` `AccountEvent::Repo(*)` oqimini takrorlaydi
  `tests/repo_lifecycle.log` `cargo test` dalillarini saqlaydi.
- `repo_proof_snapshot.json` va `repo_proof_digest.txt` suratdan olingan
  §2.7 da protsedurani yangilang va sharhlovchilarga hayot tsikli xeshini qayta hisoblashiga ruxsat bering
  jabduqni qayta ishga tushirmasdan.
- `config/settlement_repo.toml` tarkibida `[settlement.repo]` parchasi mavjud
  repo amalga oshirilganda faol bo'lgan (sochlar, almashtirish matritsasi).
- `config/peers/*.json` har bir tengdosh uchun `/v1/configuration` suratlarini oladi,
  bosqichli TOML va ish vaqti qiymatlari tengdoshlari hisoboti o'rtasidagi halqani yopish
  Torii dan yuqori.

### 3.2 Xesh manifest avlodi

Sharhlovchilar xeshlarni tekshirishlari uchun har bir toʻplamga deterministik manifestni biriktiring
arxivni ochmasdan. `scripts/repo_evidence_manifest.py` da yordamchi
kelishuv katalogida yuradi, `size`, `sha256`, `blake2b` va oxirgi
har bir fayl uchun o'zgartirilgan vaqt tamg'asi va JSON xulosasini yozadi:

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/finance/repo/wonderland-2026q1 \
  --agreement-id repo#wonderland \
  --output artifacts/finance/repo/wonderland-2026q1/manifest.json \
  --exclude 'scratch/*'
```

Jeneratör yo'llarni leksikografik jihatdan saralaydi, chiqish faylini u yashab turganda o'tkazib yuboradi
bir xil katalog ichida va boshqaruv to'g'ridan-to'g'ri nusxalashi mumkin bo'lgan jami chiqaradi
o'zgartirish chiptasiga. `--output` manifest choplari o'tkazib yuborilsa
stdout, bu stolni ko'rib chiqish paytida tez farqlar uchun qulay.
Chizilgan materialni qoldirmaslik uchun `--exclude <glob>` dan foydalaning (masalan, `--exclude 'scratch/*' --exclude '*.tmp'`)
fayllarni paketdan ko'chirmasdan; glob naqsh har doim uchun amal qiladi
`--root` ga nisbatan yo'l.Manifest namunasi (qisqalik uchun qisqartirilgan):

```json
{
  "agreement_id": "repo#wonderland",
  "generated_at": "2026-04-30T11:58:43Z",
  "root": "/var/tmp/repo/wonderland-2026q1",
  "file_count": 5,
  "total_bytes": 1898,
  "files": [
    {
      "path": "agreements_after.json",
      "size": 512,
      "sha256": "6b6ca81b00d0d889272142ce1e6456872dd6b01ce77fcd1905f7374fc7c110cc",
      "blake2b": "5f0c7f03d15cd2a69a120f85df2a4a4a219a716e1f2ec5852a9eb4cdb443cbfe3c1e8cd02b3b7dbfb89ab51a1067f4107be9eab7d5b46a957c07994eb60bb070",
      "modified_at": "2026-04-30T11:42:01Z"
    },
    {
      "path": "initiation.json",
      "size": 274,
      "sha256": "7a1a0ec8c8c5d43485c3fee2455f996191f0e17a9a7d6b25fc47df0ba8de91e7",
      "blake2b": "ce72691b4e26605f2e8a6486d2b43a3c2b472493efd824ab93683a1c1d77e4cff40f5a8d99d138651b93bcd1b1cb5aa855f2c49b5f345d8fac41f5b221859621",
      "modified_at": "2026-04-30T11:39:55Z"
    }
  ]
}
```

Manifestni dalillar to'plami yoniga qo'shing va uning SHA-256 xeshiga havola qiling
boshqaruv taklifida stollar, operatorlar va auditorlar bir xil bo'ladi
asosli haqiqat.

### 3.3 Boshqaruvni o'zgartirish jurnali va orqaga qaytarish mashqlari

Moliya kengashi har bir repo so'rovini, soch turmagini yoki almashtirishni kutadi
bog'lanishi mumkin bo'lgan takrorlanadigan boshqaruv paketi bilan kelishi uchun matritsa o'zgarishi
to'g'ridan-to'g'ri referendum bayonnomasidan.【docs/source/governance_playbook.md:1】

1. **Boshqaruv paketini yarating**
   - Shartnoma uchun dalillar to'plamidan nusxa ko'chiring
     `artifacts/finance/repo/<agreement-id>/governance/`.
   - `gar.json` (kengash ma'qullash yozuvi), `referendum.md` (ma'qullagan) qo'shing
     va qaysi xeshlarni ko'rib chiqdilar) va `rollback_playbook.md`
     `repo_runbook.md` dan teskari o'zgartirish jarayonini umumlashtirish
     §§4–5.【docs/source/finance/repo_runbook.md:1】
   - §3.2 dan deterministik manifest xeshini oling
     `hashes.txt`, shuning uchun sharhlovchilar Torii moslashuvida ko'rgan foydali yuklarni tasdiqlashlari mumkin
     bosqichli baytlar.
2. **Referendumdagi paketga murojaat qiling**
   - `iroha app governance referendum submit` (yoki unga tenglashtirilgan SDK
     yordamchi) `--notes` da `hashes.txt` dan manifest xeshni o'z ichiga oladi
     foydali yuk, shuning uchun GAR o'zgarmas paketga ishora qiladi.
   - Xuddi shu xeshni boshqaruv kuzatuvchisi yoki chiptalar tizimi ichida fayllang
     audit izlari skrinshot olish asboblar paneliga tayanmaydi.
3. **Hujjatlarni tayyorlash va qaytarish**
   - Referendum o'tgandan so'ng, `ops/drill-log.md` ni repo bilan yangilang
     kelishuv identifikatori, o'rnatilgan konfiguratsiya xeshi, GAR identifikatori va operator aloqasi
     Har choraklik burg'ulash rekordi moliyaviy harakatlarni o'z ichiga oladi.【ops/drill-log.md:1】
   - Agar orqaga qaytarish matkap bajarilsa, imzolanganni ilova qiling
     `rollback_playbook.md` va `iroha app repo unwind` dan CLI chiqishi
     `governance/drills/<timestamp>.log` va undan foydalanib kengashga xabar bering
     boshqaruv kitobida tasvirlangan qadamlar.

Misol tartibi:

```
artifacts/finance/repo/<agreement-id>/governance/
├── gar.json
├── hashes.txt
├── referendum.md
├── rollback_playbook.md
└── drills/
    └── 2026-05-12T09-00Z.log
```

GAR, referendum va burg'ulash artefaktlarini hayot aylanishi bilan birga saqlash
dalillar har bir repo o'zgarishi F1 boshqaruvi yo'l xaritasini qondirishini kafolatlaydi
keyin buyurtma chipta spelunking talab qilmasdan bar.

### 3.4 Hayotiy tsiklni boshqarish bo'yicha nazorat ro'yxati

Yo'l xaritasi **F1** boshqaruvni boshlash, hisoblash/marja va
uch partiya dam oladi. Quyidagi jadval tasdiqlashlarni birlashtiradi, deterministik
artefaktlar va har bir hayot tsikli bosqichidagi test ma'lumotnomalari, shuning uchun moliya bo'limlari a
paketni yig'ishda yagona nazorat ro'yxati.| Hayotiy aylanish bosqichi | Kerakli tasdiqlar va chiptalar | Deterministik artefaktlar va buyruqlar | Bog'langan regressiya qamrovi |
|----------------|---------------------------------------|-----------------------------------|--------------------------------------|
| **Tashabbus (ikki tomonlama yoki uch tomonlama)** | `docs/examples/finance/repo_governance_packet_template.md`, `[settlement.repo]` diff va GAR identifikatori bilan boshqaruv chiptasi, `--custodian` o'rnatilganda qo'riqchi tasdiqlovi orqali qayd etilgan ikki tomonlama nazoratdan chiqish. | Ko'rsatmalarni`iroha --config client.toml --output repo initiate ...` orqali bosqichma-bosqich bajaring.`scripts/repo_evidence_manifest.py` dan foydalanish muddatini tasdiqlovchi snapshotni (`REPO_PROOF_*` env vars) va to'plam manifestini chiqaring.Eng so'nggi `scripts/repo_evidence_manifest.py` va `config/config_snapshot_index.md` Norito va Norito X ni biriktiring. parcha (soch kesish, mos ro'yxat, almashtirish matritsasi). | `integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement` (ikki tomonlama) va `integration_tests/tests/repo.rs::repo_roundtrip_with_custodian_routes_collateral` (uch tomonli) ish vaqti bosqichli yuklamalarga mos kelishini isbotlaydi. |
| **Marja qo‘ng‘iroqlarini hisoblash kadensi** | Stol rahbari + risk menejeri boshqaruv paketida hujjatlashtirilgan kadens oynasini tasdiqlaydi; chipta rejalashtirilgan `RepoMarginCallIsi` ga havola qiladi. | `iroha app repo margin-call` ga qoʻngʻiroq qilishdan oldin `iroha app repo margin --agreement-id` chiqishini yozib oling, natijada olingan JSONni xeshlang va `RepoAccountEvent::MarginCalled` SSE foydali yukini dalillar toʻplamiga arxivlang.CLI jurnalini deterministik isbot xesh yonida saqlang. | `integration_tests/tests/repo.rs::repo_margin_call_enforces_cadence_and_participant_rules` ish vaqti muddatidan oldin qo'ng'iroqlarni va ishtirok etmaydigan yuborishlarni rad etishini kafolatlaydi. |
| **Garov o'rnini bosish va to'lov muddatini bo'shatish** | Boshqaruvni o'zgartirish yozuvida talab qilinadigan `collateral_substitution_matrix` yozuvlari va soch kesish siyosati keltirilgan; kengash bayonnomalari almashtirish juftligi SHA-256 xeshini ro'yxatga oladi. | Yechish oyogʻini `iroha app repo unwind --output ... --settlement-timestamp-ms <planned>` bilan sahnalashtiring, shunda ACT/360 hisobi (§2.8) ham, almashtirish foydali yuki ham takrorlanishi mumkin.`[settlement.repo]` TOML snippetini, almashtirish manifestini va Norito-artdagi toʻlovni qoʻshing. | `integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement` ichidagi almashtirish bo'yicha aylanib-o'tish, kelishuv identifikatorini doimiy ushlab turganda, etarli bo'lmagan va tasdiqlangan almashtirish oqimlarini amalga oshiradi. |
| **Favqulodda bo'shatish / orqaga qaytarish burg'ulash** | Voqea qo'mondoni + moliya kengashi `docs/source/finance/repo_runbook.md` (4–5-bo'limlar) da tavsiflanganidek, orqaga qaytarishni tasdiqlaydi va `ops/drill-log.md` da yozuvni yozib oladi. | Bosqichli orqaga qaytarish yukidan foydalanib `iroha app repo unwind` ni bajaring, `governance/drills/<timestamp>.log` ga CLI jurnallari + GAR havolasini qo'shing va mashqdan oldin/keyin determinizmni isbotlash uchun `repo_deterministic_lifecycle_proof_matches_fixture` va `scripts/repo_evidence_manifest.py` yordamchisini qayta ishga tushiring. | Baxtli yo'l `integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement` tomonidan qoplanadi; mashq bosqichlariga rioya qilish boshqaruv artefaktlarini ushbu test tomonidan amalga oshiriladigan ish vaqti kafolatlari bilan moslashtiradi. |

**Stol vaqt jadvali.**1. Qabul qilish shablonini nusxalash, metama'lumotlar blokini to'ldirish (kelishuv identifikatori, GAR chiptasi,
   saqlovchi, konfiguratsiya xesh) va dalillar katalogini yarating.
2. Har bir ko'rsatmani bosqichma-bosqich bajaring (`initiate`, `margin-call`, `unwind`, almashtirish)
   `--output` rejimi, JSON-ni xeshlang va har bir xesh yonida tasdiqlashlarni qayd qiling.
3. Yashash siklini isbotlovchi snapshotni chiqaring va sahnalashtirgandan so'ng darhol namoyon bo'ling
   boshqaruv sharhlovchilari dayjestni bir xil repo moslamalari bilan qayta hisoblashlari mumkin.
4. Ta'sir qilingan hisoblar uchun `RepoAccountEvent::*` SSE foydali yuklarini aks ettirish va tushirish
   eksport qilingan NDJSON `artifacts/finance/repo/<agreement-id>/events.ndjson`
   paketni topshirishdan oldin.
5. Ovoz berish tugagach, `hashes.txt` ni GAR identifikatori bilan yangilang,
   konfiguratsiya xeshini va manifest nazorat summasini belgilang, shunda kengash ishlab chiqarishni kuzatishi mumkin
   mahalliy skriptlarni qayta ishga tushirmasdan.

### 3.5 Boshqaruv paketini tezkor ishga tushirish

F1 yo'l xaritasi sharhlovchilari ular murojaat qilishlari mumkin bo'lgan qisqacha nazorat ro'yxatini so'rashdi
dalillar to'plamini yig'ish. Repo so'rovi bo'lganda quyidagi ketma-ketlikni bajaring
yoki siyosat o'zgarishi boshqaruvga yo'naltiriladi:

1. **Hayot davrini tasdiqlovchi artefaktlarni eksport qiling.**
   ```bash
   mkdir -p artifacts/finance/repo/<slug>
   REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json \
   REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt \
   cargo test -p iroha_core \
     -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
   ```
   Eksport qilingan JSON + dayjesti roʻyxatdan oʻtgan moslamalarni aks ettiradi
   `crates/iroha_core/tests/fixtures/`, shuning uchun sharhlovchilar hayot aylanishini qayta hisoblashlari mumkin
   butun to'plamni qayta ishga tushirmasdan ramka (2.7-bandga qarang). Siz ham qo'ng'iroq qilishingiz mumkin
   `scripts/regen_repo_proof_fixture.sh --bundle-dir artifacts/finance/repo/<slug>`
   bir qadamda bir xil fayllarni yangilash va nusxalash.
2. **Har bir koʻrsatmani bosqichma-bosqich va xeshlash.** Boshlanish/marja/boʻshatishni yarating
   `iroha app repo ... --output` bilan foydali yuklar. Har bir fayl uchun SHA-256 ni yozib oling
   (`hashes/` ostida saqlang) shuning uchun `docs/examples/finance/repo_governance_packet_template.md`
   ko'rib chiqilgan stollarga bir xil baytlarga murojaat qilishi mumkin.
3. **Buxgalteriya kitobi/konfiguratsiya suratlarini saqlang.** `repo query list` chiqishini oldin/keyin eksport qiling
   hisob-kitob qiling, qo'llaniladigan `[settlement.repo]` TOML blokini tashlang va
   tegishli `AccountEvent::Repo(*)` SSE tasmasini aks ettiring
   `artifacts/finance/repo/<slug>/events/repo-events.ndjson`. GARdan keyin
   o'tadi, har bir tengdosh uchun `/v1/configuration` oniy rasmlarni oling (§2.9) va ularni saqlang
   `config/peers/` ostida, shuning uchun boshqaruv paketi tarqatish muvaffaqiyatli bo'lganligini isbotlaydi.
4. **Dalil manifestini yarating.**
   ```bash
   python3 scripts/repo_evidence_manifest.py \
     --root artifacts/finance/repo/<slug> \
     --agreement-id <repo-id> \
     --output artifacts/finance/repo/<slug>/manifest.json
   ```
   Manifest xeshini boshqaruv chiptasi yoki GAR daqiqalari ichiga kiriting
   auditorlar paketni xom to'plamni yuklab olmasdan farqlashlari mumkin (3.2-bandga qarang).
5. **Paketni yig'ing.** Shablondan nusxa oling
   `docs/examples/finance/repo_governance_packet_template.md`, metama'lumotlarni to'ldiring,
   dalil suratini/digestini, manifestni, konfiguratsiya xeshini, SSE eksportini va testni ilova qiling
   jurnallar, keyin referendum `--notes` maydoni ichidagi manifest SHA-256 ni keltiring.
   Tugallangan Markdownni artefaktlar yonida saqlang, shunda orqaga qaytarishlar meros qilib olinadi
   tasdiqlash uchun yuborgan aniq dalillar.

Yuqoridagi amallarni repo so'rovini amalga oshirgandan so'ng darhol bajarish, degan ma'noni anglatadi
boshqaruv paketi so'nggi daqiqadan qochib, kengash yig'ilishi bilanoq tayyor bo'ladi
xeshlarni yoki voqea oqimlarini qayta yaratish uchun skrambles.

## 4. Uch tomonli vasiylik va garovni almashtirish- **Kastodianlar:** `--custodian <account>` marshrutlarini garovga o'tish
  saqlash ombori; ish vaqti hisob mavjudligini ta'minlaydi va rol belgilarini chiqaradi
  vasiylar yarashtirishi mumkin bo'lgan voqealar (`RepoAccountRole::Custodian`). Davlat
  mashina qo'riqchisi har ikki tomon mos keladigan shartnomalarni rad etadi.
- **Kafolatni almashtirish:** Yechish oyog'i boshqa garovni berishi mumkin
  dan **kam bo'lmagan** bo'lsa, almashtirish vaqtida miqdor/seriya
  garovga qo'yilgan miqdor *va* almashtirish matritsasi juftlikka ruxsat beradi; `ReverseRepoIsi`
  ikkala shartni ham amalga oshiradi
  (`crates/iroha_core/src/smartcontracts/isi/repo.rs:414`–`437`). Integratsiya
  test to'plami ham rad etish yo'lini, ham muvaffaqiyatli almashtirishni mashq qiladi
  qaytish (`integration_tests/tests/repo.rs:261`–`359`), repo birligi esa
  testlar yangi matritsa siyosatini qamrab oladi.
- **ISO 20022 xaritalash:** ISO konvertlarini yaratish yoki tashqi moslashtirishda
  tizimlarida hujjatlashtirilgan maydon xaritasini qayta ishlating
  `docs/source/finance/settlement_iso_mapping.md` (`colr.007`, `sese.023`,
  `sese.025`) shuning uchun Norito foydali yuk va ISO tasdiqlari sinxronlashtiriladi.

## 5. Operatsion nazorat ro'yxatlari

### Kundalik oldindan ochiq

1. `iroha app repo query list` orqali to'lanmagan shartnomani eksport qiling.
2. G'aznachilik inventarizatsiyasi bilan solishtiring va tegishli garov konfiguratsiyasini ta'minlang
   rejalashtirilgan kitobga mos keladi.
3. `--output` bilan yaqinlashib kelayotgan reposlarni bosqichma-bosqich bajaring va ikki tomonlama tasdiqlarni to'plang.

### Bir kunlik monitoring

1. `AccountEvent::Repo` tashabbuskor/kontragent/stodian uchun obuna bo'ling
   hisoblar; kutilmagan boshlanishlar sodir bo'lganda ogohlantirish.
2. `iroha app repo margin --agreement-id ID` dan foydalaning (yoki
   `RepoAgreementRecord::next_margin_check_after`) kadansni aniqlash uchun soatiga
   drift; trigger `repo margin-call` qachon `is_due = true`.
3. Operator bosh harflari bilan barcha chegara qo'ng'iroqlarini jurnalga kiriting va CLI JSON chiqishini qo'shing
   dalillar katalogi.

### Kun oxiri + hisob-kitobdan keyingi

1. `repo query list` ni qayta ishga tushiring va bekor qilingan kelishuvlar olib tashlanganligini tasdiqlang.
2. `RepoAccountEvent::Settled` foydali yuklarni arxivlash va naqd pul/garov taʼminotini oʻzaro tekshirish
   `FindAssets` orqali balanslar.
3. Repo mashqlari yoki voqea sinovlari paytida `ops/drill-log.md` da matkap yozuvini kiriting
   yugurish; vaqt belgilari uchun `scripts/telemetry/log_sorafs_drill.sh` konventsiyalaridan qayta foydalaning.

## 6. Firibgarlik va orqaga qaytarish tartiblari

- **Ikki boshqaruv:** Har doim `--output` bilan ko'rsatmalar yarating va
  Birgalikda imzolash uchun JSON. Jarayon darajasida bir partiyaning taqdimnomalarini rad eting
  garchi ish vaqti tashabbuskor vakolatini amalga oshirsa ham.
- **Buzg'unchilikka qarshi jurnal:** `RepoAccountEvent` oqimini o'z ichiga aks ettiring
  SIEM, shuning uchun har qanday soxta ko'rsatma aniqlanishi mumkin (teng imzolari etishmayotgan).
- **Orqaga qaytarish:** Agar repo muddatidan oldin ochish kerak bo'lsa, `repo unwind` ni yuboring
  bir xil kelishuv identifikatori bilan va voqeangizda `--notes` maydonini biriktiring
  GAR tomonidan tasdiqlangan qayta tiklash o'yin kitobiga havola qiluvchi treker.
- **Firibgarlikning kuchayishi:** Agar ruxsatsiz repolar paydo bo'lsa, huquqbuzarlikni eksport qiling
  `RepoAccountEvent` foydali yuklar, boshqaruv siyosati orqali hisoblarni muzlatish va
  repo boshqaruvi SOP bo'yicha kengashni xabardor qiling.

## 7. Hisobot va kuzatish

### 7.1 G'aznachilikni solishtirish va daftar dalillariYo'l xaritasi **F1** va global aholi punktlari to'sig'i (roadmap.md#L1975-L1978)
har bir repo tekshiruvi deterministik xazina dalillarini o'z ichiga olishini talab qiladi. Ishlab chiqarish a
Quyidagi nazorat ro'yxatiga rioya qilish orqali har chorakda bir kitob to'plami.

1. **Snapshot balanslari.** quvvat beruvchi `FindAssets` so‘rovidan foydalaning
   `iroha ledger asset list` (`crates/iroha_cli/src/main_shared.rs`) yoki
   `iroha_python` `ih58...` uchun XOR balanslarini eksport qilish uchun yordamchi,
   `ih58...` va ko'rib chiqishda ishtirok etgan har bir stol hisobi. Do'kon
   ostidagi JSON
   `artifacts/finance/repo/<period>/treasury_assets.json` va gitni yozib oling
   hamroh bo'lgan `README.md`-da majburiyat/uskunalar zanjiri.
2. **Kross-tekshirish daftarining prognozlari.** Qayta ishga tushirish
   `sorafs reserve ledger --quote <...> --json-out ...` va chiqishni normallashtiring
   `scripts/telemetry/reserve_ledger_digest.py` orqali. Dijestni yoniga qo'ying
   auditorlar XOR yig'indisini repoga nisbatan farqlashlari uchun aktivning surati
   CLI-ni takrorlamasdan daftar proyeksiyasi.
3. **Yaroqlilik eslatmasini nashr eting.** Deltalarni umumlashtiring
   `artifacts/finance/repo/<period>/treasury_reconciliation.md` havola orqali:
   aktiv snapshot xeshi, ledger dayjest xeshi va qamrab olingan kelishuvlar.
   Mutaxassislar tasdiqlashlari uchun moliya boshqaruvi kuzatuvchisidan eslatmani bog'lang
   repo chiqarishni tasdiqlashdan oldin xazina qoplamasi.

### 7.2 Matkap va orqaga qaytarish mashqlari dalillari

Qabul qilish mezonlari, shuningdek, bosqichma-bosqich orqaga qaytarish va hodisa mashqlarini talab qiladi. Har
burg'ulash yoki tartibsizlik mashqlari quyidagi artefaktlarni to'plashi kerak:

1. Voqea qo'mondoni tomonidan imzolangan `repo_runbook.md` 4-5 bo'limlari nazorat ro'yxati va
   moliya kengashi.
2. Repetitsiya uchun CLI/SDK jurnallari (`repo initiate|margin-call|unwind`) va
   yangilangan hayot aylanishini isbotlovchi oniy tasvir va dalillar manifesti (§§2.7–3.2) saqlangan
   `artifacts/finance/repo/drills/<timestamp>/` ostida.
3. AOK qilingan signallarni ko'rsatuvchi ogohlantirish boshqaruvchisi yoki peyjer transkriptlari
   tan olish izi. Transkriptni burg'ulash artefaktlari yoniga tashlang va
   foydalanilganda Alertmanager sukunat identifikatorini qo'shing.
4. GAR identifikatori, manifest xesh va matkapga havola qiluvchi `ops/drill-log.md` yozuvi
   to'plam yo'li, shuning uchun kelajakdagi auditlar suhbat jurnallarini qirib tashlamasdan mashqlarni kuzatishi mumkin.

### 7.3 Boshqaruv kuzatuvchisi va hujjat gigienasi

- Ushbu hujjat, `repo_runbook.md` va moliya boshqaruvi kuzatuvchisini saqlang
  CLI/SDK yoki ish vaqti xatti-harakati o'zgarganda lockstep; sharhlovchilar kutmoqda
  to'g'ri qolish uchun qabul qilish jadvali.
- To'liq dalillar to'plamini biriktiring (`agreements.json`, bosqichma-bosqich ko'rsatmalar, SSE
  transkriptlar, konfiguratsiya snapshoti, yarashuvlar, matkap artefaktlari va test
  jurnallar) har choraklik tekshiruv uchun trekerga.
- Muvofiqlashtirishda `docs/source/finance/settlement_iso_mapping.md` ma'lumotnomasi
  ISO ko'prik operatorlari bilan tizimlararo yarashuv bir xil bo'lib qoladi.

Ushbu qo'llanmaga rioya qilish orqali operatorlar F1 yo'l xaritasini qabul qilish panelini qondiradilar:
deterministik dalillar qo'lga kiritiladi, uch partiyali va almashtirish oqimlari
hujjatlashtirilgan va boshqaruv tartib-qoidalari (ikki tomonlama nazorat + hodisalarni qayd etish) mavjud
daraxt ichida kodlangan.