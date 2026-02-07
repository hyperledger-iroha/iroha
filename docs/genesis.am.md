---
lang: am
direction: ltr
source: docs/genesis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c2eab4379aa346ab7d111e1c51c0230238f260647187f1a33c1819640b9bf2c
source_last_modified: "2026-01-28T14:25:37.056140+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የዘፍጥረት ውቅር

የ`genesis.json` ፋይል የIroha አውታረ መረብ ሲጀመር የሚከናወኑትን የመጀመሪያ ግብይቶች ይገልጻል። ፋይሉ ከነዚህ መስኮች ጋር የJSON ነገር ነው፡-

- `chain` - ልዩ ሰንሰለት መለያ።
- `executor` (አማራጭ) - ወደ አስፈፃሚ ባይትኮድ (`.to`) መንገድ። ካለ፣
  ዘፍጥረት የማሻሻያ መመሪያን እንደ መጀመሪያው ግብይት ያካትታል። ከተተወ፣
  ምንም ማሻሻያ አልተደረገም እና አብሮገነብ ፈጻሚው ጥቅም ላይ ይውላል።
- `ivm_dir` - I18NT0000023X ባይትኮድ ቤተ-መጻሕፍት የያዘ ማውጫ። ከተተወ የ I18NI0000041X ነባሪዎች።
- `consensus_mode` - የጋራ ስምምነት ሁነታ በማንፀባረቂያው ውስጥ ማስታወቂያ ገብቷል። የሚያስፈልግ; `"Npos"` ለህዝብ Sora I18NT0000020X የመረጃ ቦታ፣ ወይም `"Permissioned"`/`"Npos"` ለሌሎች Iroha3 የመረጃ ቦታዎች ይጠቀሙ። Iroha2 ነባሪዎች ለ `"Permissioned"`።
- `transactions` - በቅደም ተከተል የተከናወኑ የዘፍጥረት ግብይቶች ዝርዝር። እያንዳንዱ ግቤት የሚከተሉትን ሊይዝ ይችላል
  - `parameters` - የመጀመሪያ አውታረ መረብ መለኪያዎች.
  - `instructions` - የተዋቀረ I18NT0000002X መመሪያዎች (ለምሳሌ `{ "Register": { "Domain": { "id": "wonderland" }}}`)። ጥሬ ባይት ድርድሮች ተቀባይነት የላቸውም፣ እና የ`SetParameter` መመሪያዎች እዚህ ውድቅ ተደርገዋል—የዘር መለኪያዎች በ`parameters` አግድ እና መደበኛ ማድረግ/መፈረም መመሪያዎቹን እንዲያስገባ ያድርጉ።
  - `ivm_triggers` - ቀስቅሴዎች በ I18NT0000024X ባይትኮድ ፈጻሚዎች።
  - `topology` - የመጀመሪያ አቻ ቶፖሎጂ። እያንዳንዱ ግቤት የአቻ መታወቂያውን እና ፖፕን አንድ ላይ ያቆያል፡ `{ "peer": "<public_key>", "pop_hex": "<hex>" }`። `pop_hex` በማቀናበር ላይ እያለ ሊቀር ይችላል፣ግን ከመፈረሙ በፊት መገኘት አለበት።
- `crypto` - ከ`iroha_config.crypto` (`default_hash`፣ `allowed_signing`፣ `allowed_curve_ids`፣ `sm2_distid_default`፣010000058X፣`sm2_distid_default`፣010000062X፣010000062X፣010000062X፣010000062X፣000000062X፣010000062X፣010000062X፣010000062X፣010000062X፣010000062X. `allowed_curve_ids` መስተዋቶች `crypto.curves.allowed_curve_ids` ስለዚህ አንጸባራቂዎች የትኛውን ተቆጣጣሪ ኩርባዎች ክላስተር እንደሚቀበል ያስተዋውቃል። የመሳሪያ አሠራር የኤስኤም ውህዶችን ያስፈጽማል፡ ዝርዝር `sm2` ደግሞ ሃሽ ወደ `sm3-256` መቀየር አለበት፣ግንቦች ግን ያለ `sm` ባህሪይ `sm2`ን ሙሉ በሙሉ ውድቅ ያደርጋሉ። መደበኛነት I18NI0000070X ብጁ ግቤት በተፈረመው ዘፍጥረት ውስጥ ያስገባል; የተወጋው ጭነት ከማስታወቂያው ቅጽበታዊ ገጽ እይታ ጋር ካልተስማማ አንጓዎች ለመጀመር ፈቃደኛ አይደሉም።

ምሳሌ (`kagami genesis generate default --consensus-mode npos` ውፅዓት፣ የተከረከመ መመሪያ)፡

```json
{
  "chain": "00000000-0000-0000-0000-000000000000",
  "ivm_dir": "defaults",
  "transactions": [
    {
      "parameters": { "sumeragi": { "block_time_ms": 2000 } },
      "instructions": [
        { "Register": { "Domain": { "id": "wonderland" } } }
      ],
      "ivm_triggers": [],
      "topology": [
        {
          "peer": "ed25519:...",
          "pop_hex": "ab12cd..."
        }
      ]
    }
  ],
  "consensus_mode": "Npos",
  "crypto": {
    "default_hash": "blake2b-256",
    "allowed_signing": ["ed25519", "secp256k1"],
    "allowed_curve_ids": [1],
    "sm2_distid_default": "1234567812345678",
    "sm_openssl_preview": false
  }
}
```

### ለ SM2/SM3 የ `crypto` ብሎክ ዘር

የቁልፍ ክምችት እና ለመለጠፍ ዝግጁ የሆነ የውቅር ቅንጣቢ በአንድ እርምጃ ለማምረት የ xtask አጋዥን ይጠቀሙ፡-

```bash
cargo xtask sm-operator-snippet \
  --distid CN12345678901234 \
  --json-out sm2-key.json \
  --snippet-out client-sm2.toml
```

`client-sm2.toml` አሁን ይዟል፡

```toml
# Account key material
public_key = "sm2:8626530010..."
private_key = "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569"
# public_key_pem = """\
-----BEGIN PUBLIC KEY-----
...
-----END PUBLIC KEY-----
"""
# private_key_pem = """\
-----BEGIN PRIVATE KEY-----
...
-----END PRIVATE KEY-----
"""

[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "secp256k1", "sm2"]  # remove "sm2" to stay in verify-only mode
allowed_curve_ids = [1]               # add new curve ids (e.g., 15 for SM2) when controllers are allowed
sm2_distid_default = "CN12345678901234"
# enable_sm_openssl_preview = true  # optional: only when deploying the OpenSSL/Tongsuo path
```

የ`public_key`/`private_key` እሴቶችን ወደ መለያ/ደንበኛ ውቅር ይቅዱ እና የ`crypto` ብሎክን ያዘምኑ የ`genesis.json` ስለዚህ ከቅንጭቱ ጋር ይዛመዳል (ለምሳሌ I18NI000000078909090909090000000000090909090009090909090909090909090909090909000909000909090900000000000000000000000000000000000000000000000000000000000000000000000000000000000000000076X block `"sm2"` ወደ `allowed_signing`፣ እና ትክክለኛውን I18NI0000082X ያካትቱ። Kagami የሃሽ/ከርቭ ቅንጅቶች እና የመፈረሚያ ዝርዝሩ የማይጣጣሙበትን አንጸባራቂ ያሳያል።

> ** ጠቃሚ ምክር:** ውጤቱን ብቻ ለመመርመር ሲፈልጉ በ `--snippet-out -` ወደ stdout ቅንጣቢውን ይልቀቁ። ቁልፉን በ stdout ላይም ለመልቀቅ `--json-out -` ይጠቀሙ።

ዝቅተኛ ደረጃ CLI ትዕዛዞችን በእጅ መንዳት ከመረጡ፣ ተመጣጣኝ ፍሰቱ፡-

```bash
# 1. Produce deterministic key material (writes JSON to disk)
cargo run -p iroha_cli --features sm -- \
  crypto sm2 keygen \
  --distid CN12345678901234 \
  --output sm2-key.json

# 2. Re-hydrate the snippet that can be pasted into client/config files
cargo run -p iroha_cli --features sm -- \
  crypto sm2 export \
  --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
  --distid CN12345678901234 \
  --snippet-output client-sm2.toml \
  --emit-json --quiet
```

> ** ጠቃሚ ምክር፡** I18NI0000085X በእጅ ቅጂ/መለጠፍ ደረጃን ለማስቀመጥ ከላይ ጥቅም ላይ ይውላል። የማይገኝ ከሆነ `sm2-key.json` ይክፈቱ፣የ`private_key_hex` መስኩን ይቅዱ እና በቀጥታ ወደ `crypto sm2 export` ያስተላልፉ።

> **የስደት መመሪያ፡** ያለውን ኔትወርክ ወደ SM2/SM3/SM4 ሲቀይሩ ይከተሉ
> [`docs/source/crypto/sm_config_migration.md`](source/crypto/sm_config_migration.md)
> ለተደራራቢ I18NI0000090X መሻር፣ አንጸባራቂ መታደስ እና መልሶ መመለስ
> እቅድ ማውጣት።

## ያመንጩ እና ያረጋግጡ

1. አብነት ይፍጠሩ፡
   ```bash
   cargo run -p iroha_kagami -- genesis generate \
     [--executor <path/to/executor.to>] \
     --consensus-mode npos \
     --ivm-dir <ivm/dir> \
     --genesis-public-key <PUBLIC_KEY> > genesis.json
   ```
`--consensus-mode` የ Kagami ዘሮች ወደ I18NI0000092X ብሎክ ውስጥ የትኞቹን የጋራ መግባቢያ መለኪያዎች ይቆጣጠራል። ይፋዊው የሶራ I18NT0000021X የመረጃ ቦታ `npos` ይፈልጋል እና የተደረደሩ መቁረጫዎችን አይደግፍም። ሌሎች የIroha3 ዳታ ቦታዎች የተፈቀደ ወይም NPoS ሊጠቀሙ ይችላሉ። Iroha2 ወደ I18NI0000094X ነባሪዎች እና `npos` በ I18NI0000096X/I18NI0000097X ደረጃ ሊያደርገው ይችላል። `npos` ሲመረጥ፣ Kagami የ NPoS ሰብሳቢ አድናቂ-ውጭን፣ የምርጫ ፖሊሲን እና የማዋቀር መስኮቶችን የሚያንቀሳቅሰውን የ`sumeragi_npos_parameters` ክፍያ ይዘራል። መደበኛ ማድረግ/ መፈረም በተፈረመው እገዳ ውስጥ ወደ `SetParameter` መመሪያዎች ይቀይራቸዋል።
2. በአማራጭ `genesis.json` ያርትዑ፣ ከዚያ ያረጋግጡ እና ይፈርሙ፡
   ```bash
   cargo run -p iroha_kagami -- genesis sign genesis.json \
     --public-key <PUBLIC_KEY> \
     --private-key <PRIVATE_KEY> \
     --out-file genesis.signed.nrt
   ```

   SM2/SM3/SM4-ዝግጁ መገለጫዎችን ለመልቀቅ `--default-hash sm3-256` ን ማለፍ እና `--allowed-signing sm2`ን ጨምር (ለተጨማሪ ስልተ ቀመሮች `--allowed-signing` ድገም)። ነባሪውን መለያ መለያ መሻር ከፈለጉ `--sm2-distid-default <ID>` ይጠቀሙ።

   `irohad`ን በ `--genesis-manifest-json` ብቻ (ምንም የተፈረመ የዘረመል እገዳ) ሲጀምሩ መስቀለኛ መንገዱ አሁን የሩጫ ጊዜውን crypto ውቅር ከማንፀባረቂያው በቀጥታ ይዘራል፤ የዘረመል ብሎክ ካቀረብክ አንጸባራቂው እና አወቃቀሩ አሁንም በትክክል መመሳሰል አለባቸው።

- የማረጋገጫ ማስታወሻዎች;
  - Kagami `consensus_handshake_meta`፣ `confidential_registry_root`፣ እና `crypto_manifest_meta` እንደ `SetParameter` መመሪያዎችን በተለመደው/በተፈረመ ብሎክ ያስገባል። `irohad` የመጨባበጥ ሜታዳታ ወይም ክሪፕቶ ቅጽበታዊ ገጽ እይታ ከተቀመጡት መለኪያዎች ጋር ካልተስማሙ ከተጫኑት ጭነቶች የተገኘውን የጋራ ስምምነት አሻራ እንደገና ያሰላል እና ጅምር አይሳካም። እነዚህን ከ `instructions` ውስጥ በማንፀባረቅ ያቆዩዋቸው; እነሱ በራስ-ሰር ይፈጠራሉ.
- መደበኛውን እገዳ ይፈትሹ;
  - የመጨረሻውን የታዘዙ ግብይቶች ለማየት `kagami genesis normalize genesis.json --format text` ን ያሂዱ (የተከተተ ሜታዳታ ጨምሮ) የቁልፍ ጥንድ ሳያቀርቡ።
  - ለልዩነት ወይም ለግምገማዎች ተስማሚ የሆነ የተዋቀረ እይታን ለመጣል `--format json` ይጠቀሙ።

`kagami genesis sign` JSON የሚሰራ መሆኑን ያረጋግጣል እና Norito-የተመሰጠረ ብሎክ በመስቀለኛ ውቅረት ውስጥ በ`genesis.file` ለመጠቀም ዝግጁ ያደርጋል። የተገኘው `genesis.signed.nrt` አስቀድሞ በቀኖናዊ ሽቦ መልክ ነው፡ ስሪት ባይት የተከተለው በI18NT0000004X ራስጌ የክፍያ ጭነት አቀማመጥን የሚገልጽ ነው። ሁልጊዜ ይህንን ፍሬም ያሰራጩ። ለተፈረሙ የክፍያ ጭነቶች የ `.nrt` ቅጥያ ይምረጡ። ፈጻሚውን በዘፍጥረት ማሻሻል ካላስፈለገዎት የ`executor` መስኩን ትተው የ`.to` ፋይልን መዝለል ይችላሉ።

NPoS መገለጫዎች (`--consensus-mode npos` ወይም Iroha2-ብቻ ደረጃ ያላቸው መቁረጫዎች) ሲፈርሙ, `kagami genesis sign` `sumeragi_npos_parameters` ክፍያ ያስፈልገዋል; በ `kagami genesis generate --consensus-mode npos` ያመነጫል ወይም መለኪያውን በእጅ ያክሉት።
በነባሪ፣ `kagami genesis sign` የማኒፌክትን `consensus_mode` ይጠቀማል። እሱን ለመሻር `--consensus-mode` ማለፍ።

## ዘፍጥረት ምን ሊያደርግ ይችላል።

ዘፍጥረት የሚከተሉትን ስራዎች ይደግፋል. Kagami በደንብ በተገለጸ ቅደም ተከተል ወደ ግብይቶች ያሰባስባቸዋል ስለዚህ እኩዮች ተመሳሳይ ቅደም ተከተሎችን በቆራጥነት ይፈጽማሉ።

- መለኪያዎች፡ ለSumeragi (የማገድ/የማገድ ጊዜ፣ ተንሸራታች)፣ አግድ (ከፍተኛ txs)፣ ግብይት (ከፍተኛ መመሪያዎች፣ ባይትኮድ መጠን)፣ አስፈፃሚ እና ስማርት ኮንትራት ገደቦች (ነዳጅ፣ ማህደረ ትውስታ፣ ጥልቀት) እና ብጁ መለኪያዎችን ያቀናብሩ። Kagami ዘሮች `Sumeragi::NextMode` እና `sumeragi_npos_parameters` ክፍያ (NPoS ምርጫ, reconfig) በ `parameters` ብሎክ በኩል ማስጀመሪያ በሰንሰለት ሁኔታ ከ የጋራ ስምምነት እንቡጦች ተግባራዊ; የተፈረመው እገዳ የተፈጠረውን `SetParameter` መመሪያዎችን ይይዛል።
- ቤተኛ መመሪያዎች፡ ጎራውን ይመዝገቡ/ያውጡ፣ መለያ፣ የንብረት ፍቺ; ሚንት / ማቃጠል / ንብረቶችን ማስተላለፍ; የዝውውር ጎራ እና የንብረት ትርጉም ባለቤትነት; ሜታዳታ ቀይር; ፈቃዶችን እና ሚናዎችን ይስጡ።
- IVM ቀስቅሴዎች፡ IVM ባይትኮድ የሚያስፈጽም ቀስቅሴዎችን ይመዝገቡ (`ivm_triggers` ይመልከቱ)። ቀስቅሴዎች ተፈፃሚዎች ከ`ivm_dir` አንፃር ይፈታሉ።
- ቶፖሎጂ፡- በማንኛውም ግብይት (በተለምዶ የመጀመሪያው ወይም የመጨረሻው) ውስጥ በ`topology` ድርድር በኩል የአቻዎችን የመጀመሪያ ስብስብ ያቅርቡ። እያንዳንዱ ግቤት `{ "peer": "<public_key>", "pop_hex": "<hex>" }` ነው; `pop_hex` በማቀናበር ላይ እያለ ሊቀር ይችላል ነገር ግን ከመፈረሙ በፊት መገኘት አለበት።
- አስፈፃሚ ማሻሻያ (አማራጭ): `executor` ካለ, ዘፍጥረት አንድ ነጠላ የማሻሻያ መመሪያን እንደ መጀመሪያው ግብይት ያስገባል; አለበለዚያ, ዘፍጥረት የሚጀምረው በመለኪያዎች / መመሪያዎች ነው.

### የግብይት ማዘዣ

በጽንሰ-ሀሳብ፣ የዘፍጥረት ግብይቶች በዚህ ቅደም ተከተል ይከናወናሉ፡

1) (ከተፈለገ) አስፈፃሚ ማሻሻያ
2) ለእያንዳንዱ ግብይት በ`transactions`፡
   - የመለኪያ ዝመናዎች
   - ቤተኛ መመሪያዎች
   - IVM ቀስቅሴ ምዝገባዎች
   - ቶፖሎጂ ግቤቶች

Kagami እና የመስቀለኛ ኮድ ይህን ቅደም ተከተል ያረጋግጣሉ, ስለዚህም, ለምሳሌ, መለኪያዎች በተመሳሳይ ግብይት ውስጥ ከተከታይ መመሪያዎች በፊት ተግባራዊ ይሆናሉ.

## የሚመከር የስራ ሂደት

- ከ Kagami ጋር ከአብነት ይጀምሩ:
  አብሮገነብ ISI ብቻ፡ `kagami genesis generate --ivm-dir <dir> --genesis-public-key <PK> --consensus-mode npos > genesis.json` (Sora Nexus የህዝብ ዳታ ቦታ፤ `--consensus-mode permissioned` ለIroha2 ወይም የግል Iroha3 ይጠቀሙ)።
  - በብጁ አስፈፃሚ ማሻሻያ (አማራጭ): `--executor <path/to/executor.to>` ይጨምሩ
  - Iroha2-ብቻ: ወደ NPoS የወደፊት መቁረጫ ደረጃ, `--next-consensus-mode npos --mode-activation-height <HEIGHT>` ማለፍ (ለአሁኑ ሁነታ `--consensus-mode permissioned` አቆይ).
- `<PK>` በ`iroha_crypto::Algorithm` የሚታወቅ ማንኛውም መልቲሃሽ ነው፣ Kagami በ `--features gost` (ለምሳሌ `gost3410-2012-256-paramset-a:...`) ሲገነባ የTC26 GOST ልዩነቶችን ጨምሮ።
- በማርትዕ ጊዜ ያረጋግጡ: `kagami genesis validate genesis.json`
- ለማሰማራት ይመዝገቡ: `kagami genesis sign genesis.json --public-key <PK> --private-key <SK> --out-file genesis.signed.nrt`
- አቻዎችን ያዋቅሩ፡ `genesis.file` ወደ የተፈረመው I18NT0000005X ፋይል (ለምሳሌ `genesis.signed.nrt`) እና `genesis.public_key` ወደ ተመሳሳይ `<PK>` ያቀናብሩ ለመፈረም።ማስታወሻዎች፡-
- የKagami “ነባሪ” አብነት የናሙና ጎራ እና መለያዎችን ይመዘግባል፣ ጥቂት ንብረቶችን ያስመዘግባል እና አብሮገነብ ISIs ብቻ በመጠቀም አነስተኛ ፈቃዶችን ይሰጣል - ምንም `.to` አያስፈልግም።
- የማስፈጸሚያ ማሻሻያ ካካተቱ, የመጀመሪያው ግብይት መሆን አለበት. Kagami ሲያመነጭ/ሲፈረም ያስፈጽማል።
- ከመፈረምዎ በፊት ልክ ያልሆኑ የ`Name` እሴቶችን (ለምሳሌ ነጭ ቦታ) እና የተበላሹ መመሪያዎችን ለማግኘት `kagami genesis validate` ይጠቀሙ።

## በDocker/Swarm በመሮጥ ላይ

የቀረበው Docker አዘጋጅ እና ስዋርም መሳሪያ ሁለቱንም ጉዳዮች ይይዛል፡-

- ያለ አስፈፃሚ፡ የጽሑፍ ትእዛዝ የጠፋ/ባዶ I18NI0000158X መስክ ነቅሎ ፋይሉን ይፈርማል።
- ከአስፈጻሚው ጋር፡- አንጻራዊውን የፈፃሚውን መንገድ በመያዣው ውስጥ ወዳለው ፍፁም መንገድ ፈትቶ ፋይሉን ይፈርማል።

ይህ አስቀድሞ ከተገነባው IVM ናሙናዎች ውጭ በማሽኖች ላይ ልማትን ቀላል ያደርገዋል እና አስፈላጊ ሆኖ ሲገኝ አሁንም አስፈፃሚዎችን ማሻሻያ ያደርጋል።