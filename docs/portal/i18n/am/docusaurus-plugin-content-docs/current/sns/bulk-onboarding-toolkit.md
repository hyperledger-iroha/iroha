---
lang: am
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
መታወቂያ፡ የጅምላ-የቦርዲንግ-መሳሪያ ስብስብ
ርዕስ፡ SNS የጅምላ ተሳፍሪ መሣሪያ ስብስብ
sidebar_label፡ የጅምላ የመሳፈሪያ መሣሪያ ስብስብ
መግለጫ፡ CSV ወደ RegisterNameRequestV1 አውቶሜሽን ለ SN-3b ሬጅስትራር ሩጫዎች።
---

::: ማስታወሻ ቀኖናዊ ምንጭ
መስተዋቶች `docs/source/sns/bulk_onboarding_toolkit.md` ስለዚህ ውጫዊ ኦፕሬተሮች ያዩታል።
ማከማቻውን ሳይዘጉ ተመሳሳይ የ SN-3b መመሪያ.
::

# የኤስኤንኤስ የጅምላ መሳፈሪያ መሳሪያ (SN-3ለ)

** የመንገድ ካርታ ማጣቀሻ፡** SN-3b "የጅምላ መሣፈሪያ መሳሪያ"  
** ቅርሶች፡** `scripts/sns_bulk_onboard.py`፣ `scripts/tests/test_sns_bulk_onboard.py`፣
`docs/portal/scripts/sns_bulk_release.sh`

ትላልቅ መዝጋቢዎች ብዙውን ጊዜ በመቶዎች የሚቆጠሩ የ`.sora` ወይም `.nexus` ምዝገባዎችን አስቀድመው ያደርጋሉ።
በተመሳሳይ የአስተዳደር ማፅደቂያ እና የሰፈራ ሀዲዶች. JSON በእጅ መሥራት
ጭነት ወይም ዳግም ማስኬድ CLI አይመዘንም፣ ስለዚህ SN-3b ቆራጥነት ይልካል።
`RegisterNameRequestV1` መዋቅሮችን የሚያዘጋጅ ከCSV እስከ Norito ግንበኛ
Torii ወይም CLI. ረዳቱ እያንዳንዱን ረድፍ ከፊት ለፊት ያረጋግጣል፣ ሁለቱንም አንድ ያስወጣል።
የተዋሃደ አንጸባራቂ እና አማራጭ አዲስ መስመር-የተገደበ JSON፣ እና ማስገባት ይችላል።
ለኦዲቶች የተዋቀሩ ደረሰኞችን በሚመዘግብበት ጊዜ በራስ-ሰር ይጫናል.

## 1. የCSV እቅድ

ተንታኙ የሚከተለውን የራስጌ ረድፍ ይፈልጋል (ትዕዛዙ ተለዋዋጭ ነው)

| አምድ | ያስፈልጋል | መግለጫ |
|--------|----------|--------|
| `label` | አዎ | የተጠየቀ መለያ (የተደባለቀ መያዣ ተቀባይነት አግኝቷል፤ የመሳሪያ መደበኛ አሰራር በ Norm v1 እና UTS-46)። |
| `suffix_id` | አዎ | የቁጥር ቅጥያ ለዪ (አስርዮሽ ወይም `0x` ሄክስ)። |
| `owner` | አዎ | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | አዎ | ኢንቲጀር `1..=255`. |
| `payment_asset_id` | አዎ | የሰፈራ ንብረት (ለምሳሌ `61CtjvNd9T3THAR65GsMVHr82Bjc`)። |
| `payment_gross` / `payment_net` | አዎ | የንብረት ተወላጅ ክፍሎችን የሚወክሉ ያልተፈረሙ ኢንቲጀሮች። |
| `settlement_tx` | አዎ | የክፍያ ግብይቱን ወይም ሃሽን የሚገልጽ የJSON እሴት ወይም ቀጥተኛ ሕብረቁምፊ። |
| `payment_payer` | አዎ | ክፍያውን የፈቀደ AccountId። |
| `payment_signature` | አዎ | መጋቢ ወይም የግምጃ ቤት ፊርማ ማረጋገጫ የያዘ JSON ወይም ቀጥተኛ ሕብረቁምፊ። |
| `controllers` | አማራጭ | በሴሚኮሎን- ወይም በነጠላ ሰረዝ የተለዩ የተቆጣጣሪ መለያ አድራሻዎች ዝርዝር። ሲቀሩ የ I18NI0000044X ነባሪዎች። |
| `metadata` | አማራጭ | የመስመር ላይ JSON ወይም I18NI0000046X የመፍታት ፍንጮችን፣ የTXT መዝገቦችን እና የመሳሰሉትን ያቀርባል። የ`{}` ነባሪዎች። |
| `governance` | አማራጭ | የመስመር ውስጥ JSON ወይም I18NI0000049X ወደ `GovernanceHookV1` እየጠቆመ። `--require-governance` ይህን አምድ ያስፈጽማል። |

ማንኛውም አምድ የሕዋስ እሴቱን በ`@` በማስቀደም ውጫዊ ፋይልን ሊያመለክት ይችላል።
ከCSV ፋይል አንጻር ዱካዎች ተፈትተዋል።

## 2. ረዳትን በማሄድ ላይ

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

ቁልፍ አማራጮች፡-

- `--require-governance` ያለ የአስተዳደር መንጠቆ ረድፎችን ውድቅ ያደርጋል (ይጠቅማል)
  ፕሪሚየም ጨረታዎች ወይም የተያዙ ሥራዎች)።
- `--default-controllers {owner,none}` ባዶ መቆጣጠሪያ ሴሎችን ይወስናል
  ወደ ባለቤት መለያው ይመለሱ።
- `--controllers-column`፣ `--metadata-column`፣ እና `--governance-column` እንደገና መሰየም
  ወደላይ ወደ ውጭ መላክ በሚሰሩበት ጊዜ አማራጭ አምዶች።

በስኬት ላይ ስክሪፕቱ የተዋሃደ አንጸባራቂ ይጽፋል፡-

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "<i105-account-id>",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"<i105-account-id>","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"<i105-account-id>",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```

`--ndjson` ከተሰጠ፣ እያንዳንዱ I18NI0000059X እንዲሁ ተጽፏል
ነጠላ-መስመር JSON ሰነድ አውቶማቲክስ ጥያቄዎችን በቀጥታ ወደ ውስጥ ማስተላለፍ ይችላሉ።
Torii፡

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. አውቶማቲክ ማቅረቢያዎች

### 3.1 I18NT0000006X REST ሁነታ

`--submit-torii-url` እና ወይ I18NI0000061X ወይም
`--submit-token-file` እያንዳንዱን አንጸባራቂ ግቤት በቀጥታ ወደ Torii ለመግፋት፡

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- ረዳቱ በጥያቄ አንድ I18NI0000063X ያወጣል እና ያስወርዳል
  የመጀመሪያው የኤችቲቲፒ ስህተት። ምላሾች እንደ NDJSON በሎግ ዱካ ላይ ተያይዘዋል
  መዝገቦች.
- `--poll-status` እንደገና መጠይቆች I18NI0000065X ከእያንዳንዱ በኋላ
  መዝገቡ መሆኑን ለማረጋገጥ ማስረከብ (እስከ I18NI0000066X፣ ነባሪ 5)
  የሚታይ. `--suffix-map` (JSON of I18NI0000068X እስከ I18NI0000069X እሴቶች) ያቅርቡ
  መሣሪያው ለምርጫ I18NI0000070X ቀጥተኛ ቃላትን ማግኘት ይችላል።
- Tunables: I18NI0000071X, I18NI0000072X, እና I18NI0000073X.

### 3.2 iroha CLI ሁነታ

እያንዳንዱን አንጸባራቂ ግቤት በCLI በኩል ለማምራት የሁለትዮሽ መንገድ ያቅርቡ፡

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- ተቆጣጣሪዎች I18NI0000074X ግቤቶች (`controller_type.kind = "Account"`) መሆን አለባቸው
  ምክንያቱም CLI በአሁኑ ጊዜ መለያ ላይ የተመሰረቱ ተቆጣጣሪዎችን ብቻ ያጋልጣል።
- ሜታዳታ እና የአስተዳደር ብሎብ በጥያቄ ጊዜያዊ ፋይሎች ላይ የተፃፈ ነው።
  ወደ `iroha sns register --metadata-json ... --governance-json ...` ተላልፏል.
- CLI stdout እና stderr plus መውጫ ኮዶች ገብተዋል፤ ዜሮ ያልሆኑ የመውጫ ኮዶች ይቋረጣሉ
  ሩጫው ።

ሁለቱም የማስረከቢያ ሁነታዎች አብረው ሊሄዱ ይችላሉ (Torii እና CLI) ለመዝጋቢ ቼክ
ማሰማራት ወይም ውድቀትን ይለማመዱ።

### 3.3 የማስረከቢያ ደረሰኞች

`--submission-log <path>` ሲቀርብ፣ ስክሪፕቱ የNDJSON ግቤቶችን ይጨምራል።
በመያዝ፡

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

ስኬታማ የI18NT0000009X ምላሾች የተዋቀሩ መስኮችን ያካትታሉ
`NameRecordV1` ወይም `RegisterNameResponseV1` (ለምሳሌ `record_status`፣
`record_pricing_class`፣ `record_owner`፣ `record_expires_at_ms`፣
`registry_event_version`፣ `suffix_id`፣ `label`) ስለዚህ ዳሽቦርዶች እና አስተዳደር
ሪፖርቶች ነፃ ቅጽ ጽሑፍን ሳይመረምሩ መዝገቡን ሊተነተኑ ይችላሉ። ይህን ምዝግብ ማስታወሻ ያያይዙት።
ሊባዛ ለሚችል ማስረጃ ከመግለጫው ጎን የመመዝገቢያ ትኬቶች።

## 4. የሰነዶች ፖርታል ልቀት አውቶማቲክ

CI እና ፖርታል ስራዎች `docs/portal/scripts/sns_bulk_release.sh` ይደውሉ, እሱም ይጠቀለላል
ረዳቱ እና ቅርሶችን በ `artifacts/sns/releases/<timestamp>/` ስር ያከማቻል፡

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

ስክሪፕቱ፡-

1. I18NI0000089X፣ `registrations.ndjson` ይገነባል እና
   ኦሪጅናል CSV ወደ የመልቀቂያ ማውጫ።
2. መግለጫውን Torii እና/ወይም CLI (ሲዋቀር) በመጠቀም ያቀርባል።
   `submissions.log` ከላይ ከተዋቀሩ ደረሰኞች ጋር።
3. Emits I18NI0000092X ልቀቱን የሚገልጽ (መንገዶች፣ Torii URL፣ CLI ዱካ፣
   timestamp) ስለዚህ ፖርታል አውቶሜሽን ጥቅሉን ወደ አርቲፊክ ማከማቻ መስቀል ይችላል።
4. I18NI0000093X (በ`--metrics` መሻር) ያመርታል
   Prometheus-ቅርጸት ቆጣሪዎች ለጠቅላላ ጥያቄዎች፣ ቅጥያ ስርጭት፣
   የንብረት ጠቅላላ, እና የማስረከቢያ ውጤቶች. JSON ማጠቃለያው ከዚህ ፋይል ጋር ይገናኛል።

የስራ ፍሰቶች በቀላሉ የመልቀቂያ ማውጫውን እንደ አንድ ጥበባዊ ስራ በማህደር ያስቀምጣቸዋል፣ እሱም አሁን
ለኦዲት አስተዳደር የሚያስፈልጉትን ነገሮች ሁሉ ይዟል።

## 5. ቴሌሜትሪ እና ዳሽቦርዶች

በ`sns_bulk_release.sh` የተፈጠረው የልኬት ፋይል የሚከተሉትን ያጋልጣል
ተከታታይ፡

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

`metrics.prom` ወደ የእርስዎ I18NT0000001X የጎን መኪና (ለምሳሌ በፕሮምቴይል ወይም በ a
ባች አስመጪ) ሬጅስትራሮች፣ መጋቢዎች እና የአስተዳደር እኩዮች እንዲሰለፉ ማድረግ
የጅምላ እድገት. Grafana ሰሌዳ
`dashboards/grafana/sns_bulk_release.json` ከፓነሎች ጋር ተመሳሳይ ውሂብን ያሳያል
ለአንድ ቅጥያ ቆጠራዎች፣ የክፍያ መጠን እና የማስረከቢያ ስኬት/ውድቀት ሬሾዎች።
ቦርዱ በI18NI0000098X ያጣራል ስለዚህ ኦዲተሮች ወደ አንድ የCSV ሩጫ መግባት ይችላሉ።

## 6. የማረጋገጫ እና ውድቀት ሁነታዎች

- ** መለያ ቀኖና: ** ግብዓቶች በ Python IDNA ፕላስ መደበኛ ናቸው።
  ንዑስ ሆሄ እና Norm v1 ቁምፊ ማጣሪያዎች። ልክ ያልሆኑ መለያዎች ከማንኛቸውም በፊት በፍጥነት ይወድቃሉ
  የአውታረ መረብ ጥሪዎች.
- ** የቁጥር መከላከያ መንገዶች፡** ቅጥያ መታወቂያዎች፣ የቃል ዓመታት እና የዋጋ ፍንጮች መውደቅ አለባቸው።
  በ `u16` እና `u8` ወሰኖች ውስጥ። የክፍያ መስኮች የአስርዮሽ ወይም የአስራስድስትዮሽ ኢንቲጀር ይቀበላሉ።
  እስከ `i64::MAX`.
- **ሜታዳታ ወይም የአስተዳደር መተንተን፡** የመስመር ላይ JSON በቀጥታ ተተነተነ፤ ፋይል
  ማጣቀሻዎች ከCSV አካባቢ አንፃር ተፈትተዋል። የነገር ያልሆነ ሜታዳታ
  የማረጋገጫ ስህተት ይፈጥራል.
- ** ተቆጣጣሪዎች: ** ባዶ ሕዋሳት `--default-controllers` ያከብራሉ. በግልፅ ያቅርቡ
  የመቆጣጠሪያ ዝርዝሮች (ለምሳሌ `<i105-account-id>;<i105-account-id>`) ባለቤት ላልሆኑ ውክልና ሲሰጡ
  ተዋናዮች.

አለመሳካቶች በዐውደ-ጽሑፍ የረድፍ ቁጥሮች (ለምሳሌ፦
`error: row 12 term_years must be between 1 and 255`). ስክሪፕቱ አብሮ ይወጣል
ኮድ `1` በማረጋገጫ ስህተቶች እና `2` የCSV መንገድ ሲጠፋ።

## 7. ሙከራ እና ፕሮቬንሽን

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` የCSV ትንተናን ይሸፍናል ፣
  የNDJSON ልቀት፣ የአስተዳደር ማስፈጸሚያ እና የCLI ወይም Torii ማስረከብ
  መንገዶች.
- ረዳቱ ንጹህ ፓይዘን (ምንም ተጨማሪ ጥገኛ የለም) እና በየትኛውም ቦታ ይሰራል
  `python3` ይገኛል። የቁርጥ ቀን ታሪክ በ ውስጥ ከ CLI ጋር ይከታተላል
  ለመራባት ዋና ማከማቻ።

ለምርት ስራዎች፣ የመነጨውን አንጸባራቂ እና NDJSON ጥቅልን ከ
የመመዝገቢያ ትኬት ስለዚህ መጋቢዎች የገቡትን ትክክለኛ ጭነት እንደገና መጫወት ይችላሉ።
ወደ Torii።