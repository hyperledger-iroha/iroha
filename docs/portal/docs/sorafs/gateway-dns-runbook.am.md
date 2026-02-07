---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 672a5e3a6f0c3e8999400bc6fa8c66cc3be1ba2119431c5fd26f6d9a436f767f
source_last_modified: "2025-12-29T18:16:35.187152+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS ጌትዌይ እና ዲ ኤን ኤስ Kickoff Runbook

ይህ የፖርታል ቅጂ ቀኖናዊውን የሩጫ መጽሐፍን ያንጸባርቃል
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md)።
ያልተማከለ ዲ ኤን ኤስ እና ጌትዌይ ኦፕሬሽናል ጥበቃ መንገዶችን ይይዛል
የስራ ዥረት ስለዚህ አውታረ መረብ፣ ኦፕስ እና የሰነድ እርሳሶች ይለማመዱ
አውቶሜሽን ቁልል ከ2025-03 ጅምር በፊት።

## ወሰን እና ሊደርስ የሚችል

- ቆራጥነትን በመለማመድ ዲ ኤን ኤስ (SF-4) እና መግቢያ (SF-5) ዋና ዋና ክስተቶችን ያስሩ።
  የአስተናጋጅ አመጣጥ፣ የፈታሽ ማውጫ ልቀቶች፣ TLS/GAR አውቶማቲክ፣ እና ማስረጃ
  መያዝ.
- የመክፈቻ ግብአቶችን (አጀንዳ፣ ግብዣ፣ የመገኘት መከታተያ፣ GAR ቴሌሜትሪ) አቆይ
  ቅጽበተ-ፎቶ) ከቅርብ ጊዜ የባለቤትነት ስራዎች ጋር ተመሳስሏል።
- ለአስተዳደራዊ ገምጋሚዎች ኦዲት ሊደረግ የሚችል የቅርስ ቅርቅብ ያዘጋጁ፡ ፈቺ
  የማውጫ መልቀቂያ ማስታወሻዎች፣ የጌትዌይ መጠይቅ ምዝግብ ማስታወሻዎች፣ የተስማሚ ማሰሪያ ውፅዓት እና
  የሰነዶች/DevRel ማጠቃለያ።

## ሚናዎች እና ኃላፊነቶች

| የስራ ፍሰት | ኃላፊነቶች | አስፈላጊ ቅርሶች |
|-------------|
| አውታረ መረብ TL (ዲ ኤን ኤስ ቁልል) | የሚወስን አስተናጋጅ እቅድን አቆይ፣ የ RAD ማውጫ ልቀቶችን ያስኬዱ፣ የፈታ የቴሌሜትሪ ግብዓቶችን ያትሙ። | `artifacts/soradns_directory/<ts>/`፣ ለI18NI0000014X፣ RAD ሜታዳታ ልዩነት። |
| Ops አውቶሜሽን መሪ (በረኛው) | TLS/ECH/GAR አውቶሜሽን ልምምዶችን ያስፈጽም፣ `sorafs-gateway-probe` ን ያሂዱ፣ PagerDuty መንጠቆዎችን ያዘምኑ። | `artifacts/sorafs_gateway_probe/<ts>/`፣ probe JSON፣ I18NI0000017X ግቤቶች። |
| QA Guild & Tooling WG | `ci/check_sorafs_gateway_conformance.sh`ን፣ የተስተካከሉ ዕቃዎችን፣ ማህደርን Norito የራስ ሰር ማረጋገጫ ቅርቅቦችን ያሂዱ። | `artifacts/sorafs_gateway_conformance/<ts>/`፣ `artifacts/sorafs_gateway_attest/<ts>/`። |
| ሰነዶች / DevRel | ደቂቃዎችን ያንሱ፣ የንድፍ ቅድመ-ንባብ + ተጨማሪዎችን ያዘምኑ እና የማስረጃ ማጠቃለያውን በዚህ ፖርታል ላይ ያትሙ። | የዘመኑ `docs/source/sorafs_gateway_dns_design_*.md` ፋይሎች እና የታቀዱ ማስታወሻዎች። |

## ግብዓቶች እና ቅድመ ሁኔታዎች

- ቆራጥ አስተናጋጅ ዝርዝር (`docs/source/soradns/deterministic_hosts.md`) እና የ
  የመፍታት ማረጋገጫ ስካፎልዲንግ (`docs/source/soradns/resolver_attestation_directory.md`)።
- የጌትዌይ ቅርሶች፡ ከዋኝ መመሪያ መጽሐፍ፣ TLS/ECH አውቶማቲክ አጋዥዎች፣
  ቀጥተኛ ሁነታ መመሪያ እና በ`docs/source/sorafs_gateway_*` ስር በራስ-የተረጋገጠ የስራ ፍሰት።
- መሳሪያ: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`፣ `scripts/telemetry/run_soradns_transparency_tail.sh`፣
  `scripts/sorafs_gateway_self_cert.sh`፣ እና CI ረዳቶች
  (`ci/check_sorafs_gateway_conformance.sh`፣ `ci/check_sorafs_gateway_probe.sh`)።
- ሚስጥሮች፡ የጋር መልቀቂያ ቁልፍ፣ የዲኤንኤስ/TLS ACME ምስክርነቶች፣ PagerDuty ማዞሪያ ቁልፍ፣
  Torii auth token ለመፍትሄ ሰጪዎች።

## የቅድመ በረራ ማረጋገጫ ዝርዝር

1. በማዘመን ተሳታፊዎችን እና አጀንዳዎችን ያረጋግጡ
   `docs/source/sorafs_gateway_dns_design_attendance.md` እና በማሰራጨት ላይ
   የአሁኑ አጀንዳ (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. ደረጃ artefact ሥሮች እንደ
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` እና
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. ማሰራጫዎችን ያድሱ (GAR መግለጫዎች፣ RAD ማረጋገጫዎች፣ የጌትዌይ ስምምነት ቅርቅቦች) እና
   `git submodule` ሁኔታ ከአዲሱ የመለማመጃ መለያ ጋር መዛመዱን ያረጋግጡ።
4. ሚስጥሮችን ያረጋግጡ (Ed25519 የመልቀቂያ ቁልፍ፣ የACME መለያ ፋይል፣ PagerDuty token)
   አሁን እና ግጥሚያ ቮልት ቼኮች.
5. የጭስ ሙከራ የቴሌሜትሪ ኢላማዎች (የፑሽጌት ዌይ መጨረሻ ነጥብ፣ GAR Grafana ሰሌዳ)
   ወደ መሰርሰሪያው.

## አውቶሜሽን የመለማመጃ ደረጃዎች

### ቆራጥ አስተናጋጅ ካርታ እና RAD ማውጫ ልቀት

1. ከታቀደው አንጸባራቂ አንጻር የሚወስነውን አስተናጋጅ ረዳትን ያሂዱ
   አዘጋጅ እና ምንም ተንሳፋፊ አለመኖሩን ያረጋግጡ
   `docs/source/soradns/deterministic_hosts.md`.
2. የመፍትሄ አፈላላጊ ማውጫ ቅርቅብ ይፍጠሩ፡

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. በውስጡ የታተመውን ማውጫ መታወቂያ፣ SHA-256 እና የውጤት መንገዶችን ይመዝግቡ
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` እና kickoff
   ደቂቃዎች ።

### የዲ ኤን ኤስ ቴሌሜትሪ ቀረጻ

- የጅራት መፍቻ ግልጽነት ምዝግብ ማስታወሻዎች ለ ≥10 ደቂቃዎች በመጠቀም
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- የPushgateway መለኪያዎችን ወደ ውጭ ላክ እና የNDJSON ቅጽበተ-ፎቶዎችን ከሩጫው ጋር በማህደር ያስቀምጡ
  የመታወቂያ ማውጫ.

### ጌትዌይ አውቶሜሽን ልምምዶች

1. የTLS/ECH ምርመራን ያስፈጽሙ፡-

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. የኮንፎርማንስ ማሰሪያውን (`ci/check_sorafs_gateway_conformance.sh`) እና
   ለማደስ የራስ ሰር ማረጋገጫ ረዳት (`scripts/sorafs_gateway_self_cert.sh`)
   Norito የማረጋገጫ ጥቅል።
3. የፔጀርዱቲ/የዌብሆክ ክስተቶችን ያንሱ አውቶሜሽን ዱካ መጠናቀቁን ለማረጋገጥ
   መጨረሻ።

### ማስረጃ ማሸግ

- `ops/drill-log.md` በጊዜ ማህተሞች፣ በተሳታፊዎች እና በመመርመሪያ ሃሽ ያዘምኑ።
- ቅርሶችን በሩጫ መታወቂያ ማውጫዎች ስር ያከማቹ እና አስፈፃሚ ማጠቃለያ ያትሙ
  በDocs/DevRel የስብሰባ ደቂቃዎች ውስጥ።
- ከጅማሬው ግምገማ በፊት በአስተዳደር ትኬት ውስጥ ያለውን የማስረጃ ጥቅል ያገናኙ።

## የክፍለ ጊዜ ማመቻቸት እና ማስረጃዎች እጅ መስጠት

- **አወያይ የጊዜ መስመር፡**  
  - ቲ-24 ሰ - የፕሮግራም አስተዳደር አስታዋሹን + አጀንዳ/የተገኝነት ቅጽበታዊ ገጽ እይታን በ`#nexus-steering` ውስጥ ይለጠፋል።  
  - ቲ-2ሰ — ኔትዎርክቲንግ ቲኤል የGAR telemetry ቅጽበተ ፎቶን ያድሳል እና በ`docs/source/sorafs_gateway_dns_design_gar_telemetry.md` ውስጥ ዴልታዎችን ይመዘግባል።  
  - T-15m — Ops Automation የፍተሻ ዝግጁነት ያረጋግጣል እና የነቃውን የሩጫ መታወቂያ ወደ `artifacts/sorafs_gateway_dns/current` ይጽፋል።  
  - በጥሪው ጊዜ - አወያይ ይህንን የሩጫ መጽሐፍ ያካፍላል እና የቀጥታ ጸሐፊ ይመድባል; ሰነዶች/DevRel የእርምጃ ንጥሎችን በመስመር ውስጥ ይይዛሉ።
- ** የደቂቃ አብነት:** አጽሙን ከ
  `docs/source/sorafs_gateway_dns_design_minutes.md` (በተጨማሪም በፖርታሉ ውስጥ ተንጸባርቋል
  ጥቅል) እና በአንድ ክፍለ ጊዜ አንድ የተሞላ ምሳሌ ያከናውኑ። የተመልካቾችን ዝርዝር ያካትቱ፣
  ውሳኔዎች፣ የተግባር እቃዎች፣ የማስረጃ ሃሽ እና አስደናቂ አደጋዎች።
- **የማስረጃ ሰቀላ፡** የ `runbook_bundle/` ማውጫን ከልምምድ፣
  የተተረጎሙትን ደቂቃዎች PDF ያያይዙ፣ SHA-256 hashes በደቂቃ + አጀንዳ ውስጥ ይቅረጹ፣
  ከዚያ ፒንግ የአስተዳደር ገምጋሚ ቅጽል ስም አንዴ ሰቀላ
  `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## የማስረጃ ቅጽበታዊ ገጽ እይታ (መጋቢት 2025 መጀመሪያ)

በፍኖተ ካርታው እና በአስተዳደር ውስጥ የተጠቀሱ የቅርብ ጊዜ ልምምዶች/ቀጥታ ቅርሶች
ደቂቃዎች በ `s3://sora-governance/sorafs/gateway_dns/` ባልዲ ስር ይኖራሉ። Hashes
ቀኖናዊውን አንጸባራቂ (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`) ከመስተዋት በታች።

- ** ደረቅ ሩጫ - 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - ጥቅል ታርቦል፡ `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - ደቂቃዎች ፒዲኤፍ: I18NI0000052X
- ** የቀጥታ አውደ ጥናት - 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(ሰቀላ በመጠባበቅ ላይ፡ I18NI0000060X — Docs/DevRel SHA-256 ከተሰራው ፒዲኤፍ መሬቶች ጥቅል ውስጥ አንዴ ይጨመራል።)_

## ተዛማጅ ቁሳቁስ

- [የጌትዌይ ኦፕሬሽኖች መጫወቻ መጽሐፍ](./operations-playbook.md)
- [SoraFS ታዛቢነት እቅድ](./observability-plan.md)
- [ያልተማከለ ዲ ኤን ኤስ እና ጌትዌይ መከታተያ](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)