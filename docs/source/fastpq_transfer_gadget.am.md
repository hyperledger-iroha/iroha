---
lang: am
direction: ltr
source: docs/source/fastpq_transfer_gadget.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 084add6296c5b884a6d6dc07425aeca9966576f0643f6a7cf555da3fc8586466
source_last_modified: "2026-01-08T12:24:34.985909+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% FastPQ የማስተላለፊያ መግብር ንድፍ

# አጠቃላይ እይታ

የአሁኑ የ FASTPQ እቅድ አውጪ በ`TransferAsset` መመሪያ ውስጥ የተሳተፈ እያንዳንዱን ጥንታዊ ኦፕሬሽን ይመዘግባል፣ ይህ ማለት እያንዳንዱ ዝውውር ለቀሪ ሂሳብ፣ ለሃሽ ዙሮች እና ለኤስኤምቲ ዝመናዎች ለየብቻ ይከፍላል። በእያንዳንዱ ዝውውር የመከታተያ ረድፎችን ለመቀነስ አስተናጋጁ ቀኖናዊውን የግዛት ሽግግር ማድረጉን በሚቀጥልበት ጊዜ አነስተኛውን የሂሳብ/የቁርጠኝነት ፍተሻዎችን የሚያረጋግጥ ልዩ መግብር እናስተዋውቃለን።

- ** ወሰን ***: ነጠላ ማስተላለፎች እና ትናንሽ ባች በነባሩ Kotodama/IVM `TransferAsset` syscall ወለል.
- **ግብ**፡ ለከፍተኛ መጠን ዝውውሮች የFFT/LDE አምድ አሻራ ይቁረጡ የመፈለጊያ ሰንጠረዦችን በማጋራት እና በየማስተላለፊያው ሂሳብን ወደ ውሱን እገዳ።

# አርክቴክቸር

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

## የግልባጭ ቅርጸት

አስተናጋጁ በየሲካል ጥሪ `TransferTranscript` ያወጣል፡-

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

- `batch_hash` ግልባጩን ከግብይት መግቢያ ነጥብ ሃሽ ጋር ለድጋሚ ጨዋታ ጥበቃ ያያይዘዋል።
- `authority_digest` የአስተናጋጁ ሃሽ በተደረደሩ ፈራሚዎች/የኮረም ውሂብ ላይ ነው። መግብር እኩልነትን ይፈትሻል ነገርግን የፊርማ ማረጋገጫን አይደግፍም። በትክክል አስተናጋጁ Norito-ኢ18NI00000029X (ቀደም ሲል ቀኖናዊውን መልቲሲግ ተቆጣጣሪውን የጨመረው) እና `b"iroha:fastpq:v1:authority|" || encoded_account`ን ከ Blake2b-256 ጋር በማመሳጠር የተገኘውን Kotodama
- `poseidon_preimage_digest` = Poseidon (መለያ_ከ || መለያ_ወደ || ንብረት || መጠን || ባች_ሃሽ); መግብሩ እንደ አስተናጋጁ ተመሳሳይ መፈጨት እንደገና እንደሚሰላ ያረጋግጣል። ፕሪሜጅ ባይት በጋራ Poseidon2 አጋዥ ውስጥ ከማለፉ በፊት ባዶ Norito ኢንኮዲንግ በመጠቀም እንደ `norito(from_account) || norito(to_account) || norito(asset_definition) || norito(amount) || batch_hash` የተገነቡ ናቸው። ይህ መፍጨት ለነጠላ-ዴልታ ግልባጮች አለ እና ለብዙ-ዴልታ ስብስቦች የተተወ ነው።

ሁሉም መስኮች በNorito ተከታታይ ናቸው ስለዚህ ነባር የመወሰን ዋስትናዎች ይቆያሉ።
ሁለቱም `from_path` እና `to_path` የሚለቀቁት እንደ Norito blobs በመጠቀም ነው።
`TransferMerkleProofV1` እቅድ፡ `{ version: 1, path_bits: Vec<u8>, siblings: Vec<Hash> }`።
prover የስሪት መለያውን ሲያስፈጽም የወደፊት ስሪቶች ንድፉን ሊያራዝሙ ይችላሉ።
ከመፍታቱ በፊት. `TransitionBatch` ሜታዳታ Norito-የተመሰጠረውን ግልባጭ አካትቷል
ቬክተር በ `transfer_transcripts` ቁልፍ ስር prover ምስክሩን መፍታት ይችላል።
ከባንዱ ውጪ የሆኑ መጠይቆችን ሳታደርጉ። የሕዝብ ግብዓቶች (`dsid`፣ `slot`፣ ሥሮች፣
`perm_root`፣ `tx_set_hash`) የተሸከሙት በ`FastpqTransitionBatch.public_inputs`፣
ሜታዳታ ለመግቢያ ሃሽ/ትራንስክሪፕት ቆጠራን በመተው። አስተናጋጅ ቧንቧ ድረስ
መሬቶች፣ prover ሰው ሰራሽ በሆነ መንገድ ማስረጃዎችን ከቁልፍ/ሚዛን ጥንዶች እና ረድፎች ያገኛል።
ግልባጩ የአማራጭ መስኮችን ቢያስቀምጥም ሁልጊዜ የሚወስን የSMT መንገድን ያካትቱ።

## የመግብር አቀማመጥ

1. **የሂሳብ ስሌት አግድ**
   - ግብዓቶች: `from_balance_before`, `amount`, `to_balance_before`.
   - ቼኮች;
     - `from_balance_before >= amount` (የጋራ መግብር ከጋራ RNS መበስበስ ጋር)።
     - `from_balance_after = from_balance_before - amount`.
     - `to_balance_after = to_balance_before + amount`.
   - ሦስቱም እኩልታዎች አንድ ረድፍ ቡድን እንዲፈጁ ወደ ብጁ በር የታሸገ።2. ** የፖሲዶን ቃል ኪዳን እገዳ**
   - ቀደም ሲል በሌሎች መግብሮች ውስጥ ጥቅም ላይ የዋለውን የተጋራ የPoseidon ፍለጋ ሰንጠረዥን በመጠቀም `poseidon_preimage_digest` እንደገና ያሰላል። በክትትል ውስጥ ምንም የፖሲዶን ዝውውር የለም።

3. **መርክል መንገድ ብሎክ**
   - ነባሩን የ Kaigi SMT መግብርን በ"የተጣመረ ዝማኔ" ሁነታ ያራዝመዋል። ሁለት ቅጠሎች (ላኪ፣ ተቀባይ) ለወንድም እህት ሃሽ አንድ አይነት አምድ ይጋራሉ፣ የተባዙ ረድፎችን ይቀንሳል።

4. **የባለስልጣን ዳይጀስት ፍተሻ**
   - በአስተናጋጁ የቀረበው የምግብ መፍጫ ሥርዓት እና የምሥክርነት እሴት መካከል ቀላል የእኩልነት ገደብ። በተሰጠ መግብር ውስጥ ፊርማዎች ይቀራሉ።

5. ** ባች ሉፕ ***
   - ፕሮግራሞች ከ `transfer_asset` ግንበኞች ምልልስ በፊት እና `transfer_v1_batch_end()` ወደ `transfer_v1_batch_begin()` ይደውሉ። ስፋቱ ንቁ ሆኖ ሳለ አስተናጋጁ እያንዳንዱን ማስተላለፍ እና እንደ አንድ ነጠላ `TransferAssetBatch` እንደገና ይጫወታቸዋል፣ Poseidon/SMT አውድ በቡድን አንድ ጊዜ ይጠቀማል። እያንዳንዱ ተጨማሪ ዴልታ የሂሳብ እና ሁለት ቅጠል ቼኮችን ብቻ ይጨምራል። የግልባጭ ዲኮደር አሁን ባለብዙ ዴልታ ባችዎችን ተቀብሎ እንደ `TransferGadgetInput::deltas` ላካቸው ስለዚህ እቅድ አውጪው Norito ሳያነብ ምስክሮችን ማጠፍ ይችላል። Norito የሚጫነው (ለምሳሌ፡ CLI/SDKs) ያላቸው ኮንትራቶች `transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)` በመደወል ሙሉ ለሙሉ መዝለል ይችላሉ፣ ይህም አስተናጋጁን በአንድ syscall ውስጥ ሙሉ በሙሉ ኮድ የያዘ።

# የአስተናጋጅ እና የፕሮቨር ለውጦች| ንብርብር | ለውጦች |
|-------|--------|
| `ivm::syscalls` | `transfer_v1_batch_begin` (`0x29`) / `transfer_v1_batch_end` (`0x2A`) ጨምር ፕሮግራሞች መካከለኛ አይኤስአይኤስን ሳያስወጡ በርካታ `transfer_v1` ሲስካልሎችን ማሰር እንዲችሉ እና I18000004 (`0x2B`) ለቅድመ-የተመሰጠሩ ስብስቦች። |
| `ivm::host` & ሙከራዎች | የኮር/ነባሪ አስተናጋጆች `transfer_v1`ን እንደ ባች አባሪ አድርገው ወሰናቸው ገባሪ በሆነበት ጊዜ፣ ላዩን `SYSCALL_TRANSFER_V1_BATCH_{BEGIN,END,APPLY}`፣ እና የይስሙላ WSV አስተናጋጅ ማቋቋሚያ ግቤቶችን ከመፈጸማቸው በፊት የማገገሚያ ሙከራዎች ወሳኙን ሚዛን ሊያረጋግጡ ይችላሉ። ዝመናዎች።【crates/ivm/src/core_host.rs:1001】【crates/ivm/src/host.rs:451】【crates/ivm/src/mock_wsv.rs :3713】【crates/ivm/tests/wsv_host_pointer_tlv.rs:219】【crates/ivm/tests/wsv_host_pointer_tlv.rs:287】
| `iroha_core` | ኢሚት `TransferTranscript` ከግዛቱ ሽግግር በኋላ የ`FastpqTransitionBatch` መዝገቦችን በግልፅ `public_inputs` በ `StateBlock::capture_exec_witness` ጊዜ ይገንቡ እና የ FASTPQ prover ሌይን ያሂዱ ስለዚህም ሁለቱም Torii/CLIonic መሳሪያ ይቀበላሉ `TransitionBatch` ግብዓቶች. `TransferAssetBatch` ቡድኖች በቅደም ተከተል ወደ አንድ ግልባጭ ያስተላልፋሉ፣ የፖሲዶን ዳይጀስት ለብዙ-ዴልታ ስብስቦች በመተው መግብር በወሳኝ ሁኔታ ግቤቶችን ይደግማል። |
| `fastpq_prover` | `gadgets::transfer` አሁን የብዝሃ-ዴልታ ግልባጮችን (ሚዛን አርቲሜቲክ + ፖሲዶን ዲጀስት) እና የተዋቀሩ ምስክሮችን (ቦታ ያዥ የተጣመሩ SMT ብሎቦችን ጨምሮ) ለእቅድ አውጪው (`crates/fastpq_prover/src/gadgets/transfer.rs`) ያረጋግጣል። `trace::build_trace` ግልባጭዎቹን ከባች ሜታዳታ ውጪ ያዘጋጃል፣የጠፋውን የ`transfer_transcripts` ክፍያ ጭነት ውድቅ ያደርጋል፣የተረጋገጡትን ምስክሮች ከ `Trace::transfer_witnesses` ጋር አያይዘዋል፣እና `TracePolynomialData::transfer_plan()` የተቀናጀውን ፕላን ህያው ያደርገዋል። (`crates/fastpq_prover/src/trace.rs`)። የረድፍ ቆጠራ ሪግሬሽን ማሰሪያው አሁን በ`fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) በኩል ይላካል፣ እስከ 65536 የታሸጉ ረድፎችን ይሸፍናል፣ የተጣመረው የኤስኤምቲ ሽቦ ግን ከTF-3 ባች አጋዥ ምእራፍ ጀርባ ይቆያል (ቦታ ያዢዎች የመከታተያ አቀማመጥ እስኪያቆዩ ድረስ)። |
| Kotodama | የ`transfer_batch((from,to,asset,amount), …)` ረዳትን ወደ `transfer_v1_batch_begin`፣ ተከታታይ የ`transfer_asset` ጥሪዎች እና `transfer_v1_batch_end` ዝቅ ያደርጋል። እያንዳንዱ tuple ክርክር `(AccountId, AccountId, AssetDefinitionId, int)` ቅርጽ መከተል አለበት; ነጠላ ዝውውሮች ነባሩን ግንበኛ ያቆያሉ። |

ምሳሌ Kotodama አጠቃቀም፡-

```text
fn pay(a: AccountId, b: AccountId, asset: AssetDefinitionId, x: int) {
    transfer_batch((a, b, asset, x), (b, a, asset, 1));
}
```

`TransferAssetBatch` እንደ ግለሰብ የ`Transfer::asset_numeric` ጥሪዎች ተመሳሳይ ፍቃድ እና የሂሳብ ቼኮችን ይሰራል፣ነገር ግን ሁሉንም ዴልታዎች በአንድ `TransferTranscript` ይመዘግባል። የብዝሃ-ዴልታ ግልባጮች የፔሲዶን መፈጨትን ያስወግዳሉ የዴልታ ግዴታዎች በክትትል ውስጥ እስከሚገቡ ድረስ። የ Kotodama ግንበኛ አሁን የመጀመርያ/ፍጻሜ syscallsን በራስ-ሰር ያመነጫል፣ስለዚህ ኮንትራቶች ያለእጅ ኮድ Norito ጭነት ማስተላለፍ ይችላሉ።

## የረድፍ ቆጠራ ሪግሬሽን ታጥቆ

`fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) FASTPQ የሽግግር ስብስቦችን ከተዋቀረ የመራጭ ቆጠራዎች ጋር በማዋሃድ የተገኘውን `row_usage` ማጠቃለያ (`total_rows`፣ በመራጭ ቆጠራ፣ ሬሾ) ጎን ለጎን። ባለ 65536-ረድፍ ጣሪያ መለኪያዎችን ያንሱ፡-

```bash
cargo run -p fastpq_prover --bin fastpq_row_bench -- \
  --transfer-rows 65536 \
  --mint-rows 256 \
  --burn-rows 128 \
  --pretty \
  --output fastpq_row_usage_max.json
```የወጣው JSON አሁን `iroha_cli audit witness` በነባሪ የሚያወጣውን የ FASTPQ ባች ቅርሶችን ያንፀባርቃል (ለመጨቆን `--no-fastpq-batches` ይለፉ)፣ ስለዚህ `scripts/fastpq/check_row_usage.py` እና የ CI በር ሰው ሰራሽ አሂድ ከቀደምት ቅጽበተ-ፎቶዎች ጋር ሊለያይ ይችላል።

# የልቀት እቅድ

1. **TF-1 (Transcript plumbing)**: ✅ `StateTransaction::record_transfer_transcripts` አሁን ለእያንዳንዱ `TransferAsset`/ባች `sumeragi::witness::record_fastpq_transcript` Norito ግልባጭ ያወጣል። `fastpq_batches` በግልጽ `public_inputs` ለኦፕሬተሮች እና prover ሌይን (ቀጭን ከፈለጉ `--no-fastpq-batches` ይጠቀሙ ውፅዓት)።
2. **TF-2 (የመግብር አተገባበር)**: ✅ `gadgets::transfer` አሁን የባለብዙ ዴልታ ግልባጮችን ያረጋግጣል (ሚዛን አርቲሜቲክ + ፖሲዶን ዲጀስት)፣ አስተናጋጆች ሲጥሏቸው የተጣመሩ የኤስኤምቲ ማረጋገጫዎች፣ የተዋቀሩ ምስክሮችን በ Kotodama በ Kotodama እነዚያን ክር በ Kotodama ከማስረጃዎቹ የSMT አምዶችን በሚሞሉበት ጊዜ ወደ Kotodama ምስክሮች። `fastpq_row_bench` ባለ 65536-ረድፍ ሪግሬሽን ትጥቆችን ስለሚይዝ እቅድ አውጪዎች Norito ሳትደግሙ የረድፍ አጠቃቀምን ይከታተላሉ የሚከፈል ጭነት።
3. **TF-3 (ባች አጋዥ)**፡ የአስተናጋጅ ደረጃ ተከታታይ አፕሊኬሽን እና መግብር ሉፕን ጨምሮ ባች syscall + Kotodama ገንቢን አንቃ።
4. **TF-4 (ቴሌሜትሪ እና ሰነዶች)**፡ `fastpq_plan.md`፣ `fastpq_migration_guide.md`፣ እና ዳሽቦርድ ንድፎችን ከሌሎች መግብሮች ጋር የማስተላለፊያ ረድፎችን ለመመደብ።

# ጥያቄዎችን ይክፈቱ

- **የጎራ ገደቦች**፡ የአሁኑ የኤፍኤፍቲ እቅድ አውጪ ከ2¹⁴ ረድፎች በላይ ለሆኑ ዱካዎች ደነገጠ። TF-2 የጎራውን መጠን ከፍ ማድረግ ወይም የተቀነሰ የቤንችማርክ ኢላማ መመዝገብ አለበት።
- ** ባለብዙ ንብረት ስብስቦች ***: የመጀመሪያ መግብር በዴልታ አንድ አይነት የንብረት መታወቂያ ይወስዳል። የተለያዩ ስብስቦችን ካስፈለገን፣ የንብረት መልሶ ማጫወትን ለመከላከል የPoseidon ምስክር ንብረቱን በእያንዳንዱ ጊዜ ማካተቱን ማረጋገጥ አለብን።
- **ባለስልጣን እንደገና ጥቅም ላይ ማዋል**፡- ለረጅም ጊዜ ፈራሚ ዝርዝሮችን በየ syscall ዳግም ማስላትን ለማስቀረት ለሌሎች የተፈቀደላቸው ስራዎች ተመሳሳዩን መፈጨት እንደገና መጠቀም እንችላለን።


ይህ ሰነድ የንድፍ ውሳኔዎችን ይከታተላል; ወሳኝ ደረጃዎች ሲደርሱ ከሮድ ካርታ ግቤቶች ጋር እንዲመሳሰል ያድርጉት።