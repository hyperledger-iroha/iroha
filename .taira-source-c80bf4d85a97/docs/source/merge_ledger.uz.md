---
lang: uz
direction: ltr
source: docs/source/merge_ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44f1c681730f1c94d9d00e8f829a0134374ce6cb29f21727a27685e096f0da40
source_last_modified: "2026-01-17T06:10:29.077000+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Ledger dizaynini birlashtirish - yo'lakni yakunlash va global qisqartirish

Bu eslatma Milestone 5 uchun birlashma kitobi dizaynini yakunlaydi. Unda tushuntiriladi
bo'sh bo'lmagan blok siyosati, o'zaro faoliyat QC birlashma semantikasi va yakuniy ish jarayoni
bu chiziq darajasida bajarilishini global jahon davlat majburiyatiga bog'laydi.

Dizayn `nexus.md` da tasvirlangan Nexus arxitekturasini kengaytiradi. kabi shartlar
"Line bloki", "lane QC", "birlashma maslahati" va "birlashma kitobi" ularning merosxo'ri bo'ladi.
ushbu hujjatdagi ta'rif; bu eslatma xulq-atvor qoidalariga qaratilgan va
ish vaqti, saqlash va WSV tomonidan bajarilishi kerak bo'lgan amalga oshirish bo'yicha ko'rsatmalar
qatlamlar.

## 1. Bo'sh bo'lmagan bloklash siyosati

**Qoida (MUST):** Yo‘lakni taklif etuvchi blokni faqat blokda at bo‘lgandagina chiqaradi
kamida bitta bajarilgan tranzaksiya fragmenti, vaqtga asoslangan trigger yoki deterministik
artefaktni yangilash (masalan, DA artefakt toʻplami). Bo'sh bloklar taqiqlangan.

**Ta'siri:**

- Slotni saqlab qolish: hech qanday tranzaktsiya uning deterministik majburiyat oynasiga mos kelmasa,
bo'lak hech qanday blok chiqarmaydi va oddiygina keyingi uyaga o'tadi. Birlashtirish kitobi
bu chiziq uchun oldingi uchida qoladi.
- Trigger to'plami: hech qanday holatga o'tmaydigan fon triggerlari (masalan,
invariantlarni tasdiqlovchi cron) bo'sh deb hisoblanadi va o'tkazib yuborilishi KERAK yoki
blok ishlab chiqarishdan oldin boshqa ishlar bilan birlashtirilgan.
- Telemetriya: `pipeline_detached_merged` va keyingi ko'rsatkichlar o'tkazib yuborildi
Slotlar aniq - operatorlar "ish yo'q" ni "quvur to'xtab qolgan" dan ajrata oladi.
- Takrorlash: blokli saqlash sintetik bo'sh joy egalarini kiritmaydi. Kura
takrorlash sikli oddiygina ketma-ket uyalar uchun bir xil asosiy xeshni kuzatadi, agar yo'q bo'lsa
blok chiqarildi.

**Konik tekshiruv:** Blok taklifi va tekshirish vaqtida `ValidBlock::commit`
bog'langan `StateBlock` kamida bitta majburiy qoplamani o'z ichiga oladi
(delta, artefakt, tetik). Bu `StateBlock::is_empty` himoyasi bilan mos keladi
Bu allaqachon no-op yozuvlari o'chirilishini ta'minlaydi. Amalga oshirish oldin sodir bo'ladi
Imzolar so'raladi, shuning uchun qo'mitalar hech qachon bo'sh yuklarga ovoz bermaydi.

## 2. Cross-Line QC Merge semantikasi

O'z qo'mitasi tomonidan yakunlangan `B_i` har bir qator bloki quyidagilarni ishlab chiqaradi:

- `lane_state_root_i`: Poseidon2-SMT boʻyicha majburiyat har bir DS holati ildizlariga tegdi
blokda.
- `merge_hint_root_i`: birlashtirish daftariga nomzod (`tag =)
"iroha: birlashma: nomzod: v1\0"`).
- `lane_qc_i`: yo'lak qo'mitasining umumiy imzolari
  ijro-ovoz preimage (blok xesh, `parent_state_root`,
  `post_state_root`, balandlik/ko'rinish/davr, zanjir_identifikatori va rejim yorlig'i).

Birlashtirish tugunlari `{(B_i, lane_qc_i, merge_hint_root_i)}` uchun so'nggi maslahatlarni to'playdi
barcha qatorlar `i ∈ [0, K)`.

**Kirishni birlashtirish (MUST):**

```
MergeLedgerEntry {
    epoch_id: u64,
    lane_tips: [Hash32; K],
    merge_hint_root: [Hash32; K],
    global_state_root: Hash32,
    merge_qc: QuorumCertificate,
}
```- `lane_tips[i]` - chiziq blokining xeshi, bo'lak uchun birlashma kirish muhrlari
  `i`. Agar oldingi birlashma yozuvidan keyin chiziq hech qanday blokni chiqarmasa, bu qiymat
  takrorlanadi.
- `merge_hint_root[i]` - mos keladigan chiziqdan `merge_hint_root`
  blok. `lane_tips[i]` takrorlanganda u takrorlanadi.
- `global_state_root` teng `ReduceMergeHints(merge_hint_root[0..K-1])`, a
  Domenni ajratish yorlig'i bilan Poseidon2 katlama
  `"iroha:merge:reduce:v1\0"`. Kamaytirish deterministik va MUST
  tengdoshlar orasida bir xil qiymatni qayta qurish.
- `merge_qc` - bu birlashma qo'mitasining BFT kvorum sertifikati
  seriyali kirish.

**QC yukini birlashtirish (MUST):**

Birlashtirish qo'mitasi a'zolari deterministik dayjestni imzolaydilar:

```
merge_qc_digest = blake2b32(
    "iroha:merge:qc:v1\0" ||
    chain_id ||
    norito(MergeLedgerSignPayload {
        view,
        epoch_id,
        lane_tips,
        merge_hint_roots,
        global_state_root,
    })
)
```

- `view` - chiziqli maslahatlardan (maksimal) olingan birlashma qo'mitasi ko'rinishi
  `view_change_index` yo'lak sarlavhalari bo'ylab kirish bilan muhrlangan).
- `chain_id` - konfiguratsiya qilingan zanjir identifikatori (UTF-8 bayt).
- Foydali yuk yuqorida ko'rsatilgan maydon tartibi bilan Norito kodlashdan foydalanadi.

Olingan dayjest `merge_qc.message_digest` da saqlanadi va xabar hisoblanadi
BLS imzolari bilan tasdiqlangan.

**QC qurilishini birlashtirish (MUST):**

- Birlashtirish qo'mitasining ro'yxati joriy komissiya-topologiya tekshiruvi to'plamidir.
- Kerakli kvorum = `commit_quorum_from_len(roster_len)`.
- `merge_qc.signers_bitmap` ishtirok etuvchi validator indekslarini kodlaydi (LSB-birinchi)
  topshiriq topologiyasi tartibida.
- `merge_qc.aggregate_signature` - bu hazm qilish uchun BLS-normal agregat.
  yuqorida.

**Tasdiqlash (MUST):**

1. Har bir `lane_qc_i` ni `lane_tips[i]` bilan taqqoslang va blok sarlavhalarini tasdiqlang
   mos keladigan `merge_hint_root_i` ni o'z ichiga oladi.
2. `Invalid` yoki bajarilmagan blokga `lane_qc_i` nuqtalari yo'qligiga ishonch hosil qiling. The
   yuqoridagi boʻsh boʻlmagan siyosat sarlavhada holat qoplamalarini oʻz ichiga oladi.
3. `ReduceMergeHints` ni qayta hisoblang va `global_state_root` bilan solishtiring.
4. Birlashtirish QC dayjestini qayta hisoblang va imzolovchining bitmapini, kvorum chegarasini tekshiring,
   va commit-topologiya ro'yxatiga qarshi umumiy imzo.

**Kuzatilishi:** Birlashtirish tugunlari Prometheus hisoblagichlarini chiqaradi
`merge_entry_lane_repeats_total{i}` uchun slotlarni o'tkazib yuborgan qatorlarni ajratib ko'rsatish
operatsion ko'rinish.

## 3. Yakuniy ish jarayoni

### 3.1 Yo'lak darajasidagi yakuniylik

1. Harakatlar deterministik uyalarda har bir chiziq bo'ylab rejalashtirilgan.
2. Ijrochi `StateBlock` ichiga qoplamalarni qo'llaydi, deltalar va
artefaktlar.
3. Tasdiqlangandan so'ng, yo'l qo'mitasi ijro etuvchi ovoz berish haqidagi preimageni imzolaydi.
   blok xeshini, holat ildizlarini va balandlik/ko'rinish/davrni bog'laydi. Tuple
   `(block_hash, lane_qc_i, merge_hint_root_i)` chiziqli final hisoblanadi.
4. Engil mijozlar DS-cheklangan isbotlar uchun yakuniy sifatida chiziq uchi muomala MAYOT, lekin
birlashma kitobi bilan kelishish uchun tegishli `merge_hint_root` ni yozishi kerak
keyinroq.Yo'lak qo'mitalari har bir ma'lumot maydoni bo'lib, global majburiyatni almashtirmaydi
topologiya. Qo'mita hajmi `3f+1` da belgilanadi, bu erda `f`
ma'lumotlar maydoni katalogi (`fault_tolerance`). Validator puli ma'lumotlar maydonidir
validatorlar (administrator tomonidan boshqariladigan yoʻlaklar yoki ommaviy yoʻlaklar uchun yoʻlaklarni boshqarish manifestlari)
stake-tanlangan yo'llar uchun staking rekordlari). Komissiya a'zosi
bilan bog'langan VRF epoch urug'idan foydalangan holda, har bir davr uchun bir marta aniqlangan
`dataspace_id` va `lane_id`. Hovuz `3f+1` dan kichikroq bo'lsa, chiziqning yakuniyligi
kvorum tiklanmaguncha pauza qiladi (favqulodda tiklash alohida ko'rib chiqiladi).

### 3.2 Birlashtirish daftarining yakuniyligi

1. Birlashtirish qo'mitasi eng so'nggi qator maslahatlarini to'playdi, har bir `lane_qc_i` ni tekshiradi va
yuqorida ta'riflanganidek `MergeLedgerEntry` ni quradi.
2. Deterministik qisqartirishni tekshirgandan so'ng, birlashma qo'mitasi imzolaydi
kirish (`merge_qc`).
3. Tugunlar birlashma daftarlari jurnaliga yozuvni qo'shib, uni bilan birga saqlaydi
qator bloklari havolalari.
4. `global_state_root` jahon davlatining nufuzli majburiyatiga aylanadi.
davr/slot. Toʻliq tugunlar buni aks ettirish uchun WSV nazorat nuqtasi metamaʼlumotlarini yangilaydi
qiymat; deterministik takrorlash bir xil qisqartirishni takrorlashi kerak.

### 3.3 WSV va saqlash integratsiyasi

- `State::commit_merge_entry` har bir chiziqli holat ildizlarini va
  yakuniy `global_state_root`, global nazorat summasi bilan chiziq bajarilishini birlashtiradi.
- Kura `MergeLedgerEntry` chiziqli bloklar artefaktlari yonida saqlanib qoladi, shuning uchun
  Qayta o'ynash ham chiziqli, ham global yakuniy ketma-ketliklarni qayta qurishi mumkin.
- Agar chiziq tirqishni o'tkazib yuborsa, saqlash avvalgi uchini saqlab qoladi; yo'q
  to'ldiruvchini birlashtirish yozuvlari kamida bitta chiziq yangisini yaratmaguncha yaratiladi
  blok.
- API sirtlari (Torii, telemetriya) ikkala chiziq uchlarini ham, so'nggi birlashishni ham ko'rsatadi.
  operatorlar va mijozlar har bir tarmoqli va global ko'rinishlarni moslashtirishi uchun kirish.

## 4. Amalga oshirish bo'yicha eslatmalar- `crates/iroha_core/src/state.rs`: `State::commit_merge_entry` tasdiqlaydi
  qisqartirish va tarmoqli/global metama'lumotlarni dunyo holatiga o'tkazish, shuning uchun so'rovlar
  va kuzatuvchilar birlashma maslahatlari va nufuzli global xeshga kirishlari mumkin.
- `crates/iroha_core/src/kura.rs`: `Kura::store_block_with_merge_entry` navbatlar
  bloklaydi va bir qadamda birlashma kiritishni davom ettiradi, orqaga qaytaradi
  qo'shimcha ishlamay qolganda xotira bloki, shuning uchun saqlash hech qachon blokni yozmaydi
  uning muhrlangan metama'lumotlarisiz. Birlashtirish daftarlari jurnali qulflash bosqichida kesiladi
  ishga tushirishni tiklash paytida tasdiqlangan blok balandligi bilan va xotirada keshlangan
  cheklangan oyna bilan (`kura.merge_ledger_cache_capacity`, standart 256)
  uzoq davom etadigan tugunlarda cheksiz o'sishdan saqlaning. Qayta tiklash qisman yoki qisqaradi
  katta o'lchamli birlashma daftarining quyruq yozuvlari va ilova yuqoridagi yozuvlarni rad etadi
  maksimal yuk hajmini chegara ajratish uchun himoya.
- `crates/iroha_core/src/block.rs`: blok tekshiruvi bloksiz bloklarni rad etadi
  kirish nuqtalari (tashqi operatsiyalar yoki vaqt triggerlari) va deterministiksiz
  DA to'plamlari (`BlockValidationError::EmptyBlock`) kabi artefaktlar,
  bo'sh bo'lmagan siyosat imzolarni so'rash va olib borishdan oldin amalga oshiriladi
  birlashma kitobiga.
- Deterministik qisqartirish yordamchisi birlashma xizmatida yashaydi: `reduce_merge_hint_roots`
  (`crates/iroha_core/src/merge.rs`) yuqorida tavsiflangan Poseidon2 katlamini amalga oshiradi.
  Uskunani tezlashtirish ilgaklari kelajakdagi ish bo'lib qolmoqda, ammo skaler yo'l endi amal qiladi
  kanonik qisqarish deterministik tarzda.
- Telemetriya integratsiyasi: har bir qatorda birlashma takrorlash va
  `global_state_root` o'lchagichi kuzatuvlar ro'yxatida kuzatilib qoladi, shuning uchun
  asboblar panelidagi ish birlashma xizmatini ishga tushirish bilan birga yuborilishi mumkin.
- Komponentlar o'rtasidagi testlar: birlashishni qisqartirish uchun oltin takrorlash qamrovi
  kelajakdagi o'zgarishlarni ta'minlash uchun integratsiya sinovlari to'plami bilan kuzatiladi
  `reduce_merge_hint_roots` qayd qilingan ildizlarni barqaror saqlaydi.