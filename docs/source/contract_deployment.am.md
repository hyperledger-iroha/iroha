---
lang: am
direction: ltr
source: docs/source/contract_deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f2b1d7d027d715eac5a3ca8be29dea8f0e76013e948947a4de66108ac561f34
source_last_modified: "2026-01-22T14:58:53.689594+00:00"
translation_last_reviewed: 2026-02-07
title: Contract Deployment (.to) — API & Workflow
translator: machine-google-reviewed
---

ሁኔታ፡ በTorii፣ CLI እና ዋና የመግቢያ ፈተናዎች (ህዳር 2025) የተተገበረ እና ተግባራዊ የተደረገ።

## አጠቃላይ እይታ

- የተጠናቀረው IVM ባይትኮድ (`.to`) ወደ Torii በማስገባት ወይም በማውጣት ያሰማሩ
  `RegisterSmartContractCode`/`RegisterSmartContractBytes` መመሪያዎች
  በቀጥታ.
- አንጓዎች `code_hash` እና ቀኖናዊውን ABI hash በአካባቢው እንደገና ይሰላሉ; አለመመጣጠን
  በቆራጥነት እምቢ ማለት.
- የተከማቹ ቅርሶች በሰንሰለት ስር ይኖራሉ `contract_manifests` እና
  `contract_code` መዝገቦች. የማጣቀሻ ሃሽዎችን ብቻ ያሳያል እና ትንሽ ሆኖ ይቆያል;
  ኮድ ባይት በ `code_hash` ተቆልፏል።
- የተጠበቁ የስም ቦታዎች ከሀ በፊት የፀደቀ የአስተዳደር ፕሮፖዛል ሊያስፈልጋቸው ይችላል።
  ማሰማራት ገብቷል. የመግቢያ ዱካ የፕሮፖዛል ክፍያን እና ይመለከታል
  የ `(namespace, contract_id, code_hash, abi_hash)` እኩልነትን ያስፈጽማል
  የስም ቦታ የተጠበቀ ነው።

## የተከማቹ ቅርሶች እና ማቆየት።

- `RegisterSmartContractCode` ያስገባዋል / አንድ የተሰጠ አንጸባራቂ ይጽፋል
  `code_hash`. ተመሳሳዩ ሃሽ ቀድሞውኑ ሲኖር, በአዲሱ ይተካል
  ገላጭ.
- `RegisterSmartContractBytes` የተጠናቀረውን ፕሮግራም ስር ያከማቻል
  `contract_code[code_hash]`. ለሃሽ ባይት አስቀድሞ ካለ እነሱ መዛመድ አለባቸው
  በትክክል; የተለያዩ ባይት የማይለዋወጥ ጥሰትን ያሳድጋል።
- የኮድ መጠን በብጁ መለኪያ `max_contract_code_bytes` ተሸፍኗል
  (ነባሪ 16 ሚቢ)። ከዚህ በፊት በ`SetParameter(Custom)` ግብይት ይሽሩት
  ትላልቅ ቅርሶችን መመዝገብ.
- ማቆየት ያልተገደበ ነው፡ መግለጫዎች እና ኮድ በግልጽ እስከሚገኙ ድረስ ይገኛሉ
  በወደፊት የአስተዳደር የስራ ሂደት ውስጥ ተወግዷል. ቲቲኤል ወይም አውቶማቲክ ጂሲ የለም።

## የመግቢያ መስመር

- አረጋጋጩ የIVM ራስጌን ይተነትናል፣ `version_major == 1`ን ያስፈጽማል እና ይፈትሻል
  `abi_version == 1`. ያልታወቁ ስሪቶች ወዲያውኑ ውድቅ ያደርጋሉ; የሩጫ ጊዜ የለም።
  ቀያይር።
- ለ`code_hash` አንጸባራቂ ሲገኝ፣ ማረጋገጫው ያረጋግጣል
  የተከማቸ `code_hash`/`abi_hash` ከተሰሉት እሴቶች ጋር እኩል ነው።
  ፕሮግራም. አለመመጣጠን የ`Manifest{Code,Abi}HashMismatch` ስህተቶችን ይፈጥራል።
- የተጠበቁ የስም ቦታዎችን ያነጣጠሩ ግብይቶች የሜታዳታ ቁልፎችን ማካተት አለባቸው
  `gov_namespace` እና `gov_contract_id`። የመግቢያ መንገዱ ያነፃፅራቸዋል።
  በ `DeployContract` ፕሮፖዛል ላይ; ተዛማጅ ፕሮፖዛል ከሌለ እ.ኤ.አ
  ግብይት በ `NotPermitted` ውድቅ ተደርጓል።

## Torii የመጨረሻ ነጥቦች (ባህሪ `app_api`)- `POST /v2/contracts/deploy`
  - የጥያቄ አካል፡ `DeployContractDto` (የመስክ ዝርዝሮችን ለማግኘት `docs/source/torii_contracts_api.md` ይመልከቱ)።
  - Torii ቤዝ64 ክፍያን መፍታት፣ ሁለቱንም ሃሽ ያሰላል፣ አንጸባራቂ ይሠራል፣
    እና `RegisterSmartContractCode` ፕላስ ያቀርባል
    `RegisterSmartContractBytes` በመወከል በተፈረመ ግብይት ውስጥ
    ደዋይ
  - ምላሽ: `{ ok, code_hash_hex, abi_hash_hex }`.
  - ስህተቶች፡ ልክ ያልሆነ ቤዝ64፣ የማይደገፍ የ ABI ስሪት፣ ፍቃድ ይጎድላል
    (`CanRegisterSmartContractCode`)፣ የመጠን ካፕ ታልፏል፣ የአስተዳደር መግቢያ።
- `POST /v2/contracts/code`
  - `RegisterContractCodeDto` (ባለስልጣን ፣ የግል ቁልፍ ፣ መግለጫ) ይቀበላል እና ብቻ ያቀርባል
    `RegisterSmartContractCode`. አንጸባራቂዎች ተለይተው ሲዘጋጁ ይጠቀሙ
    ባይትኮድ
- `POST /v2/contracts/instance`
  - `DeployAndActivateInstanceDto` ይቀበላል (ስልጣን ፣ የግል ቁልፍ ፣ የስም ቦታ/ኮንትራት_id ፣ `code_b64` ፣ አማራጭ አንጸባራቂ ይሽራል) እና ያሰማራው + በአቶሚካል ያነቃል።
- `POST /v2/contracts/instance/activate`
  - `ActivateInstanceDto` (ባለስልጣን ፣ የግል ቁልፍ ፣ የስም ቦታ ፣ contract_id ፣ `code_hash`) ይቀበላል እና የማግበር መመሪያውን ብቻ ያቀርባል።
- `GET /v2/contracts/code/{code_hash}`
  - `{ manifest: { code_hash, abi_hash } }` ይመልሳል።
    ተጨማሪ አንጸባራቂ መስኮች ከውስጥ ተጠብቀው ይገኛሉ ነገር ግን እዚህ ለሀ
    የተረጋጋ API.
- `GET /v2/contracts/code-bytes/{code_hash}`
  - `{ code_b64 }` በተከማቸ `.to` ምስል እንደ base64 ኮድ ይመልሳል።

ሁሉም የኮንትራት የህይወት ኡደት የመጨረሻ ነጥቦች በ በኩል የተዋቀረ የተወሰነ የማሰማራት ገደብ ያጋራሉ።
`torii.deploy_rate_per_origin_per_sec` (ቶከኖች በሰከንድ) እና
`torii.deploy_burst_per_origin` (ፍንዳታ ቶከኖች)። ነባሪዎች 4 req/s ከፍንዳታ ጋር ናቸው።
8 ለእያንዳንዱ ማስመሰያ/ቁልፍ ከ`X-API-Token`፣ ከርቀት አይፒ ወይም የመጨረሻ ነጥብ ፍንጭ የተገኘ።
የታመኑ ኦፕሬተሮችን ገደብ ለማሰናከል የትኛውንም መስክ ወደ `null` ያቀናብሩ። መቼ
ገዳይ እሳቶች፣ Torii ይጨምራል
`torii_contract_throttled_total{endpoint="code|deploy|instance|activate"}` ቴሌሜትሪ ቆጣሪ እና
HTTP 429 ይመልሳል; ማንኛውም ተቆጣጣሪ ስህተት ይጨምራል
`torii_contract_errors_total{endpoint=…}` ለማስጠንቀቅ።

## የአስተዳደር ውህደት እና የተጠበቁ የስም ቦታዎች- ብጁ መለኪያውን `gov_protected_namespaces` (የ JSON የስም ቦታ ድርድር ያቀናብሩ)
  ሕብረቁምፊዎች) የመግቢያ ጋቲንግን ለማንቃት። Torii ረዳቶችን ያጋልጣል
  `/v2/gov/protected-namespaces` እና CLI በእነርሱ በኩል ያንጸባርቃሉ
  `iroha_cli app gov protected set` / `iroha_cli app gov protected get`.
- በ `ProposeDeployContract` (ወይም በ Torii የተፈጠሩ ሀሳቦች)
  `/v2/gov/proposals/deploy-contract` የመጨረሻ ነጥብ) ቀረጻ
  `(namespace, contract_id, code_hash, abi_hash, abi_version)`.
- ህዝበ ውሳኔው አንዴ ካለፈ፣ `EnactReferendum` ፕሮፖዛሉ ተቀባይነት ያለው እና
  መግባቱ ተዛማጅ ሜታዳታ እና ኮድ የያዙ ማሰማራቶችን ይቀበላል።
- ግብይቶች የሜታዳታ ጥንድ `gov_namespace=a namespace` እና ማካተት አለባቸው
  `gov_contract_id=an identifier` (እና `contract_namespace` ማዘጋጀት አለበት /
  `contract_id` ለጥሪ ጊዜ ማሰሪያ)። የ CLI ረዳቶች እነዚህን ይሞላሉ።
  በራስ-ሰር `--namespace`/`--contract-id` ሲያልፉ።
- የተጠበቁ የስም ቦታዎች ሲነቁ ወረፋ መግባት ሙከራዎችን ውድቅ ያደርጋል
  ያለውን `contract_id` ወደ ሌላ የስም ቦታ እንደገና ማያያዝ; የወጣውን ተጠቀም
  ወደ ሌላ ቦታ ከመሰማራቱ በፊት የቀደመውን አስገዳጅ ሀሳብ ያቅርቡ ወይም ጡረታ ይውጡ።
- የሌይን አንጸባራቂ አረጋጋጭ ምልአተ ጉባኤ ከአንድ በላይ ካዘጋጀ ያካትቱ
  `gov_manifest_approvers` (የ JSON ድርድር አረጋጋጭ መለያ መታወቂያዎች) ስለዚህ ወረፋው ሊቆጠር ይችላል
  ከግብይቱ ባለስልጣን ጋር ተጨማሪ ማጽደቆች. መስመሮችም አይቀበሉም።
  በአንጸባራቂው ውስጥ የማይገኙ የስም ቦታዎችን የሚጠቅስ ሜታዳታ
  `protected_namespaces` ስብስብ።

## CLI ረዳቶች

- `iroha_cli app contracts deploy --authority <id> --private-key <hex> --code-file <path>`
  የTorii ማሰማራት ጥያቄን (በበረራ ላይ በማስላት ላይ) ያቀርባል።
- `iroha_cli app contracts deploy-activate --authority <id> --private-key <hex> --namespace <ns> --contract-id <id> --code-file <path>`
  አንጸባራቂውን ይገነባል (በቀረበው ቁልፍ የተፈረመ)፣ ባይት + አንጸባራቂ ይመዘግባል፣
  እና `(namespace, contract_id)` ማሰሪያን በአንድ ግብይት ያነቃል። ተጠቀም
  `--dry-run` የተሰላውን ሃሽ እና የመመሪያ ቆጠራን ያለሱ ለማተም
  በማስገባት ላይ፣ እና `--manifest-out` የተፈረመውን አንጸባራቂ JSON ለማስቀመጥ።
- `iroha_cli app contracts manifest build --code-file <path> [--sign-with <hex>]` ያሰላል።
  `code_hash`/`abi_hash` ለተጠናቀረ `.to` እና እንደ አማራጭ ማኒፌክትን ይፈርማል፣
  JSON ማተም ወይም ወደ `--out` መፃፍ።
- `iroha_cli app contracts simulate --authority <id> --private-key <hex> --code-file <path> --gas-limit <u64>`
  ከመስመር ውጭ የVM ማለፊያ ያስኬዳል እና ኤቢአይ/ሃሽ ዲበዳታ እና የተሰለፉትን አይኤስአይኤስ ሪፖርት ያደርጋል
  (ቆጠራዎች እና የመመሪያ መታወቂያዎች) አውታረ መረቡን ሳይነኩ. ያያይዙ
  `--namespace/--contract-id` የጥሪ ጊዜ ዲበ ውሂብን ለማንጸባረቅ።
- `iroha_cli app contracts manifest get --code-hash <hex>` አንጸባራቂውን በTorii በኩል ያመጣል
  እና በአማራጭ ወደ ዲስክ ይጽፋል.
- `iroha_cli app contracts code get --code-hash <hex> --out <path>` ውርዶች
  የተቀመጠው `.to` ምስል.
- `iroha_cli app contracts instances --namespace <ns> [--table]` ዝርዝሮች ነቅተዋል።
  የኮንትራት ሁኔታዎች (አንጸባራቂ + ሜታዳታ የሚነዳ)።
- የአስተዳደር ረዳቶች (`iroha_cli app gov deploy propose`፣ `iroha_cli app gov enact`፣
  `iroha_cli app gov protected set/get`) የተጠበቀውን የስም ቦታ የስራ ፍሰት ያቀናጃል እና
  የJSON ቅርሶችን ለኦዲት ማጋለጥ።

## ሙከራ እና ሽፋን

- ክፍል `crates/iroha_core/tests/contract_code_bytes.rs` ሽፋን ኮድ ስር ሙከራዎች
  ማከማቻ፣ idempotency እና የመጠን ቆብ።
- `crates/iroha_core/tests/gov_enact_deploy.rs` አንጸባራቂ ማስገባትን ያረጋግጣል
  ማስፈጸሚያ፣ እና `crates/iroha_core/tests/gov_protected_gate.rs` መልመጃዎች
  የተጠበቀ-ስም ቦታ መግቢያ ከጫፍ እስከ ጫፍ።
- Torii መንገዶች የጥያቄ/የምላሽ ክፍል ሙከራዎችን ያካትታሉ፣ እና የCLI ትዕዛዞች አሏቸው።
  የJSON ዙር ጉዞዎች የተረጋጋ መሆናቸውን የሚያረጋግጡ የውህደት ሙከራዎች።

ለዝርዝር የሪፈረንደም ክፍያ እና ለ `docs/source/governance_api.md` ይመልከቱ
የድምጽ መስጫ የስራ ፍሰቶች.