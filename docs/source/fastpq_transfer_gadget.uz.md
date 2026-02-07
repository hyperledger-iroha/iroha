---
lang: uz
direction: ltr
source: docs/source/fastpq_transfer_gadget.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 084add6296c5b884a6d6dc07425aeca9966576f0643f6a7cf555da3fc8586466
source_last_modified: "2026-01-08T12:24:34.985909+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% FastPQ Transfer Gadget Dizayni

# Umumiy ko'rinish

Joriy FASTPQ rejalashtiruvchisi `TransferAsset` ko'rsatmasi bilan bog'liq har bir ibtidoiy operatsiyani qayd qiladi, ya'ni har bir transfer balans arifmetikasi, xesh-raundlar va SMT yangilanishlari uchun alohida to'lanadi. Har bir o'tkazmada kuzatuv qatorlarini kamaytirish uchun biz xost kanonik holatga o'tishni davom ettirayotganda faqat minimal arifmetik/majburiyat tekshiruvlarini tekshiradigan maxsus gadjetni taqdim etamiz.

- **Qoʻllanish doirasi**: mavjud Kotodama/IVM `TransferAsset` tizimli qoʻngʻiroq yuzasi orqali chiqariladigan yagona oʻtkazmalar va kichik partiyalar.
- **Maqsad**: qidiruv jadvallarini almashish va har bir transfer arifmetikasini ixcham cheklov blokiga yig‘ish orqali katta hajmli o‘tkazmalar uchun FFT/LDE ustun izini qisqartirish.

#Arxitektura

```
Kotodama builder → IVM syscall (transfer_v1 / transfer_v1_batch)
          │
          ├─ Host (unchanged business logic)
          └─ Transfer transcript (Norito-encoded)
                   │
                   └─ FASTPQ TransferGadget
                        ├─ Balance arithmetic block
                        ├─ Poseidon commitment check
                        ├─ Dual SMT path verifier
                        └─ Authority digest equality
```

## Transkript formati

Xost har bir tizim chaqiruvi uchun `TransferTranscript` chiqaradi:

```rust
struct TransferTranscript {
    batch_hash: Hash,
    deltas: Vec<TransferDeltaTranscript>,
    authority_digest: Hash,
    poseidon_preimage_digest: Option<Hash>,
}

struct TransferDeltaTranscript {
    from_account: AccountId,
    to_account: AccountId,
    asset_definition: AssetDefinitionId,
    amount: Numeric,
    from_balance_before: Numeric,
    from_balance_after: Numeric,
    to_balance_before: Numeric,
    to_balance_after: Numeric,
    from_merkle_proof: Option<Vec<u8>>,
    to_merkle_proof: Option<Vec<u8>>,
}
```

- `batch_hash` takroriy himoya qilish uchun transkriptni tranzaksiya kirish nuqtasi xeshiga bog'laydi.
- `authority_digest` - tartiblangan imzolovchilar/kvorum ma'lumotlari ustidan xostning xeshi; gadjet tenglikni tekshiradi, lekin imzoni tekshirishni takrorlamaydi. Konkret Norito xost `AccountId`-ni kodlaydi (u allaqachon kanonik multisig kontrollerni o'rnatgan) va `b"iroha:fastpq:v1:authority|" || encoded_account`-ni Blake2b-256 bilan xeshlaydi, natijada olingan Kotodama-ni saqlaydi.
- `poseidon_preimage_digest` = Poseidon(|| hisobdan_hisobdan || aktivga || summa || to'plamli_xesh); gadget xost bilan bir xil dayjestni qayta hisoblashini ta'minlaydi. Preimage baytlari umumiy Poseidon2 yordamchisi orqali o'tishdan oldin yalang'och Norito kodlash yordamida `norito(from_account) || norito(to_account) || norito(asset_definition) || norito(amount) || batch_hash` sifatida tuzilgan. Ushbu dayjest bir deltali transkriptlar uchun mavjud va ko'p deltali to'plamlar uchun olib tashlanmaydi.

Barcha maydonlar Norito orqali ketma-ketlashtiriladi, shuning uchun mavjud determinizm kafolatlari saqlanib qoladi.
`from_path` va `to_path` ikkalasi ham Norito bloblari sifatida chiqariladi.
`TransferMerkleProofV1` sxemasi: `{ version: 1, path_bits: Vec<u8>, siblings: Vec<Hash> }`.
Kelgusi versiyalar sxemani kengaytirishi mumkin, prover esa versiya yorlig'ini qo'llaydi
dekodlashdan oldin. `TransitionBatch` metama'lumotlari Norito kodli transkriptni o'z ichiga oladi
`transfer_transcripts` kaliti ostida vektor, shuning uchun prover guvohni dekodlashi mumkin
tarmoqdan tashqari so'rovlarni bajarmasdan. Umumiy kirishlar (`dsid`, `slot`, ildizlar,
`perm_root`, `tx_set_hash`) `FastpqTransitionBatch.public_inputs` da tashiladi,
kirish xesh/transkript hisobini yuritish uchun metama'lumotlarni qoldirib. Xost sanitariya-tesisatiga qadar
lands, prover sintetik ravishda kalit/balans juftliklaridan dalillarni oladi, shuning uchun qatorlar
transkript ixtiyoriy maydonlarni o'tkazib yuborsa ham har doim deterministik SMT yo'lini o'z ichiga oladi.

## Gadjet tartibi

1. **Balans arifmetik bloki**
   - Kirishlar: `from_balance_before`, `amount`, `to_balance_before`.
   - Tekshiruvlar:
     - `from_balance_before >= amount` (umumiy RNS dekompozitsiyasiga ega diapazonli gadjet).
     - `from_balance_after = from_balance_before - amount`.
     - `to_balance_after = to_balance_before + amount`.
   - Barcha uchta tenglamalar bitta qatorli guruhni iste'mol qilish uchun maxsus darvoza ichiga o'rnatilgan.2. **Poseidon majburiyat bloki**
   - `poseidon_preimage_digest` ni boshqa gadjetlarda allaqachon qo‘llanilgan Poseidon qidiruv jadvali yordamida qayta hisoblaydi. Izda har bir transfer Poseidon raundlari yo'q.

3. **Merkle Path Block**
   - Mavjud Kaigi SMT gadjetini "juftlangan yangilash" rejimi bilan kengaytiradi. Ikki barg (yuboruvchi, qabul qiluvchi) birodarlar xeshlari uchun bir xil ustunni bo'lishadi, bu esa takrorlangan qatorlarni kamaytiradi.

4. **Authority Digest Check**
   - Xost tomonidan taqdim etilgan dayjest va guvoh qiymati o'rtasidagi oddiy tenglik cheklovi. Imzolar maxsus gadjetida qoladi.

5. **To‘plamli tsikl**
   - Dasturlar `transfer_asset` quruvchilardan oldin `transfer_v1_batch_begin()` ni va keyin `transfer_v1_batch_end()` ni chaqiradi. Qo'llanish doirasi faol bo'lsa, xost har bir uzatishni buferlaydi va ularni bitta `TransferAssetBatch` sifatida takrorlaydi, Poseidon/SMT kontekstini har bir to'plamda bir marta qayta ishlatadi. Har bir qo'shimcha delta faqat arifmetik va ikkita barg tekshiruvini qo'shadi. Transkript dekoderi endi ko'p deltali partiyalarni qabul qiladi va ularni `TransferGadgetInput::deltas` sifatida ko'rsatadi, shuning uchun rejalashtiruvchi Norito ni qayta o'qimasdan guvohlarni yig'ishi mumkin. Norito foydali yukiga ega bo'lgan shartnomalar (masalan, CLI/SDK) `transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)` raqamiga qo'ng'iroq qilish orqali ko'lamni butunlay o'tkazib yuborishi mumkin, bu esa xostga bitta tizimda to'liq kodlangan to'plamni beradi.

# Xost va prover o'zgarishlari| Qatlam | O'zgarishlar |
|-------|---------|
| `ivm::syscalls` | `transfer_v1_batch_begin` (`0x29`) / `transfer_v1_batch_end` (`0x2A`) qo'shing, shunda dasturlar oraliq ISI0NI0, plus IIS0604 chiqarmasdan bir nechta `transfer_v1` tizim qo'ng'iroqlarini qavslashi mumkin. (`0x2B`) oldindan kodlangan partiyalar uchun. |
| `ivm::host` & testlar | Asosiy/Standart xostlar `transfer_v1` ni qamrov faol bo‘lganda, `SYSCALL_TRANSFER_V1_BATCH_{BEGIN,END,APPLY}` yuzasida va soxta WSV xostlari regressiya testlari deterministik muvozanatni ta’minlashi mumkin bo‘lgan yozuvlarni bajarishdan oldin bufer sifatida ko‘radi. yangilanishlar.【crates/ivm/src/core_host.rs:1001】【crates/ivm/src/host.rs:451】【crates/ivm/src/mock_wsv.rs :3713】【crates/ivm/tests/wsv_host_pointer_tlv.rs:219】【crates/ivm/tests/wsv_host_pointer_tlv.rs:287】
| `iroha_core` | Holatga o'tgandan so'ng `TransferTranscript` chiqaring, `StateBlock::capture_exec_witness` davrida `FastpqTransitionBatch` yozuvlarini aniq `public_inputs` bilan tuzing va FASTPQ prover chizig'ini ishga tushiring, shunda Torii va St. `TransitionBatch` kirishlari. `TransferAssetBatch` ketma-ket oʻtkazmalarni bitta transkriptga guruhlaydi, koʻp deltali toʻplamlar uchun poseidon digestini oʻtkazib yuboradi, shuning uchun gadjet yozuvlar boʻylab deterministik tarzda takrorlanishi mumkin. |
| `fastpq_prover` | `gadgets::transfer` endi rejalashtiruvchi (`crates/fastpq_prover/src/gadgets/transfer.rs`) uchun koʻp deltali transkriptlarni (balans arifmetikasi + Poseidon digesti) va tuzilgan guvohlarni (jumladan, oʻrin tutuvchi bilan bogʻlangan SMT bloblarini) tasdiqlaydi. `trace::build_trace` ushbu transkriptlarni metama'lumotlarning to'plamidan dekodlaydi, `transfer_transcripts` foydali yuki bo'lmagan uzatish partiyalarini rad etadi, tasdiqlangan guvohlarni `Trace::transfer_witnesses` ga biriktiradi va `TracePolynomialData::transfer_plan()` yakuniy reja tuzilmaguncha davom etadi. (`crates/fastpq_prover/src/trace.rs`). Qatorlarni hisoblash regressiya jabduqlari endi `fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) orqali jo'natiladi va 65536 ta to'ldirilgan qatorgacha bo'lgan stsenariylarni qamrab oladi, birlashtirilgan SMT simlari esa TF-3 to'plami-yordamchi bosqichning orqasida qoladi (o'rin tutqichlari o'sha joyni o'zgartirish jadvalini ushlab turadi). |
| Kotodama | `transfer_batch((from,to,asset,amount), …)` yordamchisini `transfer_v1_batch_begin`, ketma-ket `transfer_asset` va `transfer_v1_batch_end` ga tushiradi. Har bir kortej argumenti `(AccountId, AccountId, AssetDefinitionId, int)` shakliga mos kelishi kerak; yagona transferlar mavjud quruvchini saqlab qoladi. |

Kotodama foydalanish misoli:

```text
fn pay(a: AccountId, b: AccountId, asset: AssetDefinitionId, x: int) {
    transfer_batch((a, b, asset, x), (b, a, asset, 1));
}
```

`TransferAssetBatch` individual `Transfer::asset_numeric` qo'ng'iroqlari bilan bir xil ruxsat va arifmetik tekshiruvlarni amalga oshiradi, lekin bitta `TransferTranscript` ichidagi barcha deltalarni yozib oladi. Ko'p deltali transkriptlar delta bo'yicha majburiyatlar keyingi kuzatuvga kelguniga qadar poseydon hazmini yo'q qiladi. Kotodama quruvchisi endi avtomatik ravishda boshlash/tugash tizimi chaqiruvlarini chiqaradi, shuning uchun kontraktlar Norito foydali yuklarini qo'lda kodlamasdan paketli o'tkazmalarni o'rnatishi mumkin.

## Qatorlar soni regressiyasi

`fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) FASTPQ o'tish partiyalarini sozlanishi mumkin bo'lgan selektor hisoblari bilan sintez qiladi va natijada `row_usage` xulosasi haqida hisobot beradi (`total_rows`, har bir tanlovchi soni, ₂ nisbati bilan birga). 65536 qatorli shiftlar uchun ko'rsatkichlarni suratga oling:

```bash
cargo run -p fastpq_prover --bin fastpq_row_bench -- \
  --transfer-rows 65536 \
  --mint-rows 256 \
  --burn-rows 128 \
  --pretty \
  --output fastpq_row_usage_max.json
```Chiqarilgan JSON hozirda sukut bo'yicha `iroha_cli audit witness` chiqaradigan FASTPQ to'plam artefaktlarini aks ettiradi (ularni bostirish uchun `--no-fastpq-batches` dan o'ting), shuning uchun `scripts/fastpq/check_row_usage.py` va CI gatesi sintetik tortishishlarni oldingi o'zgarishlarni rejalashtirishda farq qilishi mumkin.

# Chiqarish rejasi

1. **TF-1 (Transkript sanitariya-tesisat)**: ✅ `StateTransaction::record_transfer_transcripts` endi har bir `TransferAsset`/to‘plam uchun Norito transkriptlarini chiqaradi, `sumeragi::witness::record_fastpq_transcript` ularni global guvohlik tizimida saqlaydi va I1NI80000 `fastpq_batches` operatorlar uchun aniq `public_inputs` va prover chizig'i (agar sizga ingichka bo'lsa, `--no-fastpq-batches` dan foydalaning) chiqish).【crates/iroha_core/src/state.rs:8801】【crates/iroha_core/src/sumeragi/witness.rs:280】【crates/iroha_core/src/fastpq/mod.rs:157】【crates/157】【crates/iro.
2. **TF-2 (Gadjetni amalga oshirish)**: ✅ `gadgets::transfer` endi ko‘p deltali transkriptlarni tasdiqlaydi (balans arifmetikasi + Poseidon digesti), xostlar ularni o‘tkazib yuborganida juftlashtirilgan SMT isbotlarini sintez qiladi, Kotodama va Kotodama orqali tuzilgan guvohlarni fosh qiladi. dalillardan SMT ustunlarini to'ldirishda o'sha guvohlar `Trace::transfer_witnesses` ga. `fastpq_row_bench` 65536 qatorli regressiya jabduqlarini ushlaydi, shuning uchun rejalashtiruvchilar Norito ni takrorlamasdan qatordan foydalanishni kuzatib boradilar. foydali yuklar.【crates/fastpq_prover/src/gadgets/transfer.rs:1】【crates/fastpq_prover/src/trace.rs:1】【crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1】
3. **TF-3 (To‘plam yordamchisi)**: To‘plamli tizim qo‘ng‘irog‘ini + Kotodama quruvchisini, jumladan, xost darajasidagi ketma-ket ilova va gadjet siklini yoqing.
4. **TF-4 (Telemetriya va hujjatlar)**: `fastpq_plan.md`, `fastpq_migration_guide.md` va asboblar paneli sxemalarini boshqa gadjetlarga nisbatan uzatish qatorlarini sirt taqsimlash uchun yangilang.

# Ochiq savollar

- **Domen chegaralari**: joriy FFT rejalashtiruvchisi 2¹⁴ qatordan keyingi izlar uchun vahima. TF-2 domen hajmini oshirishi yoki qisqartirilgan benchmark maqsadini hujjatlashtirishi kerak.
- **Ko‘p aktivli partiyalar**: dastlabki gadjet har delta uchun bir xil aktiv identifikatorini qabul qiladi. Agar bizga heterojen partiyalar kerak bo'lsa, biz o'zaro aktivlar takrorlanishining oldini olish uchun har safar Poseidon guvohiga aktivni kiritishiga ishonch hosil qilishimiz kerak.
- **Authority dayjestidan qayta foydalanish**: har bir tizim qo‘ng‘irog‘i uchun imzolovchilar ro‘yxatini qayta hisoblashning oldini olish uchun biz bir xil dayjestdan boshqa ruxsat berilgan operatsiyalar uchun uzoq muddat foydalanishimiz mumkin.


Ushbu hujjat dizayn qarorlarini kuzatib boradi; marralar qo'nganda uni yo'l xaritasi yozuvlari bilan sinxronlashtiring.