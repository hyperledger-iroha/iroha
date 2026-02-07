---
lang: mn
direction: ltr
source: docs/source/fastpq_transfer_gadget.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 084add6296c5b884a6d6dc07425aeca9966576f0643f6a7cf555da3fc8586466
source_last_modified: "2026-01-08T12:24:34.985909+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% FastPQ дамжуулах гаджет дизайн

# Тойм

Одоогийн FASTPQ төлөвлөгч нь `TransferAsset` зааварт хамаарах энгийн үйлдэл бүрийг бүртгэдэг бөгөөд энэ нь шилжүүлэг бүр балансын арифметик, хэш round болон SMT шинэчлэлтийг тусад нь төлдөг гэсэн үг юм. Шилжүүлгийн мөрийг багасгахын тулд бид хост нь каноник төлөвийн шилжилтийг үргэлжлүүлэн гүйцэтгэж байх үед зөвхөн хамгийн бага арифметик/үүргийн шалгалтыг баталгаажуулдаг тусгай хэрэгсэлийг нэвтрүүлж байна.

- **Хамрах хүрээ**: одоо байгаа Kotodama/IVM `TransferAsset` системийн дуудлагын гадаргуугаар ялгардаг нэг дамжуулалт ба жижиг багцууд.
- **Зорилго**: Хайлтын хүснэгтүүдийг хуваалцаж, шилжүүлгийн арифметикийг авсаархан хязгаарлалтын блок болгон буулгах замаар их хэмжээний шилжүүлгийн FFT/LDE баганын ул мөрийг багасгах.

# Архитектур

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

## Сийрүүлгийн формат

Хост нь системийн дуудлагын дуудлага бүрт `TransferTranscript` ялгаруулдаг:

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

- `batch_hash` хуулбарыг гүйлгээний нэвтрэх цэгийн хэштэй холбож, дахин тоглуулахаас хамгаална.
- `authority_digest` нь эрэмбэлэгдсэн гарын үсэг зурсан хүмүүс/чуулгын өгөгдөл дээрх хостын хэш юм; гаджет тэгш байдлыг шалгадаг боловч гарын үсгийн баталгаажуулалтыг дахин хийдэггүй. Товчхондоо Norito хост нь `AccountId`-ийг (энэ нь аль хэдийн каноник multisig хянагчийг суулгасан) кодчилдог ба `b"iroha:fastpq:v1:authority|" || encoded_account`-ийг Blake2b-256-тай хэш болгож, Kotodama-г хадгалдаг.
- `poseidon_preimage_digest` = Посейдон( || данснаас || хөрөнгийн || дүн || багц_хэш); Энэ нь гаджет нь хосттой ижил мэдээг дахин тооцоолох боломжийг олгодог. Урьдчилсан байтуудыг Poseidon2 туслахаар дамжуулахаас өмнө нүцгэн Norito кодчилол ашиглан `norito(from_account) || norito(to_account) || norito(asset_definition) || norito(amount) || batch_hash` хэлбэрээр бүтээдэг. Энэхүү хураангуй нь нэг дельта транскриптэд байдаг бөгөөд олон дельта багцын хувьд орхигдуулдаг.

Бүх талбарууд нь Norito-ээр цуварсан тул одоо байгаа детерминизмын баталгаа хүчинтэй байна.
`from_path` болон `to_path` хоёулаа Norito бөмбөлөг хэлбэрээр ялгардаг.
`TransferMerkleProofV1` схем: `{ version: 1, path_bits: Vec<u8>, siblings: Vec<Hash> }`.
Провер нь хувилбарын шошгыг хэрэгжүүлэх үед ирээдүйн хувилбарууд схемийг сунгаж болно
тайлахын өмнө. `TransitionBatch` мета өгөгдөл нь Norito кодлогдсон транскриптийг оруулсан
`transfer_transcripts` товчлуурын дор векторыг оруулснаар судлаач гэрчийг тайлж чадна.
хамтлагаас гадуур асуулга хийхгүйгээр. Нийтийн оролт (`dsid`, `slot`, үндэс,
`perm_root`, `tx_set_hash`) нь `FastpqTransitionBatch.public_inputs`,
оруулга хэш/хуулбарын тоо бүртгэлд мета өгөгдлийг үлдээх. Байшингийн сантехник хүртэл
lands, prover нь нийлэг аргаар түлхүүр/баланс хосуудаас нотолгоог гаргаж авдаг тул мөрүүд
Транскрипт нь нэмэлт талбаруудыг орхигдуулсан ч гэсэн тодорхойлогч SMT замыг үргэлж оруулна.

## Гаджетын зохион байгуулалт

1. **Тэнцвэрийн арифметик блок**
   - Оролтууд: `from_balance_before`, `amount`, `to_balance_before`.
   - Шалгалтууд:
     - `from_balance_before >= amount` (хуваалцсан RNS задрал бүхий хүрээний хэрэгсэл).
     - `from_balance_after = from_balance_before - amount`.
     - `to_balance_after = to_balance_before + amount`.
   - Захиалгат хаалганд савлагдсан тул гурван тэгшитгэл нь нэг эгнээний бүлгийг ашигладаг.2. **Посейдоны амлалтын блок**
   - Бусад гаджетуудад ашигласан Poseidon хайлтын хуваалцсан хүснэгтийг ашиглан `poseidon_preimage_digest`-г дахин тооцоолно. Ямар ч дамжуулалт бүрт Посейдоны тойрог замд.

3. **Merkle Path Block**
   - Одоо байгаа Kaigi SMT гаджетыг "хосолсон шинэчлэлт" горимоор өргөтгөсөн. Хоёр навч (илгээгч, хүлээн авагч) нь ах дүү хэшүүдэд ижил баганыг хуваалцаж, давхардсан мөрүүдийг багасгадаг.

4. **Эрх бүхий байгууллагын дүгнэлт**
   - Хүлээн авагчаас өгсөн мэдээ болон гэрчийн утгын хоорондох тэгш байдлын энгийн хязгаарлалт. Гарын үсэг нь тэдний тусгай хэрэгсэлд үлддэг.

5. **Багцын гогцоо**
   - Хөтөлбөрүүд нь `transfer_asset` бүтээгчдийн давталтын өмнө `transfer_v1_batch_begin()`, дараа нь `transfer_v1_batch_end()` гэж дууддаг. Хамрах хүрээ идэвхтэй байх үед хост шилжүүлэг бүрийг буфер болгож, нэг `TransferAssetBatch` хэлбэрээр дахин тоглуулж, Poseidon/SMT контекстийг багц бүрт нэг удаа дахин ашигладаг. Нэмэлт дельта бүр зөвхөн арифметик болон хоёр навчны чекийг нэмнэ. Транскрипт декодлогч нь одоо олон дельта багцуудыг хүлээн авч `TransferGadgetInput::deltas` хэлбэрээр гаргадаг тул төлөвлөгч Norito-г дахин уншихгүйгээр гэрчүүдийг нугалж болно. Аль хэдийн Norito даацтай (жишээ нь, CLI/SDK) гэрээнүүд нь `transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)` руу залгаснаар хамрах хүрээг бүхэлд нь алгасаж болно.

# Хөтлөгч ба Проверийн өөрчлөлтүүд| Давхарга | Өөрчлөлтүүд |
|-------|---------|
| `ivm::syscalls` | `transfer_v1_batch_begin` (`0x29`) / `transfer_v1_batch_end` (`0x2A`) нэмснээр програмууд `transfer_v1` олон тооны системийн дуудлагуудыг завсрын ISI004, нэмэх нь `transfer_v1` хаалтанд оруулах боломжтой. (`0x2B`) урьдчилан кодлогдсон багцын хувьд. |
| `ivm::host` & тестүүд | Үндсэн/Өгөгдмөл хостууд нь хамрах хүрээ идэвхтэй байх үед `transfer_v1`-г багцын хавсралт гэж үздэг бөгөөд `SYSCALL_TRANSFER_V1_BATCH_{BEGIN,END,APPLY}` гадаргуутай, хуурамч WSV хост нь оролтуудыг хийхээс өмнө буферээр хадгалдаг тул регрессийн тестүүд тодорхойлогддог тэнцвэрийг баталгаажуулдаг. шинэчлэлтүүд.【crates/ivm/src/core_host.rs:1001】【crates/ivm/src/host.rs:451】【crates/ivm/src/mock_wsv.rs :3713】【crates/ivm/tests/wsv_host_pointer_tlv.rs:219】【crates/ivm/tests/wsv_host_pointer_tlv.rs:287】
| `iroha_core` | Төрийн шилжилтийн дараа `TransferTranscript`-г ялгаруулж, `StateBlock::capture_exec_witness` үед `FastpqTransitionBatch` бичлэгүүдийг тодорхой `public_inputs`-ээр бүтээж, FASTPQ prover lane-г ажиллуулж, Torii болон St.6-г хүлээн авна. `TransitionBatch` оролтууд. `TransferAssetBatch` нь дараалсан шилжүүлгийг нэг транскрипт болгон бүлэглэж, олон дельта багцын посейдоны дижестийг орхигдуулдаг тул гаджет нь оруулгуудыг тодорхой хэмжээгээр давтаж чаддаг. |
| `fastpq_prover` | `gadgets::transfer` одоо төлөвлөгчийн (`crates/fastpq_prover/src/gadgets/transfer.rs`) олон дельта хуулбарыг (тэнцвэрийн арифметик + Посейдоны дижест) баталгаажуулж, бүтэцлэгдсэн гэрчүүдийг (орлуулагчтай хосолсон SMT blobуудыг оруулаад) гаргадаг. `trace::build_trace` нь багцын мета өгөгдлөөс гарсан хуулбарыг тайлж, `transfer_transcripts`-ийн даацын ачаалалгүй шилжүүлгийн багцаас татгалзаж, баталгаажсан гэрчүүдийг `Trace::transfer_witnesses`-д хавсаргаж, `TracePolynomialData::transfer_plan()` нь төлөвлөгөөг нэгтгэх хүртэл төлөвлөгөөгөө биелүүлэх хүртэл хадгалдаг. (`crates/fastpq_prover/src/trace.rs`). Мөр тоолох регрессийн бэхэлгээ нь одоо `fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`)-ээр дамжиж, 65536 жийргэвчтэй мөр хүртэлх хувилбаруудыг хамардаг бол хосолсон SMT утас нь TF-3 багцын туслах үе шатын ард үлддэг (байршуулагч нь тэр хүртэл ул мөрийг хадгалдаг). |
| Kotodama | `transfer_batch((from,to,asset,amount), …)` туслахыг `transfer_v1_batch_begin`, дараалсан `transfer_asset` болон `transfer_v1_batch_end` болгон бууруулна. Tuple аргумент бүр нь `(AccountId, AccountId, AssetDefinitionId, int)` хэлбэртэй байх ёстой; нэг шилжүүлэг нь одоо байгаа барилгачин хэвээр байна. |

Жишээ Kotodama ашиглалт:

```text
fn pay(a: AccountId, b: AccountId, asset: AssetDefinitionId, x: int) {
    transfer_batch((a, b, asset, x), (b, a, asset, 1));
}
```

`TransferAssetBatch` нь `Transfer::asset_numeric` дуудлагатай адил зөвшөөрөл болон арифметик шалгалтыг гүйцэтгэдэг боловч нэг `TransferTranscript` доторх бүх дельтуудыг бүртгэдэг. Олон дельта транскрипт нь дельта бүрийн амлалтууд дагаж мөрдөх хүртэл посейдоны задралыг арилгадаг. Kotodama бүтээгч одоо эхлэх/төгсгөлийн системийн дуудлагыг автоматаар ялгаруулдаг тул гэрээнүүд Norito ачааллыг гар кодчилолгүйгээр багц шилжүүлгийг байршуулах боломжтой.

## Мөр тоолох регрессийн бэхэлгээ

`fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) тохируулж болох сонгогчийн тоо бүхий FASTPQ шилжилтийн багцуудыг нэгтгэж, `row_usage` хураангуйг мэдээлдэг (`total_rows`, сонгогч бүрийн тоо, ₂ харьцаатай. 65536 эгнээтэй таазны жишиг үзүүлэлтүүдийг дараах байдлаар аваарай.

```bash
cargo run -p fastpq_prover --bin fastpq_row_bench -- \
  --transfer-rows 65536 \
  --mint-rows 256 \
  --burn-rows 128 \
  --pretty \
  --output fastpq_row_usage_max.json
```Гаргасан JSON нь одоо `iroha_cli audit witness`-ийн өгөгдмөлөөр ялгаруулдаг FASTPQ багц олдворуудыг тусгадаг (тэдгээрийг дарахын тулд `--no-fastpq-batches`-ийг дамжуулна уу), тиймээс `scripts/fastpq/check_row_usage.py` ба CI хаалга нь өмнөх өөрчлөлтүүдийг төлөвлөхдөө нийлэг гүйлтийг өөрчилдөг.

# Дамжуулах төлөвлөгөө

1. **TF-1 (Транскрипт сантехник)**: ✅ `StateTransaction::record_transfer_transcripts` одоо `TransferAsset`/багц бүрд Norito хуулбарыг ялгаруулдаг, `sumeragi::witness::record_fastpq_transcript` тэдгээрийг дэлхийн гэрч, Norito-д хадгалдаг. Операторуудад зориулсан `public_inputs` тодорхой `fastpq_batches` ба проверийн эгнээ (хэрэв танд туранхай хэрэгтэй бол `--no-fastpq-batches`-г ашиглаарай) гаралт).【crates/iroha_core/src/state.rs:8801】【crates/iroha_core/src/sumeragi/witness.rs:280】【crates/iroha_core/src/fastpq/mod.rs:157】【crates/157】【crates/iro.
2. **TF-2 (Гаджетын хэрэгжилт)**: ✅ `gadgets::transfer` нь олон дельта хуулбарыг (тэнцвэрийн арифметик + Посейдоны дижест) баталгаажуулж, хостууд орхигдуулсан тохиолдолд хосолсон SMT нотолгоог нэгтгэж, Kotodama болон Kotodama-ээр дамжуулан бүтэцлэгдсэн гэрчүүдийг илчилдэг. нотлох баримтаас SMT баганыг бөглөж байхдаа тэдгээр гэрчүүдийг `Trace::transfer_witnesses` руу оруулав. `fastpq_row_bench` 65536 эгнээний регрессийн бэхэлгээг авдаг тул төлөвлөгчид Norito-г дахин тоглуулахгүйгээр мөрийн ашиглалтыг хянадаг. ачаалал.【crates/fastpq_prover/src/gadgets/transfer.rs:1】【crates/fastpq_prover/src/trace.rs:1】【crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1】
3. **TF-3 (Багцын туслах)**: Багцын системийн дуудлагыг + Kotodama үүсгэгчийг идэвхжүүлэх, үүнд хостын түвшний дараалсан програм болон гаджетын гогцоо орно.
4. **TF-4 (Телеметрийн болон баримт бичиг)**: `fastpq_plan.md`, `fastpq_migration_guide.md` болон хяналтын самбарын схемүүдийг бусад гаджетуудтай харьцуулан дамжуулах мөрүүдийн гадаргуугийн хуваарилалтыг шинэчилнэ үү.

# Нээлттэй асуултууд

- **Домэйн хязгаар**: одоогийн FFT төлөвлөгч 2¹⁴ мөрийн цаана байгаа ул мөрийг сандраад байна. TF-2 нь домэйны хэмжээг нэмэгдүүлэх эсвэл жишиг зорилтыг бууруулж баримтжуулах ёстой.
- **Олон хөрөнгийн багц**: анхны гаджет нь дельта бүрт ижил өмчийн ID-г авна. Хэрэв бидэнд нэг төрлийн бус багц хэрэгтэй бол бид хөрөнгийн хооронд дахин тоглуулахаас сэргийлэхийн тулд Посейдоны гэрчийг тухайн хөрөнгийг оруулах бүртээ баталгаажуулах ёстой.
- **Эрх бүхий байгууллагын дахин ашиглалт**: Системийн дуудлагад гарын үсэг зурсан жагсаалтыг дахин тооцоолохоос зайлсхийхийн тулд бид ижил хураамжийг бусад зөвшөөрөгдсөн үйлдлүүдэд дахин ашиглах боломжтой.


Энэхүү баримт бичиг нь дизайны шийдвэрийг дагаж мөрддөг; чухал цэгүүд газардах үед үүнийг замын газрын зургийн оруулгатай синхрончлох.