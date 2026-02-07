---
lang: uz
direction: ltr
source: docs/source/governance_vote_tally.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ebff8477d06e2aac8840988d31762704d05ded353d3f900a87db3ea5091e718
source_last_modified: "2026-01-04T08:19:26.508527+00:00"
translation_last_reviewed: 2026-02-07
title: Governance ZK Vote Tally
translator: machine-google-reviewed
---

## Umumiy ko'rinish

Iroha boshqaruv oqimi Halo2/IPA sxemasiga tayanadi, u biroz ovoz berish majburiyatini va uning tegishli saylovchilar to'plamiga a'zoligini tasdiqlaydi. Ushbu eslatma kontaktlarning zanglashiga olib keladigan parametrlari, umumiy ma'lumotlar va tekshirish moslamalarini qamrab oladi, shuning uchun sharhlovchilar testlarda ishlatiladigan tasdiqlash kaliti va dalillarni qayta tiklashlari mumkin.

## Sxema xulosasi

- **Sxema identifikatori**: `halo2/pasta/vote-bool-commit-merkle8-v1`
- **Amalga kirish**: `VoteBoolCommitMerkle::<8>`, `iroha_core::zk::depth`
- **Domen hajmi**: `k = 6`
- **Backend**: Makaron ustidan shaffof Halo2/IPA (ZK1 konvert: VK uchun `IPAK` + `H2VK`, isbotlar uchun `PROF` + `I10P`)
- ** Guvoh shakli**:
  - `v ∈ {0,1}` byulleten biti
  - tasodifiylik skalyar `ρ`
  - Merkle yo'li uchun sakkiz aka-uka skalyar
  - yo'nalish bitlari (mos yozuvlar guvohlarida barcha nol)
- **Merkle kompressor**: `H(x, y) = 2·(x + 7)^5 + 3·(y + 13)^5 (mod p)` bu erda `p` makaron skalyar moduli
- **Ommaviy ma'lumotlar**:
  - ustun 0: `commit`
  - 1-ustun: Merkle ildizi
  - `I10P` TLV (`cols = 2`, `rows = 1`) orqali ochiq

### O'chirish sxemasi

- **Maslahat ustunlari**:
  - `v` – mantiqiy bo'lishi cheklangan byulleten biti.
  - `ρ` – ovoz berish majburiyatida foydalaniladigan ko'r skalar.
  - `i ∈ [0, 7]` uchun `sibling[i]` - `i` chuqurlikdagi Merkle yo'l elementi.
  - `i ∈ [0, 7]` uchun `dir[i]` - yo'nalish bitini tanlash chap (`0`) yoki o'ng (`1`) filiali.
  - `node[i]` uchun `i ∈ [0, 7]` - `i` chuqurlikdan keyin Merkle akkumulyatori.
- **Masalan ustunlari**:
  - `commit` - saylovchi tomonidan e'lon qilingan ommaviy majburiyat.
  - `root` - Saylov huquqiga ega bo'lganlar to'plamining Merkle ildizi.
- **Selektor**: `s_vote` bitta to'ldirilgan qatordagi eshikni yoqadi.

Barcha maslahat xujayralari mintaqaning birinchi (va yagona) qatoriga tayinlangan; sxema `SimpleFloorPlanner` dan foydalanadi.

### Darvoza tizimi

`H` yuqorida tavsiflangan kompressor va `prev_0 = H(v, ρ)` bo'lsin. Darvoza quyidagilarni ta'minlaydi:

1. `s_vote · v · (v - 1) = 0` – mantiqiy byulleten biti.
2. `s_vote · (H(v, ρ) - commit) = 0` - majburiyatning izchilligi.
3. Har bir chuqurlik uchun `i`:
   - `s_vote · dir[i] · (dir[i] - 1) = 0` - mantiqiy yo'l yo'nalishi.
   - `left = H(prev_i, sibling[i])`
   - `right = H(sibling[i], prev_i)`
   - `expected = (1 - dir[i]) · left + dir[i] · right`
   - `s_vote · (node[i] - expected) = 0`
   - `prev_{i+1} = node[i]`
4. `s_vote · (prev_8 - root) = 0` - akkumulyator umumiy Merkle ildiziga teng.

Kompressor faqat kvintik shakllardan foydalanadi; Qidiruv jadvallari talab qilinmaydi. Barcha arifmetika Makaron skalyar maydonida amalga oshiriladi va `k = 6` qatorlari `2^k = 64` qatorlarini ajratadi - faqat nol qatori to'ldiriladi.

### Kanonik moslama

Deterministik jabduqlar (`zk_testkit::vote_merkle8_bundle`) guvohni quyidagilar bilan to'ldiradi:

- `v = 1`
- `ρ = 12345`
- `sibling[i] = 10 + i` `i ∈ [0, 7]` uchun
- `dir[i] = 0`
- `node[i] = H(node[i-1], sibling[i])`, `node[-1] = H(v, ρ)` bilan

Bu umumiy qadriyatlarni ishlab chiqaradi:

```text
commit = 0x20574662a58708e02e0000000000000000000000000000000000000000000000
root   = 0xb63752ff429362c3a9b3cd5966c23567fdb757ce3b38af724b9303a5ea2f5817
```

Tekshirish kalitlari reestrida qayd etilgan `public_inputs_schema_hash` `blake2b-256(commit_bytes || root_bytes)` boʻlib, eng kam ahamiyatli bit `1` ga majburlanadi, natijada:

```text
public_inputs_schema_hash = 0xfae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3
```

### Kalit yozuvi tekshirilmoqda

Boshqaruv tekshiruvchini quyidagi hollarda ro'yxatdan o'tkazadi:- `backend = "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1"`
- `circuit_id = "halo2/pasta/vote-bool-commit-merkle8-v1"`
- `backend tag = BackendTag::Halo2IpaPasta`
- `curve = "pallas"`
- `public_inputs_schema_hash = 0xfae4…64d3`
- `commitment = sha256(backend || vk_bytes)` (32 baytli dayjest)

Kanonik to'plamga tasdiqlash konverti bilan birga ichki tasdiqlash kaliti (`key = Some(VerifyingKeyBox { … })`) kiradi. `vk_len`, `max_proof_bytes` va ixtiyoriy metama'lumotlar URI'lari yaratilgan artefaktlardan to'ldiriladi.

## Malumot moslamalari

Integratsiya testlari (sukut bo'yicha `fixtures/zk/vote_tally/` da chiqadi) tomonidan iste'mol qilingan inline tasdiqlash kaliti va isbot to'plamini qayta tiklash uchun `cargo xtask zk-vote-tally-bundle --print-hashes` dan foydalaning. Buyruq qisqacha xulosani (`backend`, `commit`, `root`, sxema xeshini, uzunliklar) va ixtiyoriy ravishda fayl xeshlarini chop etadi, shunda auditorlar attestatsiya qaydlarini yozib olishlari mumkin. JSON bilan bir xil ma'lumotlarni chiqarish uchun `--summary-json -` dan o'ting (yoki uni diskka yozish uchun yo'lni taqdim eting). `--attestation attestation.json` (yoki stdout uchun `-`) kodini oʻtkazing, unda xulosa va Blake2b-256 dayjestlari va har bir toʻplam artefaktining oʻlchamlarini oʻz ichiga olgan Norito JSON manifestini yozish uchun sertifikat paketlarini tuzatish bilan arxivlash mumkin. `--verify` bilan ishlaganda, `--attestation <path>` taqdim etilishi manifest toʻplamining metamaʼlumotlari va artefakt uzunligi yangi tiklangan toʻplamga mos kelishini tekshiradi (u transkript tasodifiyligi bilan oʻzgarib turadigan har bir ishga tushirish isboti dayjestini solishtirmaydi).

Kanonik moslamalarni qayta tiklang va ko'rsating:

```bash
cargo xtask zk-vote-tally-bundle \
  --out fixtures/zk/vote_tally \
  --print-hashes \
  --attestation fixtures/zk/vote_tally/bundle.attestation.json
```

Tekshirilgan artefaktlarning dolzarbligini tekshiring (fiksator katalogida asosiy toʻplam boʻlishini talab qiladi):

```bash
cargo xtask zk-vote-tally-bundle \
  --out fixtures/zk/vote_tally \
  --verify \
  --attestation fixtures/zk/vote_tally/bundle.attestation.json
```

Manifestga misol:

```jsonc
{
  "generated_unix_ms": 3513801751697071715,
  "hash_algorithm": "blake2b-256",
  "bundle": {
    "backend": "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1",
    "circuit_id": "halo2/pasta/vote-bool-commit-merkle8-v1",
    "commit_hex": "20574662a58708e02e0000000000000000000000000000000000000000000000",
    "root_hex": "b63752ff429362c3a9b3cd5966c23567fdb757ce3b38af724b9303a5ea2f5817",
    "public_inputs_schema_hash_hex": "fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3",
    "vk_commitment_hex": "6f4749f5f75fee2a40880d4798123033b2b8036284225bad106b04daca5fb10e",
    "vk_len": 66,
    "proof_len": 2748
  },
  "artifacts": [
    {
      "file": "vote_tally_meta.json",
      "len": 522,
      "blake2b_256": "5d0030856f189033e5106415d885fbb2e10c96a49c6115becbbff8b7fd992b77"
    },
    {
      "file": "vote_tally_proof.zk1",
      "len": 2748,
      "blake2b_256": "01449c0599f9bdef81d45f3be21a514984357a0aa2d7fcf3a6d48be6307010bb"
    },
    {
      "file": "vote_tally_vk.zk1",
      "len": 66,
      "blake2b_256": "2fd5859365f1d9576c5d6836694def7f63149e885c58e72f5c4dff34e5005d6b"
    }
  ]
}
```

Joriy manifestni kanonik artefaktlaringiz yonida saqlang (masalan, `fixtures/zk/vote_tally/bundle.attestation.json`). Yuqori oqim ombori katta ikkilik to'plamlarni amalga oshirmaslik uchun ushbu katalogni bo'sh saqlaydi, shuning uchun `--verify` ga tayanishdan oldin uni mahalliy sifatida eking.

`generated_unix_ms` majburiyat/tekshirish kaliti barmoq izidan aniq olingan, shuning uchun u regeneratsiyalar davomida barqaror qoladi. Generator sobit ChaCha20 transkripsiyasidan foydalanadi, shuning uchun metama'lumotlar, tasdiqlash kaliti va tasdiqlovchi konvert xeshlari takrorlanishi mumkin. Har qanday hazm qilish mos kelmasligi endi tekshirilishi kerak bo'lgan driftni ko'rsatadi. Auditorlar chiqarilgan qiymatlarni o'zlari tasdiqlagan artefaktlar bilan birga yozib olishlari kerak.

Ish jarayoni haqida eslatma:

1. To'plamni mahalliy sifatida ekish uchun `cargo xtask zk-vote-tally-bundle --out fixtures/zk/vote_tally --print-hashes --attestation fixtures/zk/vote_tally/bundle.attestation.json` ni ishga tushiring.
2. Olingan artefaktlarni kerak bo'lganda topshiring yoki arxivlang.
3. Attestatsiya kanonik to'plamga mos kelishini ta'minlash uchun keyingi regeneratsiyalarda `--verify` dan foydalaning.

Ichki vazifa `xtask/src/vote_tally.rs` da deterministik generatorni ishga tushiradi, bu:

1. Guvohlar namunalari (`v = 1`, `ρ = 12345`, aka-uka `10..17`)
2. `keygen_vk`/`keygen_pk` ishlaydi
3. Halo2 isbotini ishlab chiqaradi va uni ZK1 konvertiga o'radi (jumladan, ommaviy misollar)
4. Tegishli `public_inputs_schema_hash` bilan tasdiqlovchi kalit yozuvini chiqaradi

## Soxtalashtirish

`crates/iroha_core/tests/zk_vote_tally_audit.rs` to'plamni yuklaydi va tekshiradi:

- Haqiqiy dalil to'plamdagi VK-ga qarshi tekshiriladi.
- Majburiyat ustunidagi har qanday baytni o'girish tekshirishning muvaffaqiyatsiz tugashiga olib keladi.
- Ildiz ustunidagi har qanday baytni o'girish tekshirishning muvaffaqiyatsiz bo'lishiga olib keladi.Ushbu regressiya testlari Torii (va xostlar) isbot ishlab chiqarilgandan so'ng ommaviy kirishlari o'zgartirilgan konvertlarni rad etishini kafolatlaydi.

Lokal tarzda regressiyani bajaring:

```bash
cargo test -p iroha_core zk_vote_tally_audit -- --nocapture
```

## Audit tekshiruvi ro'yxati

1. Cheklovning to'liqligi va doimiy tanlash uchun `VoteBoolCommitMerkle::<8>` ni ko'rib chiqing.
2. `cargo xtask zk-vote-tally-bundle --verify --print-hashes`-ni VK/proof-ni qayta ishlab chiqarish va yozilgan xeshlarni tasdiqlash uchun qayta ishga tushiring.
3. Torii hisoblagichi bir xil backend identifikatori va konvert tartibidan foydalanishini tasdiqlang.
4. Mutatsiyaga uchragan dalillar tekshirilmasligiga ishonch hosil qilish uchun o'zgartirish regressiyasini bajaring.
5. `bundle.attestation.json` (Blake2b-256) chiqishini xeshlang va g'iybat qiling, shunda sharhlovchilar o'zlarining attestatsiyalari bilan birga kanonik manifestni ham qayd etishlari mumkin.