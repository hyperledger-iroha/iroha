---
lang: am
direction: ltr
source: docs/portal/docs/norito/getting-started.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3754b8549f90a4f325bb58a6b4e24bc052ec65d46a6352995c13555a8d5544bf
source_last_modified: "2026-04-08T09:18:21.504260+00:00"
translation_last_reviewed: 2026-04-08
translator: machine-google-reviewed
---

# Norito መጀመር

ይህ ፈጣን መመሪያ የKotodama ውል ለማጠናቀር አነስተኛውን የስራ ሂደት ያሳያል፣
የመነጨውን I18NT0000006X ባይት ኮድ በመፈተሽ፣ በአገር ውስጥ ማስኬድ እና ማሰማራት
ወደ Iroha መስቀለኛ መንገድ።

## ቅድመ ሁኔታዎች

1. የ Rust Toolchain (1.76 ወይም አዲስ) ይጫኑ እና ይህን ማከማቻ ይመልከቱ።
2. የሚደግፉ ሁለትዮሾችን ይገንቡ ወይም ያውርዱ፡
   - `koto_compile` - Kotodama ባይትኮድ የሚያወጣው IVM/Norito
   - `ivm_run` እና `ivm_tool` - የአካባቢ ማስፈጸሚያ እና የፍተሻ መገልገያዎች
   - `iroha` - በ Torii በኩል ውል ለማሰማራት ያገለግላል

   የማጠራቀሚያው Makefile እነዚህን ሁለትዮሾች በ`PATH` ላይ ይጠብቃል። አንተም ትችላለህ
   አስቀድመው የተገነቡ ቅርሶችን ያውርዱ ወይም ከምንጩ ይገንቧቸው። እርስዎ ካጠናቀሩ
   የመሳሪያ ሰንሰለት በአገር ውስጥ፣ የ Makefile ረዳቶችን በሁለትዮሽዎቹ ላይ ያመልክቱ፡

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. የማሰማራቱ ደረጃ ላይ ሲደርሱ የI18NT0000014X መስቀለኛ መንገድ እየሰራ መሆኑን ያረጋግጡ። የ
   ከዚህ በታች ያሉት ምሳሌዎች Torii በእርስዎ ውስጥ በተዋቀረው ዩአርኤል ሊደረስ ይችላል ብለው ያስባሉ
   `iroha` መገለጫ (`~/.config/iroha/cli.toml`)።

## 1. የ Kotodama ውል ማጠናቀር

ማከማቻው አነስተኛውን የ"ሄሎ አለም" ውል ይልካል።
`examples/hello/hello.ko`. ወደ Norito/IVM ባይትኮድ (`.to`) አጠናቅሩት፡-

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

ቁልፍ ባንዲራዎች፡-

- `--abi 1` ውሉን ወደ ABI ስሪት 1 ይቆልፋል ( ብቸኛው የሚደገፍ ስሪት በ ላይ
  የጽሑፍ ጊዜ)።
- `--max-cycles 0` ያልተገደበ አፈፃፀም ይጠይቃል; ለማሰር አዎንታዊ ቁጥር ያዘጋጁ
  ለዜሮ-እውቀት ማረጋገጫዎች ዑደት ንጣፍ.

## 2. የNorito አርቲፊክስን መርምር (አማራጭ)

ራስጌውን እና የተካተተውን ሜታዳታ ለማረጋገጥ `ivm_tool` ይጠቀሙ፡-

```sh
ivm_tool inspect target/examples/hello.to
```

የ ABI ስሪትን፣ የነቁ የባህሪ ባንዲራዎችን እና ወደ ውጭ የተላከውን ግቤት ማየት አለብህ
ነጥቦች. ይህ ከመሰማራቱ በፊት ፈጣን የንፅህና ማረጋገጫ ነው።

## 3. ውሉን በአገር ውስጥ ያካሂዱ

ባህሪን ለማረጋገጥ ባይትኮዱን በ`ivm_run` ያስፈጽሙ
መስቀለኛ መንገድ

```sh
ivm_run target/examples/hello.to --args '{}'
```

የ`hello` ምሳሌ ሰላምታ ይመዘግባል እና `SET_ACCOUNT_DETAIL` syscall ያወጣል።
ከማተምዎ በፊት በኮንትራት አመክንዮ ላይ እየደጋገሙ በአገር ውስጥ መሮጥ ጠቃሚ ነው።
በሰንሰለት ላይ ነው ።

## 4. በ `iroha` ማሰማራት

በውሉ ሲረኩ CLI ን በመጠቀም ወደ መስቀለኛ መንገድ ያሰማሩት።
የባለስልጣን መለያ፣ የመፈረሚያ ቁልፉ እና ወይ `.to` ፋይል ወይም
ቤዝ64 ጭነት፡-

```sh
iroha app contracts deploy \
  --authority <i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

ትዕዛዙ Norito አንጸባራቂ + ባይትኮድ ጥቅል በTorii ላይ ያቀርባል እና ያትማል።
የተገኘው የግብይት ሁኔታ. ግብይቱ ከተፈጸመ በኋላ, ኮዱ
በምላሹ ላይ የሚታየው hash መግለጫዎችን ለማውጣት ወይም ምሳሌዎችን ለመዘርዘር ሊያገለግል ይችላል፡-

```sh
iroha app contracts manifest get --code-hash 0x<hash>
```

## 5. ከ Torii ጋር ሩጡ

በባይቴኮድ የተመዘገበ፣ መመሪያ በማስገባት ሊጠሩት ይችላሉ።
የተቀመጠውን ኮድ የሚያመለክት (ለምሳሌ በ`iroha app contracts call --contract-address <contract-address> --entrypoint main --wait`
ወይም የእርስዎ መተግበሪያ ደንበኛ)። የመለያ ፈቃዶች የሚፈለገውን እንደሚፈቅዱ ያረጋግጡ
ሲስካልስ (`set_account_detail`፣ `transfer_asset`፣ ወዘተ)።

## ጠቃሚ ምክሮች እና መላ ፍለጋ

- የቀረቡትን ምሳሌዎች በአንድ ላይ ለማሰባሰብ እና ለማስፈጸም `make examples-run` ይጠቀሙ
  ተኩስ ሁለትዮሽዎቹ ከሌሉ `KOTO`/`IVM` የአካባቢ ተለዋዋጮችን ይሽሩ
  `PATH`.
- `koto_compile` የኤቢአይ ሥሪቱን ውድቅ ካደረገ፣ አጠናቃሪው እና መስቀለኛ መንገዱን ያረጋግጡ።
  ሁለቱም ኢላማ ABI v1 (ለመዘርዘር ያለ ክርክሮች `koto_compile --abi` ያሂዱ
  ድጋፍ)።
- CLI ሄክስ ወይም Base64 የመፈረሚያ ቁልፎችን ይቀበላል። ለሙከራ, መጠቀም ይችላሉ
  በ I18NI0000055X የወጡ ቁልፎች።
- የI18NT0000011X ክፍያ ጭነቶችን ሲያርሙ የ`ivm_tool disassemble` ንዑስ ትዕዛዝ ይረዳል
  መመሪያዎችን ከ I18NT0000003X ምንጭ ጋር ያዛምዱ።

ይህ ፍሰት በCI ውስጥ ጥቅም ላይ የሚውሉትን ደረጃዎች እና የውህደት ሙከራዎችን ያንጸባርቃል። ለበለጠ
ወደ Kotodama ሰዋሰው፣ syscall mappings እና Norito ውስጠ ገብተው ይመልከቱ፡

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`