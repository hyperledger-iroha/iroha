---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-05T09:28:11.908615+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS ፈጣን ማስጀመር

ይህ በእጅ የሚሰራ መመሪያ በሚወስነው SF-1 chunker መገለጫ ውስጥ ያልፋል፣
የI18NT0000002Xን የሚደግፉ አንጸባራቂ ፊርማ እና ባለብዙ አቅራቢ ማምጣት ፍሰት
የማከማቻ ቧንቧ መስመር. ከ[አንጸባራቂ የቧንቧ መስመር ጥልቅ ዳይቭ](manifest-pipeline.md) ጋር ያጣምሩት።
ለዲዛይን ማስታወሻዎች እና የ CLI ባንዲራ ማመሳከሪያ ቁሳቁስ.

## ቅድመ ሁኔታዎች

- Rust toolchain (`rustup update`)፣ የስራ ቦታ በአካባቢው ተዘግቷል።
- አማራጭ፡ [በኤስኤስኤል የመነጨ Ed25519 የቁልፍ ጥምር](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  መግለጫዎችን ለመፈረም.
- አማራጭ፡ Node.js ≥ 18 Docusaurus ፖርታል ለማየት ካቀዱ።

አጋዥ የCLI መልዕክቶችን ለማሳየት እየሞከርክ ሳለ `export RUST_LOG=info` አዘጋጅ።

## 1. የመወሰኛ ዕቃዎችን ያድሱ

Regenerate the canonical SF-1 chunking vectors. The command verifies the
existing council signature file, or appends a signature when `--signing-key` is
supplied.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --signing-key=<ed25519-private-key-hex>
```

ውጤቶች፡

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (ከተፈረመ)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. የተከፈለ ጭነት ቆርጠህ እቅዱን ተመልከት

የዘፈቀደ ፋይል ወይም ማህደር ለመቁረጥ `sorafs_chunker` ይጠቀሙ፡

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

ቁልፍ መስኮች:

- `profile` / `break_mask` - የ I18NI0000023X መለኪያዎችን ያረጋግጣል።
- `chunks[]` - የታዘዙ ማካካሻዎች፣ ርዝመቶች እና ጥቅጥቅ ያሉ BLAKE3 መፍጨት።

ለትላልቅ መጫዎቻዎች፣ ዥረት መልቀቅን ለማረጋገጥ እና በፕሮቴስት የተደገፈ ድግግሞሹን ያሂዱ
ባች መቆራረጥ በማመሳሰል ቆይታ፡

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. አንጸባራቂ ይገንቡ እና ይፈርሙ

የጭራሹን እቅድ፣ ተለዋጭ ስሞች እና የአስተዳደር ፊርማዎችን ተጠቅመው ወደ አንጸባራቂነት ያቅርቡ
`sorafs_manifest_builder`. ከታች ያለው ትዕዛዝ ነጠላ-ፋይል ጭነት ያሳያል; ማለፍ
ዛፍ ለመጠቅለል የማውጫ መንገድ (CLI በቃላት አነጋገር ይራመዳል)።

```bash
cargo run -p sorafs_car --bin sorafs_manifest_builder -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --council-signature=<signerhex>:<signaturehex>
```

ግምገማ `/tmp/docs.report.json` ለ፡-

- `chunking.chunk_digest_sha3_256` – SHA3 የማካካሻዎችን/ርዝመቶችን መፍጨት፣ ከሚከተሉት ጋር ይዛመዳል።
  chunker ቋሚዎች.
- `manifest.manifest_blake3` – BLAKE3 ዳይጀስት በማንፌክት ፖስታ ውስጥ ተፈርሟል።
- `chunk_fetch_specs[]` - ለኦርኬስትራዎች የታዘዙ መመሪያዎችን ያግኙ።

The `--council-signature` value must be a reviewed council signer public key and
Ed25519 signature pair. The command verifies every Ed25519 signature before
writing the envelope.

## 4. የባለብዙ አቅራቢ መልሶ ማግኛን አስመስለው

የ chunk እቅዱን ከአንድ ወይም ከዛ በላይ ለማጫወት ገንቢውን CLI ን ይጠቀሙ
አቅራቢዎች. ይህ ለ CI ጭስ ሙከራዎች እና ኦርኬስትራ ፕሮቶታይፕ ተስማሚ ነው።

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

ማረጋገጫዎች፡-

- `payload_digest_hex` ከአንጸባራቂ ዘገባው ጋር መዛመድ አለበት።
- `provider_reports[]` የወለል ንጣፎች ስኬት/የሽንፈት ቆጠራ በአንድ አቅራቢ።
- ዜሮ ያልሆነ I18NI0000034X የጀርባ-ግፊት ማስተካከያዎችን ያደምቃል.
- ለአንድ ሩጫ የታቀዱ አቅራቢዎችን ቁጥር ለመገደብ `--max-peers=<n>` ማለፍ
  እና የ CI ማስመሰያዎች በዋና እጩዎች ላይ ያተኩሩ።
- `--retry-budget=<n>` ነባሪውን በየቁልቁል የድጋሚ ሙከራ ብዛት (3) ይሽራል ስለዚህ እርስዎ
  ውድቀቶችን በሚያስገቡበት ጊዜ የኦርኬስትራ ሪገሬሽን በፍጥነት እንዲታይ ሊያደርግ ይችላል።

እንዳይሳካ `--expect-payload-digest=<hex>` እና `--expect-payload-len=<bytes>` ይጨምሩ
እንደገና የተገነባው ጭነት ከማንፀባረቂያው ሲወጣ በፍጥነት።

## 5. ቀጣይ ደረጃዎች

- ** የአስተዳደር ውህደት *** - አንጸባራቂ መፍጨት እና
  የፒን መዝገብ ቤት እንዲችል `manifest_signatures.json` ወደ ምክር ቤት የስራ ሂደት
  ተገኝነትን ያስተዋውቁ.
- ** የመመዝገቢያ ድርድር *** - ያማክሩ [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  አዲስ መገለጫዎችን ከመመዝገብዎ በፊት. አውቶማቲክ ቀኖናዊ እጀታዎችን መምረጥ አለበት
  (`namespace.name@semver`) ከቁጥር መታወቂያዎች በላይ።
- ** CI አውቶሜሽን *** - የቧንቧ መስመሮችን ለመልቀቅ ከላይ ያሉትን ትዕዛዞች ያክሉ ፣
  ዕቃዎች እና ቅርሶች ከተፈረመበት ጎን ለጎን የሚወስኑ መግለጫዎችን ያትማሉ
  ሜታዳታ