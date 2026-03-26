---
lang: am
direction: ltr
source: docs/source/kotodama_grammar.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ac9b1fa221c6de46c139ee3a3c280957adad4910b49015fbb746259a4af22659
source_last_modified: "2026-01-30T12:29:10.190473+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama የቋንቋ ሰዋሰው እና ትርጓሜዎች

ይህ ሰነድ የKotodama የቋንቋ አገባብ (ሌክሲንግ፣ ሰዋሰው)፣ የመተየብ ህጎች፣ ቆራጥ ትርጉም እና ፕሮግራሞች እንዴት ወደ IVM ባይትኮድ (.to) ዝቅ ብለው ከ Norito ጠቋሚ-ABI ስምምነቶችን ይገልጻል። Kotodama ምንጮች .ko ቅጥያውን ይጠቀማሉ። አቀናባሪው IVM ባይትኮድ (.to) ያወጣል እና እንደ አማራጭ አንጸባራቂ መመለስ ይችላል።

ይዘቶች
- አጠቃላይ እይታ እና ግቦች
- የቃላት አወቃቀር
- ዓይነቶች እና ጽሑፎች
- መግለጫዎች እና ሞጁሎች
- የኮንትራት መያዣ እና ዲበ ውሂብ
- ተግባራት እና መለኪያዎች
- መግለጫዎች
- መግለጫዎች
- Builtins እና ጠቋሚ-ABI ገንቢዎች
- ስብስቦች እና ካርታዎች
- ቆራጥ ድግግሞሽ እና ገደቦች
- ስህተቶች እና ምርመራዎች
- Codegen ካርታ ወደ IVM
- ABI፣ ራስጌ እና አንጸባራቂ
- የመንገድ ካርታ

## አጠቃላይ እይታ እና ግቦች

- ቆራጥነት፡ ፕሮግራሞች በሃርድዌር ላይ ተመሳሳይ ውጤት ማምጣት አለባቸው። ምንም ተንሳፋፊ ነጥብ ወይም የማይታወቁ ምንጮች. ሁሉም የአስተናጋጅ መስተጋብሮች የሚከናወኑት በ Norito የተመሰጠሩ ክርክሮች ባለው syscalls በኩል ነው።
- ተንቀሳቃሽ፡ ዒላማዎች Iroha ምናባዊ ማሽን (IVM) ባይትኮድ እንጂ አካላዊ ISA አይደለም። RISC-V-እንደ ማከማቻው ውስጥ የሚታዩ ኢንኮዲንግ የIVM ዲኮዲንግ የትግበራ ዝርዝሮች ናቸው እና የሚታይ ባህሪ መቀየር የለባቸውም።
- ተሰሚነት ያለው: ትንሽ, ግልጽ ትርጓሜዎች; አገባብ ወደ IVM ኦፕኮድ እና syscalls ለማስተናገድ ግልጽ ካርታ።
- ወሰን፡- ባልተገደበ ውሂብ ላይ የሚደረጉ ቀለበቶች ግልጽ የሆኑ ገደቦችን መያዝ አለባቸው። የካርታ ድግግሞሽ ቆራጥነትን ለማረጋገጥ ጥብቅ ህጎች አሉት።

## የቃላት አወቃቀር

ነጭ ቦታ እና አስተያየቶች
- ዋይትስፔስ ቶከኖችን ይለያል እና በሌላ መልኩ ኢምንት ነው።
- የመስመር አስተያየቶች በ `//` ይጀምሩ እና ወደ መስመር መጨረሻ ይሮጣሉ።
- አስተያየቶችን አግድ `/* ... */` ጎጆ አያደርግም።

መለያዎች
- ይጀምሩ: `[A-Za-z_]` ከዚያ `[A-Za-z0-9_]*` ይቀጥሉ።
- ጉዳይ-ስሜታዊ; `_` ትክክለኛ መለያ ነው ግን ተስፋ የቆረጠ ነው።

ቁልፍ ቃላት (የተያዙ)
- `seiyaku`፣ `hajimari`፣ `kotoage`፣ `kaizen`፣ `state`፣ `struct`፣ Kotodama `const`፣ `return`፣ `if`፣ `else`፣ `while`፣ `for`፣ `in`00 `continue`፣ `true`፣ `false`፣ `permission`፣ `kotoba`።

ኦፕሬተሮች እና ሥርዓተ-ነጥብ
- አርቲሜቲክ: `+ - * / %`
- Bitwise፡ `& | ^ ~`፣ ፈረቃ `<< >>`
- አወዳድር: `== != < <= > >=`
- ምክንያታዊ: `&& || !`
- መድብ: `= += -= *= /= %= &= |= ^= <<= >>=`
- የተለያዩ: `: , ; . :: ->`
- ቅንፎች: `() [] {}`ቃል በቃል
- ኢንቲጀር፡ አስርዮሽ (`123`)፣ ሄክስ (`0x2A`)፣ ሁለትዮሽ (`0b1010`)። ሁሉም ኢንቲጀሮች በ64-ቢት በሩጫ ሰአት ተፈርመዋል። ቀጥተኛ ቅጥያ የሌላቸው በመረጃ ወይም በነባሪ እንደ `int` ይጻፋሉ።
- ሕብረቁምፊ፡ ድርብ ጥቅስ ከማምለጫ ጋር (`\n`፣ `\r`፣ `\t`፣ `\0`፣ `\xNN`፣ `\u{...}`፣ `\u{...}`፣ `\u{...}`፣000 `\\`); ዩቲኤፍ-8 ጥሬ ገመዶች `r"..."` ወይም `r#"..."#` ማምለጫዎችን ያሰናክሉ እና አዲስ መስመሮችን ይፍቀዱ።
- ባይት: `b"..."` ከማምለጫ ጋር, ወይም ጥሬ `br"..."` / `rb"..."`; `bytes` በጥሬው ይሰጣል።
- ቡሊያን: `true`, `false`.

## አይነቶች እና ስነ-ጽሁፎች

Scalar አይነቶች
- `int`: 64-ቢት ሁለት-ማሟያ; አርቲሜቲክ መጠቅለያዎች ሞዱሎ 2 ^ 64 ለ add / sub / mul; ክፍፍል በIVM ውስጥ የተፈረሙ/ያልተፈረሙ ልዩነቶችን ገልጿል; አቀናባሪው ለትርጉም ሥራ ተገቢውን ኦፕ ይመርጣል።
- `fixed_u128`፣ `Amount`፣ `Balance`፡ በ Norito `Numeric` (የተፈረመ አስርዮሽ እስከ 512-ቢት ማንቲሳ) የተደገፈ። Kotodama እነዚህን ተለዋጭ ስሞች እንደ አሉታዊ ያልሆኑ መጠኖች ይመለከታቸዋል። አርቲሜቲክ ይጣራል፣ ተለዋጭ ስም ይጠብቃል፣ እና ከመጠን በላይ ፍሰት ወይም በዜሮ የሚከፋፈል ወጥመዶች። ከ `int` የተፈጠሩ እሴቶች መለኪያ 0; ወደ/ከ`int` የተደረጉ ልወጣዎች በክልል-የተፈተሹ በሂደት ላይ ናቸው (አሉታዊ ያልሆነ፣ ውህደቱ፣ በ i64 ውስጥ የሚስማማ)።
- `bool`: ምክንያታዊ እውነት ዋጋ; ወደ `0`/`1` ዝቅ ብሏል።
- `string`: የማይለወጥ UTF-8 ሕብረቁምፊ; ወደ syscalls ሲተላለፉ እንደ Norito TLV ተወክሏል; የውስጠ-ቪኤም ኦፕሬሽኖች ባይት ቁርጥራጭ እና ርዝመት ይጠቀማሉ።
- `bytes`: ጥሬ Norito ጭነት; የጠቋሚውን-ABI `Blob` አይነት ለሃሺንግ/crypto/proof ግብዓቶች እና ለረጅም ጊዜ ተደራቢዎች ይለዋወጣል።

የተዋሃዱ ዓይነቶች
- `struct Name { field: Type, ... }` በተጠቃሚ የተገለጹ የምርት ዓይነቶች። ገንቢዎች በገለፃዎች ውስጥ የጥሪ አገባብ `Name(a, b, ...)` ይጠቀማሉ። የመስክ መዳረሻ `obj.field` ይደገፋል እና ከውስጥ ወደ tuple-style አቀማመጥ ሜዳዎች ዝቅ ይላል። የሚበረክት ሁኔታ ABI በሰንሰለት ላይ Norito-encoded ነው; አቀናባሪው የመዋቅር ቅደም ተከተሎችን እና የቅርብ ጊዜ ሙከራዎችን (`crates/iroha_core/tests/kotodama_struct_overlay.rs`) የሚያንፀባርቁ ተደራቢዎችን ያወጣል።
- `Map<K, V>`: ቆራጥ አሶሺዬቲቭ ካርታ; የትርጓሜ ትምህርት በመድገም ጊዜ መደጋገምን እና ሚውቴሽን ይገድባል (ከዚህ በታች ይመልከቱ)።
- `Tuple (T1, T2, ...)`: የማይታወቅ የምርት ዓይነት ከአቀማመጥ መስኮች ጋር; ለብዙ መመለስ ጥቅም ላይ ይውላል.

ልዩ ጠቋሚ-ኤቢአይ ዓይነቶች (አስተናጋጅ ፊት ለፊት)
- `AccountId`፣ `AssetDefinitionId`፣ `Name`፣ `Json`፣ `NftId`፣ `Blob` እና ተመሳሳይ አንደኛ ደረጃ የሩጫ ጊዜ አይነቶች አይደሉም። ወደ INPUT ክልል (Norito TLV ኤንቨሎፕ) የተተየቡ፣ የማይለወጡ ጠቋሚዎችን የሚያፈሩ ግንበኞች ናቸው እና እንደ ሲስካል ነጋሪ እሴቶች ብቻ የሚያገለግሉ ወይም ያለ ሚውቴሽን በተለዋዋጮች መካከል የሚንቀሳቀሱ።

ኢንፈረንስ ይተይቡ
- የአካባቢ `let` ማሰሪያዎች ኢንፈር አይነት ከጀማሪ። የተግባር መለኪያዎች በግልጽ መተየብ አለባቸው። አሻሚ ካልሆነ በስተቀር የመመለሻ ዓይነቶች ሊገመቱ ይችላሉ።

## መግለጫዎች እና ሞጁሎችከፍተኛ-ደረጃ ንጥሎች
- ኮንትራቶች፡- `seiyaku Name { ... }` ተግባራትን፣ ግዛትን፣ መዋቅርን እና ሜታዳታን ይይዛሉ።
- በአንድ ፋይል ውስጥ ብዙ ኮንትራቶች ይፈቀዳሉ ነገር ግን ተስፋ መቁረጥ; አንድ ዋና `seiyaku` እንደ ነባሪ መግቢያ በማንፌክቶች ውስጥ ጥቅም ላይ ይውላል።
- `struct` መግለጫዎች የተጠቃሚ ዓይነቶችን በውል ውስጥ ይገልፃሉ።

ታይነት
- `kotoage fn` የሕዝብ መግቢያ ነጥብ ያመለክታል; ታይነት የላኪ ፈቃዶችን እንጂ ኮድጅንን አይነካም።
- የአማራጭ የመዳረሻ ፍንጮች፡- `#[access(read=..., write=...)]` የማንበብ/የመፃፍ ቁልፎችን ለማቅረብ `fn`/`kotoage fn` ሊቀድም ይችላል። አቀናባሪው የምክር ፍንጮችን በራስ ሰር ያወጣል፤ ግልጽ ያልሆነ አስተናጋጅ ጥሪዎች ወደ ወግ አጥባቂ የዱር ካርድ ቁልፎች (`*`) ይመለሳሉ እና ግልጽ የሆኑ የመዳረሻ ፍንጮች ካልተሰጡ በስተቀር የምርመራ ውጤትን ያሳያሉ፣ ስለዚህ መርሐግብር አውጪዎች ለደቃቁ-ጥራጥሬ ቁልፎች ወደ ተለዋዋጭ ፕሪፓስ ውስጥ መምረጥ ይችላሉ።

## የኮንትራት ኮንቴይነር እና ሜታዳታ

አገባብ
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

የትርጓሜ ትምህርት
- `meta { ... }` መስኮች የማጠናከሪያ ነባሪዎችን ለተለቀቀው IVM አርዕስት፡ `abi_version`፣ `vector_length` (0 ማለት ያልተዋቀረ ማለት ነው)፣ `max_cycles` (0 ማለት የአቀናባሪ ነባሪ0 ወደ I1810000138X) ቢትስ (ZK መከታተል፣ የቬክተር ማስታወቅ)። አቀናባሪው `max_cycles: 0`ን እንደ “ነባሪ ተጠቀም” ይቆጥረዋል እና የመግቢያ መስፈርቶችን ለማሟላት የተዋቀረውን ዜሮ ያልሆነ ነባሪ ያወጣል። የማይደገፉ ባህሪያት ከማስጠንቀቂያ ጋር ችላ ይባላሉ። `meta {}` ሲቀር፣ አቀናባሪው `abi_version = 1` ያወጣል እና ለቀሪዎቹ የራስጌ መስኮች አማራጭ ነባሪዎች ይጠቀማል።
- `features: ["zk", "simd"]` (ቅጽል ስም፡ `"vector"`) ተዛማጅ የሆኑትን የራስጌ ቢትስ በግልፅ ይጠይቃል። ያልታወቁ የባህሪ ሕብረቁምፊዎች አሁን ችላ ከመባል ይልቅ የመተንተኛ ስህተት ይፈጥራሉ።
- `state` የሚበረክት የኮንትራት ተለዋዋጮች ያውጃል። ማቀናበሪያው ወደ `STATE_GET/STATE_SET/STATE_DEL` syscalls መግባቱን ዝቅ ያደርጋል እና አስተናጋጁ በየግብይት ተደራቢ (የመመልከቻ ነጥብ/ወደነበረበት መመለስ፣ ወደ WSV-በማስገባት) ደረጃ ያደርጋቸዋል። የመዳረሻ ፍንጮች ለትክክለኛው የግዛት ጎዳናዎች ይወጣሉ; ተለዋዋጭ ቁልፎች ወደ ካርታ-ደረጃ ግጭት ቁልፎች ይመለሳሉ። ግልጽ በሆነ አስተናጋጅ ለሚደገፉ ንባብ/መፃፍ የ`state_get/state_set/state_del` አጋዥ እና የ`get_or_insert_default` ካርታ ረዳቶችን ይጠቀሙ። እነዚህ በNorito TLVs በኩል የሚሄዱ ሲሆን የስሞች/የመስክ ቅደም ተከተሎችን የተረጋጋ እንዲሆን ያድርጉ።
- የግዛት መለያዎች የተጠበቁ ናቸው; የ `state` ስም በመለኪያ ወይም `let` ማሰሪያው ውድቅ ተደርጓል (`E_STATE_SHADOWED`)።
- የግዛት ካርታ እሴቶች አንደኛ ደረጃ አይደሉም፡ ለካርታ ስራዎች እና ለመድገም የስቴት መለያን በቀጥታ ይጠቀሙ። የስቴት ካርታዎችን በተጠቃሚ-የተገለጹ ተግባራት ላይ ማሰር ወይም ማለፍ ውድቅ ሆኗል (`E_STATE_MAP_ALIAS`)።
- ዘላቂ የስቴት ካርታዎች በአሁኑ ጊዜ `int` እና የጠቋሚ-ABI ቁልፍ ዓይነቶችን ብቻ ይደግፋሉ; ሌሎች ቁልፍ ዓይነቶች በማጠናቀር ጊዜ ውድቅ ይደረጋሉ።
- ዘላቂ የግዛት መስኮች `int`፣ `bool`፣ `Json`፣ `Blob`/`bytes`፣ ወይም የጠቋሚ-ABI ዓይነቶች (መዋቅሮች/ቱፕል ኮምፖችን ጨምሮ) መሆን አለባቸው። `string` የሚበረክት ሁኔታ አይደገፍም.

### የኮቶባ አከባቢ
አገባብ
```
kotoba {
  "E_UNBOUNDED_ITERATION": { en: "Loop over map lacks a bound." }
}
```የትርጓሜ ትምህርት
- `kotoba` ግቤቶች የትርጉም ሠንጠረዦችን ከኮንትራቱ መግለጫ (`kotoba` መስክ) ጋር ያያይዙታል።
- የመልእክት መታወቂያዎች እና የቋንቋ መለያዎች መለያዎችን ወይም የሕብረቁምፊ ቃል በቃል ይቀበላሉ; ግቤቶች ባዶ ያልሆኑ መሆን አለባቸው.
- የተባዙ `msg_id` + የቋንቋ መለያ ጥንዶች በተጠናቀረ ጊዜ ውድቅ ይደረጋሉ።

## ቀስቅሴ መግለጫዎች

ቀስቅሴ መግለጫዎች የመርሐግብር ዲበ ውሂብ ከመግቢያ ነጥብ መገለጫዎች ጋር ያያይዙ እና በራስ-የተመዘገቡ ናቸው።
የኮንትራት ምሳሌ ሲነቃ (በማሰናከል ላይ ይወገዳል). በ ውስጥ የተተነተኑ ናቸው።
`seiyaku` እገዳ.

አገባብ
```
register_trigger wake {
  call run;
  on time pre_commit;
  repeats 2;
  authority "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB";
  metadata { tag: "alpha"; count: 1; enabled: true; }
}
```

ማስታወሻዎች
- `call` የሕዝብ `kotoage fn` መግቢያ ነጥብ በተመሳሳይ ውል ማጣቀስ አለበት; አማራጭ
  `namespace::entrypoint` በመግለጫው ውስጥ ተመዝግቧል ነገር ግን የውል ማቋረጫ ጥሪ ውድቅ ተደርጓል
  አሁን ባለው የሂደት ጊዜ (የአካባቢ ጥሪዎች ብቻ)።
- የሚደገፉ ማጣሪያዎች፡ `time pre_commit` እና `time schedule(start_ms, period_ms?)`፣ በተጨማሪም
  `execute trigger <name>` ለጥሪ ቀስቅሴዎች፣ `data any` እና የቧንቧ መስመር ማጣሪያዎች
  (`pipeline transaction`፣ `pipeline block`፣ `pipeline merge`፣ `pipeline witness`)።
- `authority` እንደ አማራጭ ቀስቃሽ ባለስልጣንን ይሽራል (የAccountId string በቀጥታ)። ከተተወ፣
  የሩጫ ጊዜው የኮንትራት-ማግበር ባለስልጣንን ይጠቀማል.
- የሜታዳታ እሴቶች JSON ቀጥተኛ (`string`፣ `number`፣ `bool`፣ `null`) ወይም `json!(...)` መሆን አለባቸው።
- በአሂድ ጊዜ የተወጉ ቀስቅሴ ሜታዳታ ቁልፎች፡- `contract_namespace`፣ `contract_id`፣
  `contract_entrypoint`፣ `contract_code_hash`፣ `contract_trigger_id`።

## ተግባራት እና መለኪያዎች

አገባብ
- መግለጫ: `fn name(param1: Type, param2: Type, ...) -> Ret { ... }`
- የህዝብ: `kotoage fn name(...) { ... }`
- ማስጀመሪያ፡- `hajimari() { ... }` (በቪኤም ራሱ ሳይሆን በ runtime ተጠርቷል)።
- አሻሽል መንጠቆ: `kaizen(args...) permission(Role) { ... }`.

መለኪያዎች እና መመለሻዎች
- ክርክሮች በመዝገቦች `r10..r22` እንደ እሴቶች ወይም INPUT ጠቋሚዎች (Norito TLV) በ ABI ይተላለፋሉ; ለመደርደር ተጨማሪ አርጎች ይፈስሳሉ.
- ተግባራት ዜሮ ወይም አንድ scalar ወይም tuple ይመለሳሉ። ዋናው የመመለሻ ዋጋ በ `r10` ለ scalar; ቱፕልስ በተደራራቢ/በኮንቬንሽን ተዘጋጅተዋል።

## መግለጫዎች- ተለዋዋጭ ማያያዣዎች፡ `let x = expr;`፣ `let mut x = expr;` (ተለዋዋጭነት የተጠናቀረ ጊዜ ቼክ ነው፤ የአሂድ ጊዜ ሚውቴሽን የሚፈቀደው ለአካባቢው ነዋሪዎች ብቻ ነው)።
- ምደባ፡ `x = expr;` እና ውህድ ቅጾች `x += 1;` ወዘተ. ዒላማዎች ተለዋዋጭ ወይም የካርታ ኢንዴክሶች መሆን አለባቸው; tuple/struct መስኮች የማይለወጡ ናቸው።
- የቁጥር ስሞች (`fixed_u128`, `Amount`, `Balance`) የተለዩ `Numeric` የሚደገፉ ዓይነቶች ናቸው; አርቲሜቲክ ተለዋጭ ስሞችን ይጠብቃል እና ተለዋጭ ስሞችን ማደባለቅ በ `int` ማሰሪያ በኩል መለወጥን ይጠይቃል። ወደ/ከ`int` የተደረጉ ልወጣዎች በአሂድ ጊዜ (አሉታዊ ያልሆነ፣ ውህደቱ፣ ክልል-የተገደበ) ላይ ምልክት ይደረግባቸዋል።
- ቁጥጥር: `if (cond) { ... } else { ... }`, `while (cond) { ... }`, C-style `for (init; cond; step) { ... }`.
  - `for` ማስጀመሪያ እና ደረጃዎች ቀላል `let name = expr` ወይም መግለጫ መግለጫዎች መሆን አለበት; ውስብስብ ማበላሸት ውድቅ ተደርጓል (`E0005`፣ `E0006`)።
  - `for` ስፒንግ: ከመግቢያው አንቀፅ ውስጥ ማያያዣዎች በሉፕ እና ከዚያ በኋላ ይታያሉ; በሰውነት ወይም በደረጃ ውስጥ የተፈጠሩ ማሰሪያዎች ከሉፕ አያመልጡም.
- እኩልነት (`==`, `!=`) ለ `int`, `bool`, `string`, ጠቋሚ-ABI scalars (ለምሳሌ, I102NI050X) ይደገፋል. `Name`፣ `Blob`/`bytes`፣ `Json`); tuples፣ structs እና ካርታዎች አይወዳደሩም።
- የካርታ ምልልስ: `for (k, v) in map { ... }` (የሚወስነው; ከታች ይመልከቱ).
- ፍሰት፡ `return expr;`፣ `break;`፣ `continue;`።
- ይደውሉ፡ `name(args...);` ወይም `call name(args...);` (ሁለቱም ተቀባይነት አላቸው፤ አቀናባሪው መግለጫዎችን ለመጥራት መደበኛ ያደርገዋል)።
- ማረጋገጫዎች: `assert(cond);`, `assert_eq(a, b);` ካርታ ወደ IVM `ASSERT*` ያልሆኑ ZK ግንባታዎች ወይም ZK ገደቦች ZK ሁነታ ውስጥ.

## መግለጫዎች

ቅድሚያ (ከፍተኛ → ዝቅተኛ)
1. አባል/ኢንዴክስ፡ `a.b`፣ `a[b]`
2. Unary: `! ~ -`
3. ማባዛት: `* / %`
4. ተጨማሪ፡ `+ -`
5. ፈረቃ፡ `<< >>`
6. ተዛማጅ: `< <= > >=`
7. እኩልነት: `== !=`
8. Bitwise AND/XOR/ወይም፡ `& ^ |`
9. አመክንዮአዊ እና/ወይም፡ `&& ||`
10. ደረጃ: `cond ? a : b`

ጥሪዎች እና tuples
ጥሪዎች የቦታ ነጋሪ እሴቶችን ይጠቀማሉ፡- `f(a, b, c)`።
- Tuple ቀጥተኛ: `(a, b, c)` እና ማጥፋት: `let (x, y) = pair;`.
- Tuple destructuring tuple/struct አይነቶችን የሚዛመድ አሪቲ ያስፈልገዋል; አለመመጣጠን ውድቅ ነው።

ሕብረቁምፊዎች እና ባይት
- ሕብረቁምፊዎች UTF-8; ጥሬ ሕብረቁምፊ እና ባይት ቀጥተኛ ቅጾች ከምንጩ ተቀባይነት አላቸው።
- ባይት ቀጥተኛ (`b"..."`, `br"..."`, `rb"..."`) ወደ `bytes` (ብሎብ) ጠቋሚዎች ዝቅተኛ; አንድ syscall NoritoBytes TLV ጭነት ሲጠብቅ `norito_bytes(...)` ጋር መጠቅለል.

## Builtins እና ጠቋሚ-ABI ገንቢዎች

ጠቋሚ ገንቢዎች (Norito TLV ወደ INPUT ይልቀቁ እና የተተየበው ጠቋሚ ይመልሱ)
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
- `proof_blob(string|0xhex) -> ProofBlob*`Prelude ማክሮዎች ለእነዚህ ግንበኞች አጠር ያሉ ተለዋጭ ስሞችን እና የመስመር ላይ ማረጋገጫን ይሰጣሉ፡-
- `account!("<i105-account-id>")`፣ `account_id!("<i105-account-id>")`
- `asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")`፣ `asset_id!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")`
- `domain!("wonderland")`፣ `domain_id!("wonderland")`
- `name!("example")`
- `json!("{\"hello\":\"world\"}")` ወይም እንደ `json!{ hello: "world" }` ያሉ የተዋቀሩ ቀጥተኛ ቃላት
- `nft_id!("dragon$demo")`፣ `blob!("bytes")`፣ `norito_bytes!("...")`

ማክሮዎቹ ከላይ ወደ ገንቢዎቹ ይስፋፋሉ እና በተጠናቀረ ጊዜ ልክ ያልሆኑ የቃል ፊደሎችን አይቀበሉም።

የአተገባበር ሁኔታ
- ተተግብሯል፡ ገንቢዎች ከላይ የቃል በቃል ክርክሮችን ይቀበላሉ እና ዝቅተኛ ወደ የተተየቡ Norito TLV ኢንቨሎፕ በ INPUT ክልል ውስጥ ይቀመጣሉ። እንደ syscall ነጋሪ እሴቶች የማይለወጡ የተተየቡ ጠቋሚዎችን ይመለሳሉ። ቀጥተኛ ያልሆኑ የሕብረቁምፊ አገላለጾች ውድቅ ይደረጋሉ; ለተለዋዋጭ ግብዓቶች `Blob`/`bytes` ይጠቀሙ። `blob`/`norito_bytes` እንዲሁም `bytes` የተተየቡ እሴቶችን ያለማክሮ ሺምስ ይቀበላሉ።
- የተራዘሙ ቅጾች;
  - `json(Blob[NoritoBytes]) -> Json*` በ `JSON_DECODE` syscall.
  - `name(Blob[NoritoBytes]) -> Name*` በ `NAME_DECODE` syscall.
  ጠቋሚ መፍታት ከብሎብ/NoritoBytes፡ ማንኛውም ጠቋሚ ገንቢ (AXT አይነቶችን ጨምሮ) የ`Blob`/`NoritoBytes` ክፍያን ተቀብሎ ወደ `POINTER_FROM_NORITO` ዝቅ ብሎ በሚጠበቀው አይነት መታወቂያ።
  - ለጠቋሚ ቅጾች ማለፍ፡- `name(Name) -> Name*`፣ `blob(Blob) -> Blob*`፣ `norito_bytes(Blob) -> Blob*`።
  - ዘዴ ስኳር ይደገፋል: `s.name()`, `s.json()`, `b.blob()`, `b.norito_bytes()`.

Host/syscall builtins (ካርታ ወደ SCALL፤ ትክክለኛ ቁጥሮች በivm.md)
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

የመገልገያ ግንባታዎች
- `info(string|int)`: በOUTPUT በኩል የተዋቀረ ክስተት/መልእክት ያወጣል።
- `hash(blob) -> Blob*`፡ Norito-encoded hash እንደ ብሎብ ይመልሳል።
- `build_submit_ballot_inline(election_id, ciphertext, nullifier32, backend, proof, vk) -> Blob*` እና `build_unshield_inline(asset, to, amount, inputs32, backend, proof, vk) -> Blob*`: የመስመር ውስጥ ISI ግንበኞች; ሁሉም ነጋሪ እሴቶች የተጠናቀረ-ጊዜ ቀጥተኛ (የሕብረቁምፊ ቃል በቃል ወይም የጠቋሚ አዘጋጆች ከጽሑፋዊ) መሆን አለባቸው። `nullifier32` እና `inputs32` በትክክል 32 ባይት (ጥሬ ገመድ ወይም `0x` hex) መሆን አለባቸው እና `amount` አሉታዊ ያልሆኑ መሆን አለባቸው።
- `schema_info(Name*) -> Json* { "id": "<hex>", "version": N }`
- `encode_schema(Name*, Json*) -> Blob`፡ የአስተናጋጁን እቅድ መዝገብ በመጠቀም JSON ን ኮድ ያደርጋል (DefaultRegistry `QueryRequest` እና `QueryResponse` ከትእዛዝ/የንግድ ናሙናዎች በተጨማሪ ይደግፋል)።
- `decode_schema(Name*, Blob|bytes) -> Json*`: የአስተናጋጅ schema መዝገብ በመጠቀም Norito ባይት መፍታት.
- `pointer_to_norito(ptr) -> NoritoBytes*`፡ ነባር ጠቋሚ-ABI TLVን እንደ NoritoBytes ለማከማቻ ወይም ለማጓጓዝ ይጠቀልላል።
- `isqrt(int) -> int`፡ ኢንቲጀር ካሬ ስር (`floor(sqrt(x))`) እንደ IVM ኦፕኮድ ተተግብሯል።
- `min(int, int) -> int`፣ `max(int, int) -> int`፣ `abs(int) -> int`፣ `div_ceil(int, int) -> int`፣ `gcd(int, int) -> int`፣ `mean(int, int) -> int` — የተዋሃዱ የሂሳብ ረዳቶች180 በNNT0s የተደገፉ (የጣሪያ ክፍፍል ወጥመዶች በዜሮ ክፍፍል ላይ).ማስታወሻዎች
- Builtins ቀጭን ሺም ናቸው; አቀናባሪው እንቅስቃሴን እና `SCALL` ለመመዝገብ ዝቅ ያደርጋቸዋል።
ጠቋሚ ገንቢዎች ንፁህ ናቸው፡ ቪኤም በINPUT ውስጥ ያለው Norito TLV ለጥሪው ጊዜ የማይለወጥ መሆኑን ያረጋግጣል።
 - በጠቋሚ-ABI መስኮች (ለምሳሌ `DomainId`, `AccountId`) መዋቅሮች syscall ክርክሮችን ergonomically ለመቧደን ሊያገለግሉ ይችላሉ። የማጠናከሪያው ካርታ `obj.field` ወደ ትክክለኛው መዝገብ/ዋጋ ያለ ተጨማሪ ምደባ።

## ስብስቦች እና ካርታዎች

ዓይነት: `Map<K, V>`
- የማህደረ ትውስታ ካርታዎች (በ `Map::new()` ክምር የተመደበ ወይም እንደ ግቤቶች አልፏል) አንድ ነጠላ ቁልፍ / እሴት ጥንድ ያከማቻል; ቁልፎች እና እሴቶች የቃላት-መጠን አይነት መሆን አለባቸው፡ `int`፣ `bool`፣ `string`፣ `Blob`፣ `bytes`፣ `Json`፣e. `AccountId`፣ `Name`)።
- ዘላቂ የግዛት ካርታዎች (`state Map<...>`) Norito-የተመሰጠሩ ቁልፎችን/እሴቶችን ይጠቀማሉ። የሚደገፉ ቁልፎች: `int` ወይም የጠቋሚ ዓይነቶች. የሚደገፉ እሴቶች፡ `int`፣ `bool`፣ `Json`፣ `Blob`/`bytes`፣ ወይም የጠቋሚ ዓይነቶች።
- `Map::new()` ነጠላ ውስጠ-ትውስታ መግቢያ (ቁልፍ/ዋጋ = 0) ይመድባል እና ዜሮ ያስጀምራል። ለ `Map<int,int>` ካርታዎች ግልጽ የሆነ ማብራሪያ ወይም የመመለሻ አይነት ያቅርቡ።
- የስቴት ካርታዎች የመጀመሪያ ደረጃ እሴቶች አይደሉም: እንደገና መመደብ አይችሉም (ለምሳሌ, `M = Map::new()`); በመረጃ ጠቋሚ (`M[key] = value`) ግቤቶችን ያዘምኑ።
- ተግባራት;
  - መረጃ ጠቋሚ ማድረግ፡- `map[key]` ማግኘት/ማዘጋጀት እሴት (በአስተናጋጅ syscall በኩል ተዘጋጅቷል፤ የአሂድ ጊዜ ኤፒአይ ካርታ ስራን ይመልከቱ)።
  - መኖር፡- `contains(map, key) -> bool` (የወረደ ረዳት፤ ውስጣዊ syscall ሊሆን ይችላል።
  - መደጋገም: `for (k, v) in map { ... }` ከመወሰኛ ቅደም ተከተል እና ሚውቴሽን ህጎች ጋር።

ቆራጥ የመድገም ደንቦች
- የድግግሞሽ ስብስብ በ loop መግቢያ ላይ ያሉ ቁልፎች ቅጽበታዊ እይታ ነው።
- ትዕዛዙ የ Norito-የተመሰጠሩ ቁልፎች በባይት-ሌክሲኮግራፊያዊ ቅደም ተከተል በጥብቅ እየጨመረ ነው።
- በሉፕ ወቅት በተደገፈው ካርታ ላይ የመዋቅር ማሻሻያ (ማስገባት/ማስወገድ/ግልጽ) ቆራጥ የሆነ `E_ITER_MUTATION` ወጥመድ ያስከትላል።
- ወሰን ያስፈልጋል፡- ወይም የተገለጸ ከፍተኛ (`@max_len`) በካርታው ላይ፣ ግልጽ የሆነ ባህሪ `#[bounded(n)]`፣ ወይም `.take(n)`/`.range(..)` በመጠቀም ግልጽ የሆነ ገደብ; አለበለዚያ ማጠናከሪያው `E_UNBOUNDED_ITERATION` ያወጣል.

የታሰሩ ረዳቶች
- `#[bounded(n)]`: በካርታው አገላለጽ ላይ ያለ አማራጭ ባህሪ, ለምሳሌ. `for (k, v) in my_map #[bounded(2)] { ... }`.
- `.take(n)`: ከመጀመሪያው የ `n` ግቤቶችን ይድገሙት።
- `.range(start, end)`: በግማሽ ክፍት ጊዜ `[start, end)` ውስጥ ተደጋጋሚ ግቤቶች። ሴማቲክስ ከ `start` እና `n = end - start` ጋር እኩል ነው።በተለዋዋጭ ድንበሮች ላይ ማስታወሻዎች
- የቃል ድንበሮች፡- `n`፣ `start`፣ እና `end` እንደ ኢንቲጀር ቃል በቃል የተደገፉ እና ወደ ቋሚ የድግግሞሽ ብዛት ያጠናቅራሉ።
- ቃል በቃል ያልሆኑ ድንበሮች፡ የ`kotodama_dynamic_bounds` ባህሪ በ`ivm` ሳጥን ውስጥ ሲነቃ አቀናባሪው ተለዋዋጭ `n`፣ `start`፣ እና `end`ን ይቀበላል `end >= start`)። ተጨማሪ የሰውነት ግድያዎችን ለማስቀረት (ነባሪ K=2) በ`if (i < n)` ቼኮች እስከ ኬ የሚጠበቁ ድግግሞሾችን ዝቅ ማድረግ። K በፕሮግራም በ `CompilerOptions { dynamic_iter_cap, .. }` በኩል ማስተካከል ይችላሉ።
- ከማጠናቀርዎ በፊት `koto_lint` የ Kotodama lint ማስጠንቀቂያዎችን ለመመርመር ያሂዱ; ዋናው ማጠናከሪያ ሁልጊዜ ከመተንተን እና ከተየመፈተሸ በኋላ ወደ ታች በማውረድ ይቀጥላል.
- የስህተት ኮዶች በ [Kotodama Compiler Error Codes] (./kotodama_error_codes.md) ውስጥ ተመዝግበዋል; ለፈጣን ማብራሪያዎች `koto_compile --explain <code>` ይጠቀሙ።

## ስህተቶች እና ምርመራዎች

የተጠናከረ ጊዜ ምርመራዎች (ምሳሌዎች)
- `E_UNBOUNDED_ITERATION`፡ loop over map ወሰን የለውም።
- `E_MUT_DURING_ITER`: በ loop አካል ውስጥ የተደጋገመ ካርታ መዋቅራዊ ሚውቴሽን።
- `E_STATE_SHADOWED`: የአካባቢ ማያያዣዎች የ`state` መግለጫዎችን ጥላ ሊሆኑ አይችሉም።
- `E_BREAK_OUTSIDE_LOOP`: `break` ጥቅም ላይ የዋለው ከሉፕ ውጭ ነው።
- `E_CONTINUE_OUTSIDE_LOOP`: `continue` ጥቅም ላይ የዋለው ከሉፕ ውጭ ነው።
- `E0005`: ለ-loop ማስጀመሪያ ከሚደገፈው የበለጠ የተወሳሰበ ነው።
- `E0006`፡ ለ-loop የእርምጃ አንቀጽ ከሚደገፈው የበለጠ የተወሳሰበ ነው።
- `E_BAD_POINTER_USE`: የአንደኛ ደረጃ ዓይነት የሚፈለግበት የጠቋሚ-ABI ገንቢ ውጤትን በመጠቀም።
- `E_UNRESOLVED_NAME`፣ `E_TYPE_MISMATCH`፣ `E_ARITY_MISMATCH`፣ `E_DUP_SYMBOL`።
- Tooling: `koto_compile` ባይት ኮድ ከማውጣቱ በፊት የሊንት ማለፊያውን ያካሂዳል; ለመዝለል `--no-lint` ይጠቀሙ ወይም `--deny-lint-warnings` በ lint ውፅዓት ላይ ያለውን ግንባታ ለመክሸፍ ይጠቀሙ።

የአሂድ ጊዜ ቪኤም ስህተቶች (የተመረጡ፣ ሙሉ ዝርዝር በivm.md)
- `E_NORITO_INVALID`፣ `E_OOB`፣ `E_UNALIGNED`፣ `E_SCALL_UNKNOWN`፣ `E_ASSERT`፣ `E_ASSERT_EQ`፣ `E_ITER_MUTATION`.

የስህተት መልዕክቶች
- ዲያግኖስቲክስ በ `kotoba {}` የትርጉም ሠንጠረዦች ውስጥ በሚገኝበት ጊዜ በካርታው ላይ የተረጋጋ `msg_id`s ይይዛል።

## Codegen ካርታ ወደ IVM

የቧንቧ መስመር
1. Lexer/parser AST ያመርታሉ።
2. የትርጉም ትንተና ስሞችን ይፈታል፣ አይነቶችን ይፈትሻል እና የምልክት ሠንጠረዦችን ይሞላል።
3. IR ወደ ቀላል SSA መሰል ቅፅ ዝቅ ማድረግ።
4. ለ IVM GPRs (`r10+` ለ args / ret በአንድ ጥሪ ስምምነት) ላይ ምደባን መመዝገብ; ለመደርደር መፍሰስ.
5. የባይቴኮድ ልቀት፡ እንደተፈቀደው የIVM-ቤተኛ እና የ RV-compat ኢንኮዲንግ ድብልቅ; ከ`abi_version`፣ ባህሪያት፣ የቬክተር ርዝመት እና `max_cycles` ጋር የተለቀቀ ሜታዳታ ራስጌ።የካርታ ስራዎች ድምቀቶች
- አርቲሜቲክ እና ሎጂክ ካርታ ወደ IVM ALU ops።
- የቅርንጫፍ እና የቁጥጥር ካርታ ወደ ሁኔታዊ ቅርንጫፎች እና መዝለሎች; አቀናባሪው ትርፋማ በሚሆንበት ጊዜ የታመቁ ቅጾችን ይጠቀማል።
- ለአካባቢው ነዋሪዎች ማህደረ ትውስታ ወደ VM ቁልል ይፈስሳል; አሰላለፍ ተፈጻሚ ነው።
- እንቅስቃሴዎችን ለመመዝገብ ዝቅተኛ ግንባታዎች እና `SCALL` ከ 8-ቢት ቁጥር ጋር።
ጠቋሚ አዘጋጆች Norito TLVs ወደ INPUT ክልል ያስቀምጣሉ እና አድራሻቸውን ያዘጋጃሉ።
- የZK አፈፃፀም ላይ ወጥመድ እና ገደቦችን የሚያመነጨው ለ`ASSERT`/`ASSERT_EQ` ማረጋገጫ ካርታ።

ቆራጥነት ገደቦች
- ኤፍፒ የለም; ምንም የማይወስኑ ስኪሎች የሉም።
- የሲምዲ/ጂፒዩ ማጣደፍ ለባይትኮድ የማይታይ ነው እና ትንሽ ተመሳሳይ መሆን አለበት። ኮምፕሌተር ሃርድዌር-ተኮር ኦፕስ አያወጣም።

## ኤቢአይ፣ ራስጌ እና አንጸባራቂ

IVM ራስጌ መስኮች በአቀነባባሪው ተቀናብረዋል
- `version`: IVM ባይትኮድ ቅርጸት ስሪት (major.minor).
- `abi_version`: syscall ሠንጠረዥ እና ጠቋሚ-ABI ንድፍ ስሪት.
- `feature_bits`፡ የባህሪ ባንዲራዎች (ለምሳሌ፡ `ZK`፣ `VECTOR`)።
- `vector_len`: ምክንያታዊ የቬክተር ርዝመት (0 → ያልተቀናበረ).
- `max_cycles`፡ የመግቢያ ገደብ እና የ ZK ንጣፍ ፍንጭ።

አንጸባራቂ (አማራጭ የጎን መኪና)
- `code_hash`፣ `abi_hash`፣ ሜታዳታ ከ`meta {}` ብሎክ፣ የማጠናከሪያ ሥሪት፣ እና ለመራባት ፍንጮችን ይገንቡ።

## የመንገድ ካርታ

- **KD-231 (ኤፕሪል 2026):** ለድግግሞሽ ድንበሮች የማጠናቀር-የጊዜ ክልል ትንታኔን ይጨምሩ ስለዚህ ቀለበቶች የተገደቡ መዳረሻ ስብስቦችን ወደ መርሐግብር አውጪው ያጋልጣሉ።
- **KD-235 (ግንቦት2026):** ለጠቋሚ አዘጋጆች እና ABI ግልጽነት ከ `string` የተለየ የአንደኛ ደረጃ `bytes` scalar አስተዋውቅ።
- **KD-242 (ጁን2026):** አብሮ የተሰራውን የኦፕኮድ ስብስብ (ሃሽ/ፊርማ ማረጋገጫ) ከባህሪ ባንዲራዎች በስተጀርባ አስፋፊ ጥፋቶች።
- **KD-247 (Jun2026):** ስህተትን `msg_id`s ማረጋጋት እና ካርታውን በ`kotoba {}` ሰንጠረዦች ለአካባቢያዊ ምርመራ ማቆየት።
### ልቀት ያሳያል

- የ Kotodama ማጠናከሪያ ኤፒአይ `ContractManifest` ከተቀናበረው `.to` በ `ivm::kotodama::compiler::Compiler::compile_source_with_manifest` በኩል መመለስ ይችላል።
- መስኮች:
  - `code_hash`፡ ቅርሱን ለማሰር በአቀነባባሪው የተሰላ የኮድ ባይት ሃሽ (ከIVM አርዕስት እና ቃል በቃል)።
  - `abi_hash`፡ ለፕሮግራሙ `abi_version` የሚፈቀደው የሲሲካል ወለል የተረጋጋ መፍጨት (`ivm.md` እና `ivm::syscalls::compute_abi_hash` ይመልከቱ)።
- አማራጭ `compiler_fingerprint` እና `features_bitmap` ለመሳሪያ ሰንሰለት የተጠበቁ ናቸው።
- `entrypoints`፡ የታዘዙ ወደ ውጭ የሚላኩ የመግቢያ ነጥቦች ዝርዝር (ይፋዊ፣ `hajimari`፣ `kaizen`) የሚፈለጉትን `permission(...)` ሕብረቁምፊዎች እና የአቀናባሪው ምርጥ ጥረት የማንበብ/የመፃፍ ቁልፍ ፍንጮች እና WS ለመግባት የሚጠበቁትን ቁልፍ ፍንጮችን ጨምሮ።
- አንጸባራቂው ለመግቢያ-ጊዜ ቼኮች እና ለመመዝገቢያዎች የታሰበ ነው; ለሕይወት ዑደት `docs/source/new_pipeline.md` ይመልከቱ።