---
lang: uz
direction: ltr
source: docs/source/da/commitments_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ea1b16b73a55e3e47dfe9d5bfc77dedce2e8fa9ff964d244856767f14931733
source_last_modified: "2026-01-22T14:45:02.095688+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus Maʼlumotlar mavjudligi boʻyicha majburiyatlar rejasi (DA-3)

_Tuzilgan: 25-03-2026 — Egalari: Asosiy Protokol WG / Smart kontrakt jamoasi / Saqlash jamoasi_

DA-3 Nexus blok formatini kengaytiradi, shuning uchun har bir qator deterministik yozuvlarni joylashtiradi.
DA-2 tomonidan qabul qilingan bloblarni tavsiflash. Ushbu eslatma kanonik ma'lumotlarni qamrab oladi
tuzilmalar, blokli quvur liniyasi ilgaklari, yorug'lik-klient isbotlari va Torii/RPC sirtlari
validatorlar qabul paytida DA majburiyatlariga tayanishidan oldin qo'nishi kerak yoki
boshqaruv tekshiruvlari. Barcha foydali yuklar Norito kodlangan; SCALE yoki ad-hoc JSON yo'q.

## Maqsadlar

- Har bir blob uchun majburiyatlarni bajarish (chunk root + manifest xesh + ixtiyoriy KZG
  majburiyat) har bir Nexus blokida tengdoshlar mavjudligini qayta qurishi mumkin
  hisobdan tashqari saqlash bilan maslahatlashmasdan davlat.
- Engil mijozlar buni tasdiqlashlari uchun deterministik a'zolik dalillarini taqdim eting
  manifest xeshi berilgan blokda yakunlandi.
- Torii so'rovlarini (`/v1/da/commitments/*`) va o'tishga imkon beruvchi dalillarni ko'rsating,
  SDK'lar va boshqaruvni avtomatlashtirish auditining mavjudligi har birini takrorlamasdan
  blok.
- Mavjud `SignedBlockWire` konvertini yangisini o'tkazish orqali kanonik saqlang
  tuzilmalarni Norito metama'lumotlar sarlavhasi va blok xesh hosilasi orqali.

## Qo'llash sohasiga umumiy nuqtai

1. `iroha_data_model::da::commitment` plus blokidagi **Maʼlumotlar modeli qoʻshimchalari**
   `iroha_data_model::block` da sarlavha o'zgarishi.
2. **Ijrochi ilgaklari** shuning uchun `iroha_core` Torii tomonidan chiqarilgan DA kvitantsiyalarini oladi
   (`crates/iroha_core/src/queue.rs` va `crates/iroha_core/src/block.rs`).
3. **Doimiylik/indekslar**, shuning uchun WSV majburiyat so'rovlariga tezda javob berishi mumkin
   (`iroha_core/src/wsv/mod.rs`).
4. **Torii RPC qoʻshimchalari** ostidagi roʻyxat/soʻrov/tasdiqlash yakuniy nuqtalari uchun
   `/v1/da/commitments`.
5. **Integratsiya testlari + moslamalar** simning joylashuvi va o'tkazuvchanligini tasdiqlovchi
   `integration_tests/tests/da/commitments.rs`.

## 1. Ma'lumotlar modeli qo'shimchalari

### 1.1 `DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // lane policy (merkle_sha256 or kzg_bls12_381)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub kzg_commitment: Option<KzgCommitment>,
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- `KzgCommitment` ostida ishlatiladigan mavjud 48 bayt nuqtadan qayta foydalanadi
  `iroha_crypto::kzg`. Merkle yo'llari uni bo'sh qoldiradi; `kzg_bls12_381` qatorlari hozir
  chunk ildizidan olingan deterministik BLAKE3-XOF majburiyatini olish va
  saqlash chiptasi shuning uchun blok xeshlari tashqi proversiz barqaror qoladi.
- `proof_scheme` chiziqli katalogdan olingan; Merkle yo'llari adashgan KZGni rad etadi
  foydali yuklar, `kzg_bls12_381` qatorlari esa nolga teng bo'lmagan KZG majburiyatlarini talab qiladi.
- `proof_digest` DA-5 PDP/PoTR integratsiyasini kutadi, shuning uchun bir xil rekord
  bloblarni jonli saqlash uchun foydalaniladigan namuna olish jadvalini sanab o'tadi.

### 1.2 Blok sarlavhasi kengaytmasi

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```

Toʻplam xeshi ham blok xeshga, ham `SignedBlockWire` metamaʼlumotlariga taʼminlanadi.
tepalik.

Amalga oshirish bo'yicha eslatma: `BlockPayload` va shaffof `BlockBuilder` endi ochiladi
`da_commitments` sozlagichlar/qabul qiluvchilar (qarang: `BlockBuilder::set_da_commitments` va
`SignedBlock::set_da_commitments`), shuning uchun xostlar oldindan tuzilgan paketni biriktirishi mumkin
blokni muhrlashdan oldin. Barcha yordamchi konstruktorlar maydonni sukut bo'yicha `None` deb belgilaydilar
Torii haqiqiy to'plamlarni o'tkazmaguncha.

### 1.3 Simli kodlash- `SignedBlockWire::canonical_wire()` uchun Norito sarlavhasini qo'shadi
  Mavjud operatsiyalar ro'yxatidan so'ng darhol `DaCommitmentBundle`. The
  versiya bayti `0x01`.
- `SignedBlockWire::decode_wire()` `version` noma'lum bo'lgan to'plamlarni rad etadi,
  `norito.md` da tasvirlangan Norito siyosatiga mos keladi.
- Xesh chiqarish yangilanishlari faqat `block::Hasher` da mavjud; engil mijozlar dekodlash
  mavjud sim formati avtomatik ravishda yangi maydonni oladi, chunki Norito
  sarlavha uning mavjudligini e'lon qiladi.

## 2. Blok ishlab chiqarish oqimi

1. Torii DA qabul qilishda imzolangan kvitansiyalar va majburiyat yozuvlari davom etadi.
   DA g'altak (`da-receipt-*.norito` / `da-commitment-*.norito`). Bardoshli
   qayta ishga tushirilganda kvitansiya jurnali urug'lari kursorlari, shuning uchun takrorlangan kvitansiyalar hali ham buyurtma qilinadi
   deterministik tarzda.
2. Blok yig'ish shpildan tushumlarni yuklaydi, eskirgan/allaqachon muhrlangan tomchilar
   kiritilgan kursor snapshotidan foydalangan holda yozuvlarni kiritadi va kontiguity per
   `(lane, epoch)`. Agar erishish mumkin bo'lgan kvitansiyada tegishli majburiyat bo'lmasa yoki
   manifest xesh taklifni jimgina o'tkazib yuborish o'rniga uni bekor qiladi.
3. To'g'ridan-to'g'ri muhrlashdan oldin, quruvchi majburiyat to'plamini bo'laklarga ajratadi
   kvitansiyaga asoslangan to'plam, `(lane_id, epoch, sequence)` bo'yicha saralanadi, kodlaydi
   Norito kodek bilan to'plam va yangilanishlar `da_commitments_hash`.
4. To'liq to'plam WSVda saqlanadi va ichidagi blok bilan birga chiqariladi
   `SignedBlockWire`; Qabul qilingan to'plamlar kvitansiya kursorlarini oldinga siljitadi (gidratlangan
   qayta ishga tushirilganda Kura'dan) va disk o'sishini bog'lash uchun eskirgan g'altakning yozuvlarini kesib tashlang.

Blokni yig'ish va `BlockCreated` qabul qilish har bir majburiyatni qayta tasdiqlaydi
yo'laklar katalogi: Merkle yo'llari adashgan KZG majburiyatlarini rad etadi, KZG yo'llari
nolga teng bo'lmagan KZG majburiyati va nolga teng bo'lmagan `chunk_root` va noma'lum yo'llar
tushib ketdi. Torii ning `/v1/da/commitments/verify` so'nggi nuqtasi bir xil himoyani aks ettiradi,
va ingest endi har biriga deterministik KZG majburiyatini kiritadi
`kzg_bls12_381` yozuvi siyosatga mos keladigan toʻplamlar blok yigʻilishiga yetib boradi.

DA-2 qabul qilish rejasida tasvirlangan manifest moslamalari manba sifatida ikki baravar ko'payadi
majburiyat to'plami uchun haqiqat. Torii testi
`manifest_fixtures_cover_all_blob_classes` har bir uchun manifestlarni qayta tiklaydi
`BlobClass` varianti va yangi sinflar moslamalar olguncha kompilyatsiya qilishni rad etadi,
har bir `DaCommitmentRecord` ichidagi kodlangan manifest xesh bilan mos kelishini ta'minlash
oltin Norito/JSON juftligi.【crates/iroha_torii/src/da/tests.rs:2902】

Agar blokni yaratish muvaffaqiyatsiz tugasa, kvitansiyalar navbatdagi blokda qoladi
urinish ularni olishi mumkin; quruvchi oxirgi kiritilgan `sequence` boshiga qayd
takroriy hujumlardan qochish uchun chiziq.

## 3. RPC va so'rovlar yuzasi

Torii uchta so'nggi nuqtani ochib beradi:| Marshrut | Usul | Yuk yuk | Eslatmalar |
|-------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (yo'l/davr/ketma-ketlik bo'yicha diapazon filtri, sahifalash) | `DaCommitmentPage` ni umumiy hisob, majburiyatlar va blok xesh bilan qaytaradi. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (bo'lak + manifest xesh yoki `(epoch, sequence)` korteji). | `DaCommitmentProof` bilan javob beradi (yozuv + Merkle yo'li + blok xeshi). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Blok xesh hisobini takrorlaydigan va kiritishni tasdiqlovchi fuqaroligi bo'lmagan yordamchi; to'g'ridan-to'g'ri `iroha_crypto` ga ulana olmaydigan SDKlar tomonidan foydalaniladi. |

Barcha foydali yuklar `iroha_data_model::da::commitment` ostida ishlaydi. Torii marshrutizatorlari o'rnatiladi
mavjud DA yonidagi ishlov beruvchilar token/mTLSni qayta ishlatish uchun oxirgi nuqtalarni qabul qiladi
siyosatlar.

## 4. Inclusion Proofs & Light Clients

- Blok ishlab chiqaruvchisi seriallashtirilgan ustiga ikkilik Merkle daraxtini quradi
  `DaCommitmentRecord` ro'yxati. Ildiz `da_commitments_hash` ni oziqlantiradi.
- `DaCommitmentProof` maqsadli yozuvni va `(sibling_hash,) vektorini paketlaydi.
  position)` yozuvlari, shuning uchun tekshiruvchilar ildizni qayta qurishlari mumkin. Isbotlar ham o'z ichiga oladi
  blok xesh va imzolangan sarlavha, shuning uchun engil mijozlar yakuniyligini tekshirishlari mumkin.
- CLI yordamchilari (`iroha_cli app da prove-commitment`) isbot so'rovini o'rab oladi/tekshiradi
  operatorlar uchun tsikl va sirt Norito/hex chiqishi.

## 5. Saqlash va indekslash

WSV majburiyatlarni `manifest_hash` tomonidan kalitlangan maxsus ustunlar oilasida saqlaydi.
Ikkilamchi indekslar `(lane_id, epoch)` va `(lane_id, sequence)` so'rovlarini qamrab oladi
to'liq to'plamlarni skanerlashdan saqlaning. Har bir yozuv uni muhrlagan blok balandligini kuzatadi,
ushlash tugunlariga blok jurnalidan indeksni tezda qayta tiklashga imkon beradi.

## 6. Telemetriya va kuzatuvchanlik

- `torii_da_commitments_total` har safar blok kamida bittasini muhrlaganda oshadi
  rekord.
- `torii_da_commitment_queue_depth` paketlanishi kutilayotgan kvitansiyalarni kuzatib boradi
  bo'lak).
- Grafana asboblar paneli `dashboards/grafana/da_commitments.json` blokni ingl.
  inklyuziya, navbat chuqurligi va isbot o'tkazuvchanligi, shuning uchun DA-3 chiqarish eshiklari tekshirilishi mumkin
  xatti-harakati.

## 7. Sinov strategiyasi

1. `DaCommitmentBundle` kodlash/dekodlash va blok xesh uchun **birlik sinovlari**
   hosila yangilanishlari.
2. `fixtures/da/commitments/` ostida **Oltin armatura** kanonik suratga olish
   to'plam baytlari va Merkle dalillari. Har bir to'plam manifest baytlariga havola qiladi
   `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` dan, shuning uchun
   qayta tiklash `cargo test -p iroha_torii regenerate_da_ingest_fixtures -- --ignored --nocapture`
   `ci/check_da_commitments.sh` majburiyatni yangilagunga qadar Norito hikoyasini izchil saqlaydi
   dalillar.【Fixtures/da/ingest/README.md:1】
3. **Integratsiya testlari** ikkita validatorni yuklash, namunaviy bloblarni qabul qilish va
   ikkala tugun ham to'plam mazmuni va so'rov/dalil bo'yicha kelishib olishini ta'kidlaydi
   javoblar.
4. `integration_tests/tests/da/commitments.rs` da **Light-mijoz testlari**
   (Rust) `/prove`ga qo'ng'iroq qiladi va Torii bilan gaplashmasdan dalilni tasdiqlaydi.
5. Operatorni ushlab turish uchun **CLI smoke** skripti `scripts/da/check_commitments.sh`
   asboblarni takrorlash mumkin.

## 8. Ishlab chiqarish rejasi| Bosqich | Tavsif | Chiqish mezonlari |
|-------|-------------|---------------|
| P0 - Ma'lumotlar modelini birlashtirish | Land `DaCommitmentRecord`, blok sarlavhalari yangilanishlari va Norito kodeklari. | `cargo test -p iroha_data_model` yashil, yangi jihozlar bilan. |
| P1 - Yadro/WSV simlari | Mavzu navbati + blok quruvchi mantiq, doimiy indekslar va RPC ishlov beruvchilarini ochish. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` to'plamni isbotlovchi tasdiqlar bilan o'tadi. |
| P2 — Operator asboblari | CLI yordamchilarini, Grafana asboblar panelini va tasdiqlovchi hujjat yangilanishlarini yuboring. | `iroha_cli app da prove-commitment` devnetga qarshi ishlaydi; asboblar paneli jonli ma'lumotlarni ko'rsatadi. |
| P3 - Boshqaruv darvozasi | `iroha_config::nexus` da belgilangan qatorlarda DA majburiyatlarini talab qiluvchi blok validatorni yoqing. | Status kiritish + yo‘l xaritasini yangilash DA-3 ni 🈴 sifatida belgilang. |

## Ochiq savollar

1. **KZG va Merkle defoltlari** — Kichik bloklar har doim KZG majburiyatlarini o'tkazib yuborishi kerakmi?
   blok hajmini kamaytirasizmi? Taklif: `kzg_commitment` ni ixtiyoriy va darvoza orqali saqlang
   `iroha_config::da.enable_kzg`.
2. **Tartibli bo‘shliqlar** — Biz tartibsiz bo‘laklarga ruxsat beramizmi? Joriy reja bo'shliqlarni rad etadi
   boshqaruv favqulodda takrorlash uchun `allow_sequence_skips` ni almashtirmasa.
3. **Light-mijoz keshi** — SDK jamoasi yengil SQLite keshini talab qildi.
   dalillar; DA-8 bo'yicha kuzatuv kutilmoqda.

PRlarni amalga oshirishda ularga javob berish DA-3 ni 🈸 (ushbu hujjat) dan 🈺 ga o'tkazadi.
kod ishi boshlangandan keyin.