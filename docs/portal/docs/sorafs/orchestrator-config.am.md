---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7db41ca4cc25ba00f6f14e357d8871e1b27c8d6b5d489557d8416b3f6f7a39ef
source_last_modified: "2026-01-22T14:45:01.294133+00:00"
translation_last_reviewed: 2026-02-07
id: orchestrator-config
title: SoraFS Orchestrator Configuration
sidebar_label: Orchestrator Configuration
description: Configure the multi-source fetch orchestrator, interpret failures, and debug telemetry output.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

# ባለብዙ ምንጭ አምጣ ኦርኬስትራ መመሪያ

የSoraFS ባለብዙ-ምንጭ ፈልሳፊ ኦርኬስትራ ቆራጥ፣ ትይዩ ያንቀሳቅሳል
በአስተዳደር በሚደገፉ ማስታወቂያዎች ላይ የታተመ ከአቅራቢው የሚወርዱ። ይህ
መመሪያው ኦርኬስትራውን እንዴት ማዋቀር እንደሚቻል፣ ምን አይነት ውድቀት ምልክቶች እንደሚጠብቁ ያብራራል።
በመልቀቅ ወቅት፣ እና የትኞቹ የቴሌሜትሪ ዥረቶች የጤና አመልካቾችን ያጋልጣሉ።

## 1. የማዋቀር አጠቃላይ እይታ

ኦርኬስትራ ሶስት የውቅር ምንጮችን ያዋህዳል፡-

| ምንጭ | ዓላማ | ማስታወሻ |
|--------|---------|-------|
| `OrchestratorConfig.scoreboard` | የአቅራቢዎችን ክብደትን መደበኛ ያደርገዋል፣ የቴሌሜትሪ ትኩስነትን ያረጋግጣል፣ እና የJSON የውጤት ሰሌዳ ለኦዲት ጥቅም ላይ ይውላል። | በ`crates/sorafs_car::scoreboard::ScoreboardConfig` የተደገፈ። |
| `OrchestratorConfig.fetch` | የሩጫ ጊዜ ገደቦችን ይተገበራል (በጀቶችን እንደገና ይሞክሩ ፣ የተዛማጅ ገደቦች ፣ የማረጋገጫ መቀየሪያዎች)። | ካርታዎች ወደ `FetchOptions` በ I18NI0000022X። |
| CLI / SDK መለኪያዎች | የእኩዮችን ብዛት ይቁጠሩ፣ የቴሌሜትሪ ክልሎችን ያያይዙ፣ እና የገጽታ መከልከል/የማሳደግ ፖሊሲዎች። | `sorafs_cli fetch` እነዚህን ባንዲራዎች በቀጥታ ያጋልጣል; ኤስዲኬዎች በ`OrchestratorConfig` በኩል ይሰርዟቸዋል። |

በ I18NI0000025X ውስጥ ያሉት የJSON ረዳቶች ሙሉውን ተከታታይነት አላቸው።
ወደ Norito JSON በማዋቀር በኤስዲኬ ማሰሪያዎች ላይ ተንቀሳቃሽ እና
አውቶሜሽን.

### 1.1 ናሙና JSON ውቅር

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

ፋይሉን በተለመደው I18NI0000026X ንብርብር (`defaults/`፣ ተጠቃሚ፣
ትክክለኛ) ስለዚህ ቆራጥ ማሰማራቶች በአንጓዎች ላይ ተመሳሳይ ገደቦችን ይወርሳሉ።
ከSNNet-5a ልቀት ጋር ለሚስማማ ቀጥታ-ብቻ የመመለሻ መገለጫ፣
`docs/examples/sorafs_direct_mode_policy.json` እና ጓደኛውን ያማክሩ
መመሪያ በ `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 ተገዢነትን ይሽራል።

SNNet-9 በአስተዳደር የሚመራ ተገዢነትን በኦርኬስትራ ውስጥ ያስገባል። አዲስ
በI18NT0000002X JSON ውቅር ውስጥ ያለው የ`compliance` ነገር የቅርጻ ቅርጾችን ይይዛል
የቧንቧ መስመርን ወደ ቀጥታ-ብቻ ሁነታ የሚያስገድድ:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` ISO-3166 alpha-2 ኮዶችን በዚህ ቦታ ያውጃል
  ኦርኬስትራ ምሳሌ ይሰራል። በዚህ ጊዜ ኮዶች ወደ አቢይ ሆሄ ተደርገዋል።
  መተንተን.
- `jurisdiction_opt_outs` የአስተዳደር መዝገብ ያንጸባርቃል. መቼ ማንኛውም ኦፕሬተር
  ስልጣን በዝርዝሩ ላይ ይታያል, ኦርኬስትራ ያስገድዳል
  `transport_policy=direct-only` እና የመመሪያውን ውድቀት ምክንያት ያወጣል።
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` አንጸባራቂ የምግብ መፈጨትን ይዘረዝራል (ታወሩ CIDs፣ በ ኮድ
  አቢይ ሆክስ)። የሚዛመዱ የክፍያ ጭነቶች እንዲሁ በቀጥታ-ብቻ መርሐግብር ያስገድዳሉ እና
  በቴሌሜትሪ የ `compliance_blinded_cid_opt_out` ውድቀት
- `audit_contacts` የዩአርአይ አስተዳደር ኦፕሬተሮች እንዲያትሙ ይጠብቃል በማለት ይመዘግባል
  ያላቸውን GAR playbooks.
- `attestations` ፖሊሲውን የሚደግፉ የተፈረመባቸው የተሟሉ ፓኬቶችን ይይዛል።
  እያንዳንዱ ግቤት አማራጭ `jurisdiction` (ISO-3166 alpha-2 ኮድ) ይገልጻል።
  `document_uri`፣ ቀኖናዊው 64-ቁምፊ `digest_hex`፣ የተሰጠበት
  timestamp `issued_at_ms`፣ እና አማራጭ `expires_at_ms`። እነዚህ ቅርሶች
  የአስተዳደር መሳሪያዎች ማገናኘት እንዲችል ወደ ኦርኬስትራ የኦዲት ማረጋገጫ ዝርዝር ውስጥ ይግቡ
  የተፈረመውን ወረቀት ይሽራል.

ተገዢነትን ማገጃውን በተለመደው የውቅረት ንብርብር ስለዚህ ኦፕሬተሮች ያቅርቡ
ቆራጥ መሻሮችን መቀበል። ኦርኬስትራተሩ _በኋላ_ ማክበርን ይተገበራል
የአጻጻፍ ሁነታ ፍንጮች፡ ምንም እንኳን ኤስዲኬ `upload-pq-only` ቢጠይቅም፣ ሕጋዊ ወይም
አንጸባራቂ መርጦ መውጣቶች አሁንም ወደ ቀጥታ-ብቻ ማጓጓዝ ይመለሳሉ እና ቁ
ታዛዥ አቅራቢዎች አሉ።

ቀኖናዊ መርጦ መውጣት ካታሎጎች ስር ይኖራሉ
`governance/compliance/soranet_opt_outs.json`; የአስተዳደር ምክር ቤት ያትማል
መለያ በተሰጣቸው ልቀቶች በኩል ዝማኔዎች። የተሟላ ምሳሌ ውቅር (ጨምሮ
ማረጋገጫዎች) በ `docs/examples/sorafs_compliance_policy.json` ውስጥ ይገኛል።
የአሰራር ሂደቱ በ ውስጥ ተይዟል
[GAR compliance playbook](../../../source/soranet/gar_compliance_playbook.md)።

### 1.3 CLI እና SDK Knobs

| ባንዲራ / መስክ | ውጤት |
|-------------|-------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | የውጤት ሰሌዳ ማጣሪያ ምን ያህል አቅራቢዎች እንደሚተርፉ ይገድባል። እያንዳንዱን ብቁ አቅራቢ ለመጠቀም ወደ `None` ያቀናብሩ። |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | ካፕስ በአንድ ክፍል ይሞክራል። ከገደቡ ማለፍ `MultiSourceError::ExhaustedRetries` ከፍ ያደርገዋል። |
| `--telemetry-json` | የውጤት ሰሌዳ ገንቢ ላይ የቆይታ/የሽንፈት ቅጽበተ-ፎቶዎችን ያስገባል። ከ`telemetry_grace_secs` በላይ የቆየ ቴሌሜትሪ አቅራቢዎችን ብቁ እንዳልሆኑ ያሳያል። |
| `--scoreboard-out` | ለድህረ ሩጫ ፍተሻ የተሰላው የውጤት ሰሌዳ (ብቁ + ብቁ ያልሆኑ አቅራቢዎች) ጸንቷል። |
| `--scoreboard-now` | የውጤት ሰሌዳ የጊዜ ማህተምን (ዩኒክስ ሰከንድ) ይሽራል። |
| `--deny-provider` / የውጤት ፖሊሲ መንጠቆ | ማስታወቂያዎችን ሳይሰርዙ አቅራቢዎችን ከመርሐግብር ማግለል በቆራጥነት። ለፈጣን ምላሽ ለጥቁር መዝገብ ጠቃሚ። |
| `--boost-provider=name:delta` | የአስተዳደር ክብደቶች ሳይነኩ በመተው ለአቅራቢው ክብደተ-ሮቢን ክሬዲቶችን ያስተካክሉ። |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | ዳሽቦርዶች በጂኦግራፊ ወይም በታቀደው ሞገድ መገልበጥ እንዲችሉ መለያዎች መለኪያዎችን እና የተዋቀሩ ምዝግቦችን አውጥተዋል። |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | የብዝሃ-ምንጭ ኦርኬስትራ የመነሻ መስመር ስለሆነ አሁን የ I18NI0000063X ነባሪዎች ናቸው። ማዋረድ ሲያዘጋጁ ወይም የማክበር መመሪያን ሲከተሉ `direct-only` ይጠቀሙ እና `soranet-strict` ለPQ-ብቻ አብራሪዎች ያስቀምጡ። ማክበር መሻር አሁንም እንደ ጠንካራ ጣሪያ ሆኖ ይሠራል። |

SoraNet-መጀመሪያ አሁን የመላኪያ ነባሪ ነው፣ እና መልሶ ማሰራጫዎች ተገቢውን የSNNet አጋጅ መጥቀስ አለባቸው። SNNet-4/5/5a/5b/6a/7/8/12/13 ከተመረቀ በኋላ፣ አስተዳደር የሚፈለገውን አቋም ወደፊት (ወደ `soranet-strict`) ያስተካክላል። እስከዚያው ድረስ፣ በአጋጣሚ የሚነዱ መሻሮች ብቻ ለ`direct-only` ቅድሚያ መስጠት አለባቸው፣ እና በታቀደ መዝገብ ውስጥ መመዝገብ አለባቸው።

ከላይ ያሉት ሁሉም ባንዲራዎች በሁለቱም `sorafs_cli fetch` እና የ `--` አይነት አገባብ ይቀበላሉ
ገንቢ-ፊት ለፊት `sorafs_fetch` ሁለትዮሽ. ኤስዲኬዎች በተተየቡ በኩል ተመሳሳይ አማራጮችን ያጋልጣሉ
ግንበኞች ።

### 1.4 የጥበቃ መሸጎጫ አስተዳደር

CLI አሁን በ SoraNet Guard መራጭ ውስጥ ኦፕሬተሮች ግቤትን መሰካት እንዲችሉ ሽቦዎች አሉት
ከሙሉ SNNet-5 የትራንስፖርት ልቀት በፊት በቆራጥነት ማሰራጫዎች። ሶስት
አዲስ ባንዲራዎች የስራ ሂደቱን ይቆጣጠራሉ፡

| ባንዲራ | ዓላማ |
|------|--------|
| `--guard-directory <PATH>` | የቅርብ ጊዜውን የመተላለፊያ ስምምነት የሚገልጽ የJSON ፋይል ነጥቦች (ንዑስ ስብስብ ከዚህ በታች ይታያል)። ማውጫውን ማለፍ ማውጣቱን ከመፈጸሙ በፊት የጥበቃ መሸጎጫውን ያድሳል። |
| `--guard-cache <PATH>` | Norito-encoded `GuardSet` ጸንቷል። የሚቀጥሉት ሩጫዎች አዲስ ማውጫ ባይቀርብም መሸጎጫውን እንደገና ይጠቀማሉ። |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | ለመሰካት የመግቢያ ጠባቂዎች ብዛት (ነባሪ 3) እና የማቆያ መስኮቱ (ነባሪ 30 ቀናት) አማራጭ ይሻራል። |
| `--guard-cache-key <HEX>` | አማራጭ 32-ባይት ቁልፍ የጥበቃ መሸጎጫዎችን በBlake3 MAC መለያ ለመስጠት ጥቅም ላይ ይውላል ስለዚህ ፋይሉ እንደገና ጥቅም ላይ ከመዋሉ በፊት ሊረጋገጥ ይችላል። |

የጥበቃ ማውጫ ክፍያ ጭነቶች የታመቀ ንድፍ ይጠቀማሉ፡-

የ`--guard-directory` ባንዲራ አሁን Norito-የተመሰጠረ ይጠብቃል
`GuardDirectorySnapshotV2` ጭነት. የሁለትዮሽ ቅጽበታዊ ገጽ እይታው የሚከተሉትን ያጠቃልላል

- `version` - የመርሃግብር ስሪት (በአሁኑ ጊዜ `2`)።
- `directory_hash`፣ `published_at_unix`፣ `valid_after_unix`፣ `valid_until_unix` — የጋራ ስምምነት
  ከእያንዳንዱ የተካተተ የምስክር ወረቀት ጋር መዛመድ ያለበት ዲበ ውሂብ።
- `validation_phase` — የምስክር ወረቀት ፖሊሲ በር (`1` = ነጠላ የ Ed25519 ፊርማ ፍቀድ ፣
  `2` = ሁለት ፊርማዎችን እመርጣለሁ፣ `3` = ድርብ ፊርማ ያስፈልገዋል)።
- `issuers` - `fingerprint`፣ `ed25519_public` እና I18NI0000092X ያላቸው የአስተዳደር ሰጪዎች።
  የጣት አሻራዎች እንደ ይሰላሉ
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` - የ SRCv2 ጥቅሎች ዝርዝር (`RelayCertificateBundleV2::to_cbor()` ውፅዓት)። እያንዳንዱ ጥቅል
  የማስተላለፊያ ገላጭን፣ የችሎታ ባንዲራዎችን፣ ML-KEM ፖሊሲን እና ባለሁለት Ed25519/ML-DSA-65ን ይይዛል።
  ፊርማዎች.

CLI ማውጫውን ከማዋሃዱ በፊት እያንዳንዱን ጥቅል ከታወጁት የሰጪ ቁልፎች ጋር ያረጋግጣል

የቅርብ ጊዜውን ስምምነት ከ
አሁን ያለው መሸጎጫ. መራጩ አሁንም በ ውስጥ ያሉትን የተሰኩ ጠባቂዎችን ይጠብቃል።
የማቆያ መስኮት እና በማውጫው ውስጥ ብቁ; አዲስ ማሰራጫዎች መተካት ጊዜው አልፎበታል።
ግቤቶች. በተሳካ ሁኔታ ከተገኘ በኋላ የዘመነው መሸጎጫ ወደ መንገዱ ተመልሶ ይጻፋል
በI18NI0000097X በኩል የሚቀርብ፣ የሚቀጥሉትን ክፍለ ጊዜዎች የሚወስን ነው። ኤስዲኬዎች
በመደወል ተመሳሳይ ባህሪን ማባዛት ይችላል
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` እና
የተገኘውን I18NI0000099X በ `SorafsGatewayFetchOptions` በክር ማድረግ።

`ml_kem_public_hex` መራጩ PQ ለሚችሉ ጠባቂዎች ቅድሚያ እንዲሰጥ ያስችለዋል።
የ SNNet-5 ልቀት. የመድረክ መቀየሪያዎች (`anon-guard-pq`፣ `anon-majority-pq`፣
`anon-strict-pq`) አሁን ክላሲካል ቅብብሎሽዎችን በራስ ሰር ዝቅ አድርግ፡ የPQ ጠባቂ ሲሆን
አለ መራጩ ከመጠን በላይ ክላሲካል ፒን ስለሚጥል ተከታይ ክፍለ ጊዜዎች ይደግፋሉ
ድብልቅ የእጅ መጨባበጥ. CLI/SDK ማጠቃለያዎች የተገኘውን ድብልቅ በ
`anonymity_status`/`anonymity_reason`፣ `anonymity_effective_policy`፣
`anonymity_pq_selected`፣
`anonymity_classical_selected`፣ `anonymity_pq_ratio`፣
`anonymity_classical_ratio`፣ እና ተጓዳኝ እጩ/ጉድለት/አቅርቦት ዴልታ
ሜዳዎች፣ ቡኒዎች እና ክላሲካል ውድቀቶችን ግልፅ ማድረግ።

የጥበቃ ማውጫዎች አሁን የተሟላ የSRCv2 ጥቅል በ በኩል ሊከተቱ ይችላሉ።
`certificate_base64`. ኦርኬስትራተሩ እያንዳንዱን ጥቅል ይፈታዋል፣ እንደገና ያረጋግጣል
Ed25519/ML-DSA ፊርማዎች፣ እና የተተነተነውን የእውቅና ማረጋገጫ ከ
የጥበቃ መሸጎጫ. የምስክር ወረቀት ሲገኝ ለ ቀኖናዊ ምንጭ ይሆናል
PQ ቁልፎች፣ የእጅ መጨባበጥ ምርጫዎች እና ክብደት ማድረግ; ጊዜው ያለፈባቸው የምስክር ወረቀቶች ናቸው
በወረዳ የሕይወት ዑደት አስተዳደር በኩል ይሰራጫሉ እና በ
`telemetry::sorafs.guard` እና `telemetry::sorafs.circuit`
ተቀባይነት ያለው መስኮት፣ የመጨባበጥ ስብስቦች፣ እና ድርብ ፊርማዎች መታየታቸውን
እያንዳንዱ ጠባቂ.

ቅጽበተ-ፎቶዎችን ከአታሚዎች ጋር ማመሳሰል ለማድረግ የCLI አጋሮችን ይጠቀሙ፡-```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` SRCv2 ቅጽበተ ፎቶን ወደ ዲስክ ከመጻፉ በፊት አውርዶ ያረጋግጣል፣
`verify` ከሌሎች ለመጡ ቅርሶች የማረጋገጫ ቧንቧን ሲደግም
ቡድኖች፣ የCLI/SDK ጠባቂ መምረጫ ውፅዓትን የሚያንፀባርቅ የJSON ማጠቃለያ በማሰራጨት ላይ።

### 1.5 የወረዳ የሕይወት ዑደት አስተዳዳሪ

ሁለቱም የሪሌይ ማውጫ እና የጥበቃ መሸጎጫ ሲቀርቡ ኦርኬስትራ
የሶራኔት ወረዳዎችን አስቀድሞ ለመገንባት እና ለማደስ የወረዳውን የህይወት ዑደት አስተዳዳሪን ያነቃል።
ከእያንዳንዱ ማምለጫ በፊት. ውቅር በ`OrchestratorConfig` ውስጥ ይኖራል
(`crates/sorafs_orchestrator/src/lib.rs:305`) በሁለት አዳዲስ መስኮች፡-

- `relay_directory`፡ የ SNNet-3 ማውጫ ቅጽበተ ፎቶን ይይዛል ወደ መሃል/ውጣ ሆፕ
  በቆራጥነት ሊመረጥ ይችላል.
- `circuit_manager`: አማራጭ ውቅር (በነባሪ የነቃ) ቁጥጥር
  የወረዳ TTL.

Norito JSON አሁን የ`circuit_manager` ብሎክ ይቀበላል፡-

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

ኤስዲኬዎች የማውጫ ውሂብን በ በኩል ያስተላልፋሉ
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`)፣ እና CLI በማንኛውም ጊዜ በራስ-ሰር ያሰራዋል።
`--guard-directory` ቀርቧል (`crates/iroha_cli/src/commands/sorafs.rs:365`)።

የጥበቃ ሜታዳታ በተለወጠ ቁጥር አስተዳዳሪው ወረዳዎችን ያድሳል (የመጨረሻ ነጥብ፣ PQ ቁልፍ፣
ወይም የተሰካው የጊዜ ማህተም) ወይም ቲቲኤል ያልፋል። ረዳት `refresh_circuits`
ከእያንዳንዱ ፈልጎ (`crates/sorafs_orchestrator/src/lib.rs:1346`) በፊት ተጠርቷል
ኦፕሬተሮች የሕይወት ዑደት ውሳኔዎችን መከታተል እንዲችሉ የ `CircuitEvent` ሎግ ያወጣል። መስቀያው
ሙከራ `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) የተረጋጋ መዘግየትን ያሳያል
በሶስት የጥበቃ ሽክርክሪቶች ላይ; የተያያዘውን ዘገባ በ ላይ ይመልከቱ
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 የአካባቢ QUIC ፕሮክሲ

ኦርኬስትራተሩ እንደ አማራጭ የአሳሽ ቅጥያዎችን የአካባቢ QUIC ፕሮክሲ ማፍለቅ ይችላል።
እና የኤስዲኬ አስማሚዎች የምስክር ወረቀቶችን ማስተዳደር ወይም መሸጎጫ ቁልፎችን መጠበቅ የለባቸውም። የ
ፕሮክሲ ከ loopback አድራሻ ጋር ይገናኛል፣ የQUIC ግንኙነቶችን ያቋርጣል እና ይመልሳል ሀ
የምስክር ወረቀቱን እና የአማራጭ የጥበቃ መሸጎጫ ቁልፍን የሚገልጽ Norito አንጸባራቂ
ደንበኛ. በፕሮክሲው የሚለቀቁ የትራንስፖርት ክንውኖች በ በኩል ይቆጠራሉ።
`sorafs_orchestrator_transport_events_total`.

በኦርኬስትራ JSON ውስጥ በአዲሱ የ`local_proxy` ብሎክ በኩል ተኪውን ያንቁ፡-

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```

- `bind_addr` ተኪው የሚያዳምጥበትን ይቆጣጠራል (ለመጠየቅ `0` ወደብ ይጠቀሙ
  የኢፌመር ወደብ)።
- `telemetry_label` ዳሽቦርዶች እንዲለዩ ወደ ልኬቶች ይሰራጫል
  ፕሮክሲዎች ከ fetch ክፍለ ጊዜዎች።
- `guard_cache_key_hex` (አማራጭ) ተኪው ተመሳሳይ የቁልፍ ጠባቂ እንዲታይ ያስችለዋል።
  CLI/SDKs የሚተማመኑበት መሸጎጫ፣ የአሳሽ ቅጥያዎችን በማመሳሰል ላይ።
- `emit_browser_manifest` መጨባበጥ ያንን አንጸባራቂ ይመልስ እንደሆነ ይቀየራል።
  ቅጥያዎች ማከማቸት እና ማረጋገጥ ይችላሉ.
- `proxy_mode` ተኪ ድልድይ ትራፊክን በአካባቢው (`bridge`) ወይም አለመሆኑን ይመርጣል
  ኤስዲኬዎች የ SoraNet ወረዳዎችን ራሳቸው እንዲከፍቱ ሜታዳታ ብቻ ነው የሚያወጣው
  (`metadata-only`)። ፕሮክሲው ነባሪው ለ `bridge`; አዘጋጅ `metadata-only` መቼ ሀ
  የስራ ቦታ ጅረቶችን ሳያስተላልፍ አንጸባራቂውን ማጋለጥ አለበት።
- `prewarm_circuits`፣ `max_streams_per_circuit`፣ እና `circuit_ttl_hint_secs`
  ትይዩ ዥረቶችን ማበጀት እንዲችል እና ተጨማሪ ፍንጮችን ለአሳሹ ያቅርቡ
  ተኪው ወረዳዎችን ምን ያህል ኃይለኛ በሆነ መልኩ እንደሚጠቀም ይረዱ።
- `car_bridge` (አማራጭ) ነጥቦች በአካባቢያዊ የ CAR መዝገብ ቤት መሸጎጫ። `extension`
  የመስክ ዥረቱ ዒላማ `*.car` ሲቀር የተገጠመውን ቅጥያ ይቆጣጠራል; አዘጋጅ
  `allow_zst = true` ቀድሞ የተጨመቁ `*.car.zst` ጭነት ጭነቶችን በቀጥታ ለማገልገል።
- `kaigi_bridge` (አማራጭ) የተንቆጠቆጡ የካይጂ መንገዶችን ለፕሮክሲው ያጋልጣል። የ
  `room_policy` መስክ ድልድዩ በ `public` ውስጥ ይሠራ እንደሆነ ያስተዋውቃል ወይም
  የ`authenticated` ሁነታ ስለዚህ የአሳሽ ደንበኞች ትክክለኛውን የ GAR መለያዎች አስቀድመው መምረጥ ይችላሉ።
- `sorafs_cli fetch` `--local-proxy-mode=bridge|metadata-only` እና
  `--local-proxy-norito-spool=PATH` ይሽራል፣ ኦፕሬተሮች እንዲቀይሩ ያስችላቸዋል
  የJSON ፖሊሲን ሳይቀይሩ የሩጫ ሁነታ ወይም በተለዋጭ spools ላይ ያመልክቱ።
- `downgrade_remediation` (አማራጭ) አውቶማቲክ የማውረድ መንጠቆን ያዋቅራል።
  ኦርኬስትራተሩ ሲነቃ ፍንጣቂዎችን ዝቅ ለማድረግ የቴሌሜትሪ ቅብብሎሽ ይመለከተዋል።
  እና በ `window_secs` ውስጥ ከተዋቀረው `threshold` በኋላ የአካባቢውን ያስገድዳል
  ፕሮክሲ ወደ `target_mode` (ነባሪው `metadata-only`)። አንዴ ማሽቆልቆሉ ይቆማል
  ፕሮክሲው ከ `cooldown_secs` በኋላ ወደ `resume_mode` ይመለሳል። `modes` ይጠቀሙ
  ቀስቅሴውን ወደ ተወሰኑ የዝውውር ሚናዎች (የመግቢያ ቅብብሎሽ ነባሪዎች) ለማካተት ድርድር።

ተኪው በድልድይ ሁነታ ላይ ሲሄድ ሁለት የመተግበሪያ አገልግሎቶችን ያገለግላል፡-

- **`norito`** - የደንበኛው የዥረት ዒላማ ከሚከተሉት አንፃር ተፈትቷል
  `norito_bridge.spool_dir`. ዒላማዎች የፀዱ ናቸው (ምንም መሻገር የለም፣ ፍፁም የለም።
  ዱካዎች)፣ እና ፋይሉ ቅጥያ ሲጎድል የተዋቀረው ቅጥያ ይተገበራል።
  ክፍያው በቃል ወደ አሳሹ ከመተላለፉ በፊት።
- **`car`** - የዥረት ኢላማዎች በ`car_bridge.cache_dir` ውስጥ ይፈታሉ ፣
  የተዋቀረ ነባሪ ቅጥያ እና ካልሆነ በስተቀር የተጨመቁ ጭነቶችን ውድቅ ያድርጉ
  `allow_zst` ተዘጋጅቷል። የተሳካላቸው ድልድዮች ከ `STREAM_ACK_OK` በፊት ምላሽ ሰጥተዋል
  ደንበኞች የቧንቧ መስመር ማረጋገጥ እንዲችሉ የማህደሩን ባይት ማስተላለፍ።

በሁለቱም ሁኔታዎች ተኪው የመሸጎጫ መለያ ኤችኤምኤሲ (የጠባቂ መሸጎጫ ቁልፍ በነበረበት ጊዜ) ያቀርባል
በመጨባበጥ ወቅት ይገኛል) እና `norito_*` / `car_*` የቴሌሜትሪ ምክንያትን ይመዘግባል
ዳሽቦርዶች ስኬቶችን፣ የጎደሉ ፋይሎችን እና የንፅህና አጠባበቅን የሚለዩ ኮዶች
ውድቀቶች በጨረፍታ.

`Orchestrator::local_proxy().await` ጠሪዎች እንዲችሉ የሩጫ እጀታውን ያጋልጣል
PEM የምስክር ወረቀቱን አንብብ፣ የአሳሹን አንጸባራቂ አምጪ ወይም ሞገስን ጠይቅ
አፕሊኬሽኑ ሲወጣ መዝጋት።

ሲነቃ ተኪው አሁን *ማኒፌስት v2** መዝገቦችን ያገለግላል። ካለው በተጨማሪ
የምስክር ወረቀት እና የጥበቃ መሸጎጫ ቁልፍ፣ v2 ያክላል፡-

- `alpn` (`"sorafs-proxy/1"`) እና `capabilities` ድርድር ደንበኞች ማረጋገጥ እንዲችሉ
  እነሱ መናገር ያለባቸው የዥረት ፕሮቶኮል.
- በእጅ መጨባበጥ `session_id` እና መሸጎጫ መለያ ጨው (`cache_tagging` ብሎክ) ወደ
  በየክፍለ-ጊዜው የጥበቃ ቅርርብ እና የኤችኤምኤሲ መለያዎችን ያግኙ።
- የወረዳ እና ጠባቂ ምርጫ ፍንጮች (`circuit`፣ `guard_selection`፣
  `route_hints`) ስለዚህ የአሳሽ ውህደቶች ዥረቶች ከመድረሳቸው በፊት የበለፀገ UIን ሊያጋልጥ ይችላል
  ተከፍቷል።
- `telemetry_v2` ለአካባቢያዊ መሳሪያዎች ናሙና እና የግላዊነት ቁልፎች።
- እያንዳንዱ `STREAM_ACK_OK` `cache_tag_hex` ያካትታል። ደንበኞች እሴቱን ያንፀባርቃሉ
  ኤችቲቲፒ ወይም ቲሲፒ ጥያቄዎችን ሲያወጡ የ`x-sorafs-cache-tag` አርዕስት የተሸጎጡ
  የጥበቃ ምርጫዎች በእረፍት ጊዜ እንደተመሰጠሩ ይቆያሉ።

በ v1 ንዑስ ስብስብ ላይ መተማመንዎን ይቀጥሉ።

## 2. አለመሳካት የትርጉም

ኦርኬስትራተሩ ከአንድ ነጠላ በፊት ጥብቅ የአቅም እና የበጀት ፍተሻዎችን ያስፈጽማል
ባይት ተላልፏል. ውድቀቶች በሦስት ምድቦች ይከፈላሉ።

1. **የብቁነት አለመሳካቶች (የቅድመ በረራ)።
   ጊዜ ያለፈባቸው ማስታወቂያዎች፣ ወይም የቆየ ቴሌሜትሪ በውጤት ሰሌዳው አርቲፊክስ ውስጥ ገብተዋል።
   ከመርሐግብር ቀርቷል። የ CLI ማጠቃለያዎች `ineligible_providers` ይሞላሉ።
   ኦፕሬተሮች የአስተዳደር ድቀትን ሳይቧጥጡ ለመመርመር እንዲችሉ ምክንያቶችን ያደራጁ
   መዝገቦች.
2. **የሩጫ ጊዜ ድካም።** እያንዳንዱ አቅራቢ ተከታታይ አለመሳካቶችን ይከታተላል። አንዴ የ
   የተዋቀረ `provider_failure_threshold` ደርሷል፣ አቅራቢው ምልክት ተደርጎበታል።
   `disabled` ለቀሪው ክፍለ ጊዜ። እያንዳንዱ አቅራቢ ወደ ቢሸጋገር
   `disabled`፣ ኦርኬስትራተሩ ይመለሳል
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. ** ቆራጥ ውርጃዎች።** ጠንካራ ገደቦች እንደ የተዋቀሩ ስህተቶች ወለል።
   - `MultiSourceError::NoCompatibleProviders` - አንጸባራቂው ቁራጭ ይፈልጋል
     የቀሩት አቅራቢዎች ማክበር አይችሉም span ወይም አሰላለፍ.
   - `MultiSourceError::ExhaustedRetries` - በእያንዳንዱ-ቻንክ ድጋሚ ሙከራ በጀት ነበር።
     ተበላ።
   - `MultiSourceError::ObserverFailed` - የታችኛው ተፋሰስ ታዛቢዎች (የዥረት መንጠቆዎች)
     የተረጋገጠ ቁርጥራጭን ውድቅ አደረገው።

እያንዳንዱ ስህተት አፀያፊውን ቸንክ ኢንዴክስ እና፣ ሲገኝ፣ የመጨረሻውን ያካትታል
የአቅራቢው ውድቀት ምክንያት. እነዚህን እንደ መልቀቂያ ማገጃዎች ይያዙ - ተመሳሳይ ሙከራዎችን ያድርጉ
ግቤት ስህተቱን እስከ ዋናው ማስታወቂያ፣ ቴሌሜትሪ ወይም ድረስ ይደግማል
አቅራቢ የጤና ለውጦች.

### 2.1 የውጤት ሰሌዳ ጽናት

`persist_path` ሲዋቀር ኦርኬስትራ የመጨረሻውን የውጤት ሰሌዳ ይጽፋል
ከእያንዳንዱ ሩጫ በኋላ. የJSON ሰነዱ የሚከተሉትን ያጠቃልላል

- `eligibility` (`eligible` ወይም `ineligible::<reason>`)።
- `weight` (ለዚህ ሩጫ የተመደበ መደበኛ ክብደት)።
- `provider` ሜታዳታ (መለያ፣ የመጨረሻ ነጥቦች፣ የተመጣጣኝ በጀት)።

ከተለቀቁ ቅርሶች ጎን ለጎን የውጤት ሰሌዳ ቅጽበተ-ፎቶዎችን በማህደር በማህደር ጥቁር መዝገብ ውስጥ ያስገቡ
የልቀት ውሳኔዎች ኦዲት መደረጉን ይቀራሉ።

## 3. ቴሌሜትሪ እና ማረም

### 3.1 I18NT00000000 ሜትሪክስ

ኦርኬስትራተሩ የሚከተሉትን መለኪያዎች በ`iroha_telemetry` በኩል ያወጣል፡| መለኪያ | መለያዎች | መግለጫ |
|--------|--------|------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | በበረራ ውስጥ የተቀነባበሩ ፌችዎች መለኪያ። |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | ሂስቶግራም ከጫፍ እስከ ጫፍ የማምጣት መዘግየት። |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`፣ `region`፣ `reason` | የተርሚናል ውድቀቶች ቆጣሪ (ሙከራዎች ደክመዋል፣ አቅራቢዎች የሉም፣ የተመልካቾች አለመሳካት)። |
| `sorafs_orchestrator_retries_total` | `manifest_id`፣ `provider`፣ `reason` | በአገልግሎት አቅራቢው የድጋሚ ሙከራዎች ቆጣሪ። |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`፣ `provider`፣ `reason` | ወደ አካል ጉዳተኝነት የሚያመሩ የክፍለ-ደረጃ አቅራቢ ውድቀቶች ቆጣሪ። |
| `sorafs_orchestrator_policy_events_total` | `region`፣ `stage`፣ `outcome`፣ `reason` | በስም-አልባነት ፖሊሲ ውሳኔዎች ብዛት (የተገናኙት ከ ቡኒ ውጪ) በታቀደ ልቀት ደረጃ እና የመውደቅ ምክንያት። |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | ከተመረጠው የሶራኔት ስብስብ መካከል የPQ ቅብብሎሽ ድርሻ ሂስቶግራም። |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | በውጤት ሰሌዳ ቅጽበታዊ እይታ ውስጥ የPQ ቅብብል አቅርቦት ሬሾ ሂስቶግራም። |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | የፖሊሲ እጥረት ሂስቶግራም (በዒላማ እና በትክክለኛ PQ ድርሻ መካከል ያለው ክፍተት)። |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | በእያንዳንዱ ክፍለ ጊዜ ጥቅም ላይ የዋለው የክላሲካል ቅብብል ሂስቶግራም። |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | የክላሲካል ቅብብል ሂስቶግራም በአንድ ክፍለ ጊዜ ተመርጧል። |

የማምረቻ ቁልፎችን ከመገልበጥዎ በፊት መለኪያዎችን ወደ ዳሽቦርዶች ዝግጅት ያዋህዱ።
የሚመከረው አቀማመጥ የSF-6 ታዛቢነት እቅድን ያንጸባርቃል፡-

1. ** ገባሪ ፈልሳፊዎች *** - መለኪያው ሳይመጣጠን ሲወጣ ያስጠነቅቃል።
2. ** ጥምርታ ድጋሚ ሞክር *** — `retry` ቆጣሪዎች ከታሪካዊ መነሻዎች ሲበልጡ ያስጠነቅቃል።
3. **የአቅራቢዎች አለመሳካቶች *** - ማንኛውም አቅራቢ ሲሻገር የፔጀር ማንቂያዎችን ያነሳሳል።
   `session_failure > 0` በ15 ደቂቃ ውስጥ።

### 3.2 የተዋቀሩ የምዝግብ ማስታወሻዎች

ኦርኬስትራተሩ ዒላማዎችን ለመወሰን የተዋቀሩ ክስተቶችን ያትማል፡-

- `telemetry::sorafs.fetch.lifecycle` — `start` እና `complete` የህይወት ዑደት
  ምልክት ማድረጊያዎች ከቁመቶች፣ ሙከራዎች እና አጠቃላይ ቆይታ ጋር።
- `telemetry::sorafs.fetch.retry` — ክስተቶችን እንደገና ይሞክሩ (`provider`፣ I18NI0000248X፣
  `attempts`) በእጅ የመለየት ሂደት ውስጥ ለመመገብ።
- `telemetry::sorafs.fetch.provider_failure` - አቅራቢዎች በዚህ ምክንያት ተሰናክለዋል።
  ተደጋጋሚ ስህተቶች.
- `telemetry::sorafs.fetch.error` - የተርሚናል ውድቀቶች ጠቅለል ያለ
  `reason` እና አማራጭ አቅራቢ ዲበ ውሂብ።

እነዚህን ዥረቶች ወደ ነባሩ Norito የምዝግብ ማስታወሻ ቧንቧ መስመር አስተላልፍ ስለዚህ የአደጋ ምላሽ
አንድ የእውነት ምንጭ አለው። የህይወት ዑደት ክስተቶች PQ/ ክላሲካል ድብልቅን በ በኩል ያጋልጣሉ
`anonymity_effective_policy`፣ `anonymity_pq_ratio`፣
`anonymity_classical_ratio`፣ እና የእነርሱ አጋዥ ቆጣሪዎች፣
መለኪያዎችን ሳይቧጭ ወደ ሽቦ ዳሽቦርዶች ቀጥተኛ ማድረግ። ወቅት
GA መልቀቅ፣ ለህይወት ኡደት/ለክስተቶች ዳግም ሞክር የምዝግብ ማስታወሻውን ደረጃ ከ `info` ጋር ይሰኩት እና በ
`warn` ለተርሚናል ስህተቶች።

### 3.3 JSON ማጠቃለያዎች

ሁለቱም `sorafs_cli fetch` እና Rust SDK የሚከተሉትን የያዘ የተዋቀረ ማጠቃለያ ይመለሳሉ፡-

- `provider_reports` በስኬት/በሽንፈት ቆጠራዎች እና አቅራቢው እንደነበረ
  አካል ጉዳተኛ
- `chunk_receipts` የትኛውን አቅራቢ እያንዳንዱን ቁራጭ እንዳረካ በዝርዝር ይገልጻል።
- `retry_stats` እና `ineligible_providers` ድርድሮች።

የተሳሳቱ አቅራቢዎችን ሲያርሙ የማጠቃለያ ፋይሉን በማህደር ያስቀምጡ—የደረሰኝ ካርታ
በቀጥታ ከላይ ወዳለው የምዝግብ ማስታወሻ ሜታዳታ።

## 4. የተግባር ማረጋገጫ ዝርዝር

1. **የደረጃ ውቅር በCI.** `sorafs_fetch`ን ከዒላማው ጋር ያሂዱ
   የማዋቀር፣ የብቃት እይታን ለመያዝ `--scoreboard-out` ማለፍ እና
   ከቀዳሚው ልቀት ጋር ልዩነት። ማንኛውም ያልተጠበቀ ብቁ ያልሆነ አቅራቢ ይቆማል
   ማስተዋወቂያው ።
2. ** ቴሌሜትሪ ያረጋግጡ።** ወደ ውጭ መላኩን ያረጋግጡ `sorafs.fetch.*`
   ለተጠቃሚዎች ባለብዙ ምንጭ ፈልጎዎችን ከማንቃትዎ በፊት መለኪያዎች እና የተዋቀሩ ምዝግብ ማስታወሻዎች።
   የመለኪያዎች አለመኖር በተለምዶ የኦርኬስትራ ፊት ለፊት እንዳልሆነ ያሳያል
   ተጠርቷል ።
3. **ሰነድ ይሽራል።** ድንገተኛ ሁኔታ ሲያመለክቱ `--deny-provider` ወይም
   የ`--boost-provider` ቅንጅቶች፣ የJSON (ወይም CLI ጥሪ) ለእርስዎ ይስጡ
   መዝገብ ይቀይሩ. Rollbacks መሻርን መመለስ እና አዲስ የውጤት ሰሌዳ መያዝ አለባቸው
   ቅጽበታዊ ገጽ እይታ
4. **የጭስ ሙከራዎችን እንደገና ያካሂዱ።** በጀት ወይም የአገልግሎት አቅራቢዎችን እንደገና ይሞክሩ።
   ቀኖናዊውን መሳሪያ (`fixtures/sorafs_manifest/ci_sample/`) እና
   ቁርጥራጭ ደረሰኞች የሚወስኑ መሆናቸውን ያረጋግጡ።

ከላይ ያሉትን ደረጃዎች መከተል የኦርኬስትራ ባህሪን በሁሉም መልኩ እንዲባዛ ያደርገዋል
የታቀደ ልቀቶች እና ለአደጋ ምላሽ አስፈላጊ የሆነውን ቴሌሜትሪ ያቀርባል።

### 4.1 ፖሊሲ ይሽራል።

ኦፕሬተሮች ንቁውን የትራንስፖርት/ስም-መታወቅ ደረጃን ሳያርትዑ ፒን ማድረግ ይችላሉ።
ቤዝ ውቅር `policy_override.transport_policy` እና በማቀናበር
`policy_override.anonymity_policy` በእነሱ `orchestrator` JSON (ወይም በማቅረብ ላይ)
`--transport-policy-override=` / `--anonymity-policy-override=` ወደ
`sorafs_cli fetch`)። የትኛውም መሻር ሲኖር ኦርኬስትራውን ይዘላል
የተለመደው ቡኒውት ውድቀት፡ የተጠየቀው የPQ እርከን ማርካት ካልቻለ፣ እ.ኤ.አ
በጸጥታ ደረጃ ዝቅ ከማድረግ ይልቅ በ`no providers` ማግኘት አልተሳካም። ወደ ኋላ ተመለስ
ነባሪ ባህሪ የተሻሩ መስኮችን እንደ ማጽዳት ቀላል ነው።

መደበኛው `iroha_cli app sorafs fetch` ትዕዛዝ ተመሳሳይ የመሻር ባንዲራዎችን ያጋልጣል፣
የማስታወቂያ ሰጭዎችን እና አውቶማቲክ ስክሪፕቶችን ወደ ጌትዌይ ደንበኛ ማስተላለፍ
ተመሳሳይ ደረጃ የመገጣጠም ባህሪን ያካፍሉ።

ክሮስ-ኤስዲኬ መጫዎቻዎች በ`fixtures/sorafs_gateway/policy_override/` ስር ይኖራሉ። የ
CLI፣ Rust ደንበኛ፣ የጃቫስክሪፕት ማሰሪያዎች እና የስዊፍት ትጥቆች መፍታት
`override.json` በተመጣጣኝ ስብስቦች ውስጥ ስለዚህ ማንኛውም ለውጥ የሚከፈል ጭነት
መሣሪያውን ማዘመን እና `cargo test -p iroha`፣ `npm test`፣ እና እንደገና ማስኬድ አለበት።
`swift test` ኤስዲኬዎች እንዲሰለፉ ለማድረግ። ሁልጊዜ የታደሰውን እቃ ያያይዙት።
የታችኛው ተፋሰስ ሸማቾች የመሻር ውሉን እንዲለያዩ ግምገማን ይቀይሩ።

አስተዳደር ለእያንዳንዱ መሻር የ runbook ግቤት ያስፈልገዋል። ምክንያቱን ይመዝግቡ ፣
የሚጠበቀው የቆይታ ጊዜ፣ እና በለውጥ ምዝግብ ማስታወሻዎ ውስጥ የመመለሻ ቀስቅሴ፣ PQውን ያሳውቁ
የዝውውር ቻናልን ያውጡ፣ እና የተፈረመውን ማጽደቅ ከተመሳሳይ ቅርስ ጋር ያያይዙት።
የውጤት ሰሌዳውን ቅጽበታዊ ገጽ እይታ የሚያከማች ጥቅል። መሻር ለአጭር ጊዜ የታሰበ ነው።
ድንገተኛ ሁኔታዎች (ለምሳሌ, PQ guard brownouts); የረጅም ጊዜ የፖሊሲ ለውጦች መሄድ አለባቸው
በመደበኛ ምክር ቤት ድምጽ በኩል አንጓዎች በአዲሱ ነባሪ ላይ ይሰበሰባሉ.

### 4.2 PQ Ratchet Fire Drill

- ** Runbook: ** `docs/source/soranet/pq_ratchet_runbook.md` ን ይከተሉ
  የማስተዋወቅ/የማውረድ ልምምድ፣ የጥበቃ-ዳይሬክተሩ አያያዝ እና መልሶ መመለስን ጨምሮ።
- ** ዳሽቦርድ: *** ለመከታተል `dashboards/grafana/soranet_pq_ratchet.json` አስመጣ
  `sorafs_orchestrator_policy_events_total`፣ ቡኒ መውጫ ተመን እና የPQ ጥምርታ ማለት ነው።
  በመሰርሰሪያው ወቅት.
- ** አውቶሜትድ: *** `cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics`
  ተመሳሳይ ሽግግሮችን ይለማመዳል እና ያንን መለኪያዎች እንደ መጨመር ያረጋግጣል
  ኦፕሬተሮች የቀጥታ መሰርሰሪያውን ከማስኬዳቸው በፊት ይጠበቃል።

## 5. የመጫወቻ መጽሐፍትን መልቀቅ

የ SNNet-5 የትራንስፖርት ልቀት አዲስ የጥበቃ ምርጫን፣ አስተዳደርን ያስተዋውቃል
ማረጋገጫዎች እና የፖሊሲ ውድቀቶች። ከዚህ በታች ያሉት የመጫወቻ መጽሐፍት ቅደም ተከተሎችን ያመለክታሉ
ለዋና ተጠቃሚዎች የብዝሃ-ምንጭ መፈለጊያዎችን ከማንቃትዎ በፊት እና ደረጃውን ዝቅ ለማድረግ ይከተሉ
ወደ ቀጥታ ሁነታ የሚመለስ መንገድ.

### 5.1 ገንቢ ቅድመ በረራ (CI / Staging)

1. ** የውጤት ሰሌዳዎችን በCI ውስጥ ያድሱ።** `sorafs_cli fetch` (ወይም ኤስዲኬን) ያሂዱ።
   ተመጣጣኝ) ከ `fixtures/sorafs_manifest/ci_sample/` አንጸባራቂ ጋር
   የእጩ ውቅር. የውጤት ሰሌዳውን በ በኩል ቀጥል
   `--scoreboard-out=artifacts/sorafs/scoreboard.json` እና አስረግጠው፡
   - `anonymity_status=="met"` እና `anonymity_pq_ratio` የታለመውን ያሟላሉ
     ደረጃ (`anon-guard-pq`፣ `anon-majority-pq`፣ ወይም `anon-strict-pq`)።
   - የመወሰኛ ቁርጥራጭ ደረሰኞች አሁንም ከወርቃማው ስብስብ ጋር ይዛመዳሉ
     ማከማቻው ።
2. **አንጸባራቂ አስተዳደርን አረጋግጥ።** የCLI/SDK ማጠቃለያን መርምር እና አረጋግጥ
   አዲስ የሚታየው `manifest_governance.council_signatures` ድርድር ይዟል
   የሚጠበቁ የምክር ቤት አሻራዎች. ይህ የጌትዌይ ምላሾችን GAR መላክን ያረጋግጣል
   ኤንቨሎፕ እና ያ `validate_manifest` ተቀብሎታል።
3. **የአካል ብቃት እንቅስቃሴ ተገዢነትን ይሽራል።** እያንዳንዱን የዳኝነት መገለጫ ከ
   `docs/examples/sorafs_compliance_policy.json` እና ኦርኬስትራውን አስረግጠው
   ትክክለኛውን የፖሊሲ ውድቀት ያወጣል (`compliance_jurisdiction_opt_out` ወይም
   `compliance_blinded_cid_opt_out`)። ቁ
   ታዛዥ መጓጓዣዎች አሉ።
4. **ማውረድን አስመስለው።** `transport_policy` ወደ `direct-only` ገልብጥ በ
   በሙከራ ላይ ያለ ማዋቀር እና ኦርኬስትራውን ለማረጋገጥ ፈልጎውን እንደገና ያስኪዱ
   SoraNet relaysን ሳይነካ ወደ I18NT0000010X/QUIC ይወድቃል። ይህን JSON አቆይ
   በስሪት ቁጥጥር ስር ያለ ልዩነት ስለዚህ በፍጥነት ማስተዋወቅ በኤ
   ክስተት.

### 5.2 ኦፕሬተር ልቀት (የምርት ሞገዶች)1. ** አወቃቀሩን በ`iroha_config` በኩል ደረጃ ያድርጉት።** ጥቅም ላይ የዋለውን JSON በትክክል ያትሙ።
   በCI ውስጥ እንደ I18NI0000302X ንብርብር መሻር። ኦርኬስትራውን ፖድ / ሁለትዮሽ ያረጋግጡ
   ጅምር ላይ አዲሱን የውቅር hash ይመዘግባል።
2. ** የፕሪም ጠባቂ መሸጎጫዎች።** የማስተላለፊያ ማውጫውን በ`--guard-directory` ያድሱ
   እና የ Norito የጥበቃ መሸጎጫ ከ `--guard-cache` ጋር ቀጥል። መሸጎጫውን ያረጋግጡ
   የተፈረመ (`--guard-cache-key` ከተዋቀረ) እና በስሪት ውስጥ ተከማችቷል።
   መቆጣጠሪያ ለውጥ.
3. **የቴሌሜትሪ ዳሽቦርዶችን ያንቁ።** የተጠቃሚ ትራፊክን ከማቅረብዎ በፊት ያረጋግጡ
   አካባቢ `sorafs.fetch.*`ን፣ `sorafs_orchestrator_policy_events_total`ን እና
   የተኪ መለኪያዎች (የአካባቢውን QUIC ፕሮክሲ ሲጠቀሙ)። ማንቂያዎች መታሰር አለባቸው
   `anonymity_brownout_effective` እና ተገዢ የመመለሻ ቆጣሪዎች።
4. **የቀጥታ የጭስ ሙከራዎችን ያካሂዱ።**በእያንዳንዱ በኩል በአስተዳደር የጸደቀ ሰነድ ይምጡ
   የአቅራቢዎች ስብስብ (PQ፣ ክላሲካል እና ቀጥተኛ) እና ቁርጥራጭ ደረሰኞችን ያረጋግጡ፣
   የCAR መፈጨት፣ እና የምክር ቤት ፊርማዎች ከCI መነሻ መስመር ጋር ይዛመዳሉ።
5. **ማግበር ተገናኝ።** የታቀደውን መከታተያ በ. ያዘምኑ
   `scoreboard.json` አርቴፍክት፣ የጥበቃ መሸጎጫ አሻራ፣ እና አገናኝ
   ለመጀመሪያው የምርት ማምጣት አንጸባራቂ አስተዳደር ማረጋገጫን የሚያሳዩ ምዝግብ ማስታወሻዎች።

### 5.3 የማውረድ/የመመለሻ ሂደት

ክስተቶች፣ የPQ ጉድለቶች ወይም የቁጥጥር ጥያቄዎች መልሶ መመለስን ሲያስገድዱ ይከተሉ
ይህ የመወሰን ቅደም ተከተል

1. ** የትራንስፖርት ፖሊሲን ቀይር።** `transport_policy=direct-only` ያመልክቱ (እና ከሆነ
   አዲሱን የሶራኔት ወረዳ ግንባታ አቁሟል።
2. **Flush guard state.** የተጠቀሰውን የጥበቃ መሸጎጫ ፋይል ሰርዝ ወይም በማህደር አስቀምጥ
   `--guard-cache` ስለዚህ ተከታይ ሩጫዎች የተሰኩ ሪሌይዎችን እንደገና ለመጠቀም አይሞክሩም።
   ይህን ደረጃ ይዝለሉት ፈጣን ዳግም ማንቃት ሲታቀድ እና መሸጎጫው ሲቀር ብቻ ነው።
   ልክ ነው።
3. **አካባቢያዊ ፕሮክሲዎችን አሰናክል።** የአካባቢው የQUIC ፕሮክሲ በ`bridge` ሁነታ ከሆነ፣
   ኦርኬስትራውን በ `proxy_mode="metadata-only"` እንደገና ያስጀምሩት ወይም ያስወግዱት።
   `local_proxy` ሙሉ በሙሉ አግድ። የወደብ መልቀቂያውን ስለዚህ የሥራ ቦታ እና
   የአሳሽ ውህደቶች ወደ Torii መዳረሻ ይመለሳሉ።
4. **ተገዢነትን ያጽዱ
   ዓይነ ስውር-CID ግቤት) ለተጎዱት ጭነቶች ተገዢነት ፖሊሲ
   አውቶሜሽን እና ዳሽቦርዶች ሆን ተብሎ ቀጥተኛ ሁነታን ያንፀባርቃሉ።
5. **የኦዲት ማስረጃዎችን ይያዙ።** ከለውጥ በኋላ ለማምጣት በ`--scoreboard-out` ያሂዱ።
   እና የCLI JSON ማጠቃለያ (`manifest_governance`ን ጨምሮ) ያከማቹ።
   የክስተቱ ትኬት.

### 5.4 የተስተካከለ የሥምሪት ማረጋገጫ ዝርዝር

| የፍተሻ ነጥብ | ዓላማ | የሚመከር ማስረጃ |
|--------|--------|
| ተገዢነት ፖሊሲ ደረጃ | ከ GAR መዛግብት ጋር የተጣጣመውን የዳኝነት ስልጣኑን ያረጋግጣል። | የተፈረመ `soranet_opt_outs.json` ቅጽበታዊ ገጽ እይታ + ኦርኬስትራ ውቅር ልዩነት። |
| አንጸባራቂ አስተዳደር ተመዝግቧል | የማረጋገጫ ምክር ቤት ፊርማዎች ከእያንዳንዱ የመግቢያ ሰነድ ጋር አብረው ይመጣሉ። | `sorafs_cli fetch ... --output /dev/null --summary out.json` ከ `manifest_governance.council_signatures` ጋር ተቀምጧል። |
| የማረጋገጫ ክምችት | በ `compliance.attestations` ውስጥ የተጠቀሱትን ሰነዶች ይከታተላል። | ፒዲኤፍ/JSON ቅርሶችን ከማስረጃው መፍጨት እና የአገልግሎት ማብቂያው ጎን ያከማቹ። |
| የማውረድ ቁፋሮ ተመዝግቧል | ወደ ኋላ መመለስ የሚወስን መሆኑን ያረጋግጣል። | የሩብ ጊዜ የደረቅ አሂድ መዝገብ በቀጥታ-ብቻ ፖሊሲ ተተግብሯል እና የጥበቃ መሸጎጫ ጸድቷል። |
| ቴሌሜትሪ ማቆየት | ለተቆጣጣሪዎች የፎረንሲክ መረጃ ያቀርባል። | `sorafs.fetch.*` የሚያረጋግጥ ዳሽቦርድ ወደ ውጭ መላክ ወይም የኦቲኤል ቅጽበታዊ ገጽ እይታ እና የማክበር ውድቀቶች በፖሊሲ እንዲቆዩ እየተደረገ ነው። |

ኦፕሬተሮች ከእያንዳንዱ የታቀፈ መስኮት በፊት የማረጋገጫ ዝርዝሩን መከለስ እና ከማቅረባቸው በፊት መሆን አለባቸው
ማስረጃው ጥቅል ለአስተዳደር ወይም ተቆጣጣሪዎች በተጠየቀ ጊዜ. ገንቢዎች እንደገና መጠቀም ይችላሉ።
ቡኒዎች ወይም ተገዢነት ሲሻሩ ለድህረ-ሞት ፓኬቶች ተመሳሳይ ቅርሶች
በሙከራ ጊዜ ይነሳሉ.