---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> ከ[`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md) የተወሰደ።

# SoraFS የአቅራቢ መግቢያ እና የማንነት ፖሊሲ (SF-2b ረቂቅ)

ይህ ማስታወሻ ለ **SF-2b** ተግባራዊ ሊሆኑ የሚችሉ አቅርቦቶችን ይይዛል፡ መግለፅ እና
የመግቢያ የስራ ሂደትን፣ የማንነት መስፈርቶችን እና ማረጋገጫን ማስፈጸም
ለSoraFS ማከማቻ አቅራቢዎች የሚከፈል ጭነት። የከፍተኛ ደረጃ ሂደቱን ያሰፋዋል
በI18NT0000011X Architecture RFC ውስጥ ተዘርዝሯል እና የቀረውን ስራ ወደ
መከታተል የሚችሉ የምህንድስና ተግባራት.

## የፖሊሲ ግቦች

- የተረጋገጡ ኦፕሬተሮች ብቻ የ `ProviderAdvertV1` መዝገቦችን ማተም እንደሚችሉ ያረጋግጡ
  አውታረ መረብ ይቀበላል.
- እያንዳንዱን የማስታወቂያ ቁልፍ በአስተዳደር ከተረጋገጠ የማንነት ሰነድ ጋር ማሰር፣
  የተመሰከረላቸው የመጨረሻ ነጥቦች፣ እና ዝቅተኛው የካስማ መዋጮ።
- Torii፣ መግቢያ መንገዶች እና የማረጋገጫ መሳሪያዎችን ያቅርቡ
  `sorafs-node` ተመሳሳይ ቼኮችን ያስፈጽማል።
- እድሳት እና የአደጋ ጊዜ መሻርን ይደግፉ ቆራጥነት ሳይጣሱ ወይም
  መሣሪያ ergonomics.

## የማንነት እና የካስማ መስፈርቶች

| መስፈርት | መግለጫ | ሊደርስ የሚችል |
|------------|------------|------------|
| የማስታወቂያ ቁልፍ provenance | አቅራቢዎች እያንዳንዱን ማስታወቂያ የሚፈርም የ Ed25519 ኪይ ጥንድ መመዝገብ አለባቸው። የመግቢያ ቅርቅቡ የህዝብ ቁልፉን ከአስተዳደር ፊርማ ጋር ያከማቻል። | የ`ProviderAdmissionProposalV1` እቅድን ከI18NI0000029X (32 ባይት) ጋር ያራዝሙ እና ከመዝገቡ (`sorafs_manifest::provider_admission`) ያጣቅሱት። |
| የካስማ ጠቋሚ | መግቢያ ዜሮ ያልሆነ I18NI0000031X ወደ ገባሪ ስቴኪንግ ገንዳ የሚያመለክት ያስፈልገዋል። | በ`sorafs_manifest::provider_advert::StakePointer::validate()` ውስጥ ማረጋገጫን እና የገጽታ ስህተቶችን በCLI/ሙከራዎች ይጨምሩ። |
| ስልጣን መለያዎች | አቅራቢዎች ስልጣንን + ህጋዊ ግንኙነትን ያውጃሉ። | ከ`jurisdiction_code` (ISO 3166-1 alpha-2) እና ከተፈለገ `contact_uri` ጋር የፕሮፖዛል እቅድን ያራዝሙ። |
| የመጨረሻ ነጥብ ማረጋገጫ | እያንዳንዱ የማስታወቂያ የመጨረሻ ነጥብ በmTLS ወይም QUIC ሰርተፍኬት ሪፖርት መደገፍ አለበት። | `EndpointAttestationV1` I18NT0000000X ክፍያን ይግለጹ እና በእያንዳንዱ የመጨረሻ ነጥብ በመግቢያ ቅርቅብ ውስጥ ያከማቹ። |

## የመግቢያ የስራ ፍሰት

1. ** ፕሮፖዛል መፍጠር ***
   - CLI: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal …` ይጨምሩ
     `ProviderAdmissionProposalV1` + የማረጋገጫ ጥቅል በማዘጋጀት ላይ።
   - ማረጋገጫ፡ አስፈላጊ የሆኑትን መስኮች፣ አክሲዮን> 0፣ ቀኖናዊ ቻንከር እጀታ በ`profile_id`።
2. **የመንግስት ድጋፍ**
   - ካውንስል `blake3("sorafs-provider-admission-v1" || canonical_bytes)` ነባሩን በመጠቀም ይፈርማል
     የኤንቨሎፕ መሳሪያ (`sorafs_manifest::governance` ሞጁል)።
   - ኤንቨሎፕ እስከ `governance/providers/<provider_id>/admission.json` ድረስ ይቆያል።
3. **የመዝገብ ቤት መግባት**
   - የጋራ አረጋጋጭ ይተግብሩ (`sorafs_manifest::provider_admission::validate_envelope`)
     ያ Torii / ጌትዌይስ / CLI እንደገና ጥቅም ላይ ይውላል.
   - የመግቢያ መንገዱን ያዘምኑ Torii ማስታዎቂያዎችን ላለመቀበል የምግብ መፈጨት ወይም የአገልግሎት ጊዜው ከፖስታው የሚለይ።
4. ** መታደስ እና መሻር**
   - `ProviderAdmissionRenewalV1` ከአማራጭ የመጨረሻ ነጥብ/የካስማ ዝማኔዎች ጋር ይጨምሩ።
   - የተሻረበትን ምክንያት የሚመዘግብ እና የአስተዳደር ክስተትን የሚገፋውን የI18NI0000044X CLI ዱካ ያጋልጡ።

## የትግበራ ተግባራት

| አካባቢ | ተግባር | ባለቤት(ዎች) | ሁኔታ |
|-------------|-------|----|
| እቅድ | `ProviderAdmissionProposalV1`፣ `ProviderAdmissionEnvelopeV1`፣ `EndpointAttestationV1` (Norito) በI18NI0000048X ይግለጹ። በI18NI0000049X ከማረጋገጫ ረዳቶች ጋር ተተግብሯል።【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | ማከማቻ / አስተዳደር | ✅ ተጠናቀቀ |
| CLI መሳሪያ | `sorafs_manifest_stub`ን በንዑስ ትዕዛዞች ያራዝሙ፡ `provider-admission proposal`፣ `provider-admission sign`፣ `provider-admission verify`። | Tooling WG | ✅ |

የCLI ፍሰት አሁን መካከለኛ የምስክር ወረቀት ቅርቅቦችን (`--endpoint-attestation-intermediate`) ይቀበላል።
ቀኖናዊ ፕሮፖዛል/የኤንቨሎፕ ባይት፣ እና የምክር ቤት ፊርማዎችን በ`sign`/`verify` ጊዜ ያረጋግጣል። ኦፕሬተሮች ይችላሉ።
የማስታወቂያ አካላትን በቀጥታ ያቅርቡ ወይም የተፈረሙ ማስታወቂያዎችን እንደገና ይጠቀሙ እና የፊርማ ፋይሎች በማጣመር ሊቀርቡ ይችላሉ
`--council-signature-public-key` ከ `--council-signature-file` ጋር ለአውቶሜሽን ወዳጃዊነት።

### የ CLI ማጣቀሻ

እያንዳንዱን ትዕዛዝ በ I18NI0000059X በኩል ያሂዱ።

- `proposal`
  - አስፈላጊ ባንዲራዎች፡ I18NI0000061X፣ `--chunker-profile=<namespace.name@semver>`፣
    `--stake-pool-id=<hex32>`፣ `--stake-amount=<amount>`፣ `--advert-key=<hex32>`፣
    `--jurisdiction-code=<ISO3166-1>`፣ እና ቢያንስ አንድ I18NI0000067X።
  - በየመጨረሻ ነጥብ ማረጋገጫ `--endpoint-attestation-attested-at=<secs>` ይጠብቃል፣
    `--endpoint-attestation-expires-at=<secs>`፣ በ በኩል የምስክር ወረቀት
    `--endpoint-attestation-leaf=<path>` (ከአማራጭ `--endpoint-attestation-intermediate=<path>` በተጨማሪ
    ለእያንዳንዱ ሰንሰለት አባል) እና ማንኛውም የተደራደሩ ALPN መታወቂያዎች
    (`--endpoint-attestation-alpn=<token>`)። የQUIC የመጨረሻ ነጥቦች የትራንስፖርት ሪፖርቶችን ሊያቀርቡ ይችላሉ።
    `--endpoint-attestation-report[-hex]=…`.
  - ውፅዓት፡ ቀኖናዊ Norito ፕሮፖዛል ባይት (`--proposal-out`) እና የJSON ማጠቃለያ
    (ነባሪ stdout ወይም `--json-out`)።
- `sign`
  - ግብዓቶች፡ ፕሮፖዛል (`--proposal`)፣ የተፈረመ ማስታወቂያ (`--advert`)፣ አማራጭ የማስታወቂያ አካል
    (`--advert-body`)፣ የማቆየት ዘመን፣ እና ቢያንስ አንድ የምክር ቤት ፊርማ። ፊርማዎችን ማቅረብ ይቻላል
    inline (`--council-signature=<signer_hex:signature_hex>`) ወይም በፋይሎች በማጣመር
    `--council-signature-public-key` ከ `--council-signature-file=<path>` ጋር።
  - የተረጋገጠ ኤንቨሎፕ (`--envelope-out`) እና የJSON ዘገባ የምግብ መፈጨት ትስስርን ያሳያል።
    የፈራሚ ብዛት፣ እና የግቤት መንገዶች።
- `verify`
  - ያለውን ኤንቨሎፕ (`--envelope`) ያረጋግጣል፣ እንደ አማራጭ የሚዛመደውን ፕሮፖዛል በማጣራት፣
    አካልን ማስተዋወቅ ወይም ማስተዋወቅ። የJSON ሪፖርቱ የመፍጨት እሴቶችን፣ የፊርማ ማረጋገጫ ሁኔታን፣
    እና ከየትኞቹ አማራጭ ቅርሶች ጋር ይጣጣማሉ።
- `renewal`
  - አዲስ የጸደቀ ኤንቨሎፕ ከዚህ ቀደም ከተረጋገጠው የምግብ መፍጨት ጋር ያገናኛል። ይፈልጋል
    `--previous-envelope=<path>` እና ተተኪው `--envelope=<path>` (ሁለቱም Norito ጭነት)።
    CLI የመገለጫ ተለዋጭ ስሞች፣ ችሎታዎች እና የማስታወቂያ ቁልፎች ሳይለወጡ እንደሚቆዩ ያረጋግጣል
    የአክሲዮን፣ የመጨረሻ ነጥቦችን እና የሜታዳታ ዝመናዎችን መፍቀድ። ቀኖናዊውን ያወጣል።
    `ProviderAdmissionRenewalV1` ባይት (I18NI0000090X) እና የJSON ማጠቃለያ።
- `revoke`
  - ፖስታው ላለበት አገልግሎት አቅራቢ የድንገተኛ I18NI0000092X ጥቅል ያወጣል።
    መወገድ አለባቸው ። `--envelope=<path>`፣ `--reason=<text>`፣ ቢያንስ አንድ ያስፈልገዋል
    `--council-signature`፣ እና አማራጭ I18NI0000096X/I18NI0000097X። CLI ይፈርማል እና ያረጋግጣል
    የስረዛ መፍጨት፣ የNorito ክፍያን በI18NI0000098X ይጽፋል እና የJSON ሪፖርት ያትማል።
    የምግብ መፍጫውን እና የፊርማውን ብዛት በመያዝ.
| ማረጋገጫ | በTorii፣ ጌትዌይስ እና `sorafs-node` ጥቅም ላይ የዋለውን የጋራ አረጋጋጭ ተግብር። ክፍል + CLI ውህደት ሙከራዎችን ያቅርቡ።【F: crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | አውታረ መረብ TL / ማከማቻ | ✅ ተጠናቀቀ |
| Torii ውህደት | የክር አረጋጋጭ ወደ I18NT0000017X ማስታወቂያ ማስገባት፣ ከፖሊሲ ውጪ ማስታወቂያዎችን አለመቀበል፣ ቴሌሜትሪ ልቀት። | አውታረ መረብ TL | ✅ ተጠናቀቀ | Torii አሁን የአስተዳደር ኤንቨሎፖችን (`torii.sorafs.admission_envelopes_dir`) ይጭናል፣ ወደ ውስጥ በሚገቡበት ጊዜ የምግብ መፈጨት/ፊርማ ግጥሚያዎችን ያረጋግጣል፣ እና የገጽታ መግቢያ telemetry.【F: crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| መታደስ | የእድሳት/የመሻሪያ እቅድ + CLI ረዳቶችን ያክሉ፣ የህይወት ኡደት መመሪያን በሰነዶች ውስጥ ያትሙ (ከዚህ በታች ያለውን runbook ይመልከቱ እና የ CLI ትዕዛዞች በ ውስጥ `provider-admission renewal`/`revoke`)【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【ዶክመንቶች/ምንጭ/sorafs/አቅራቢ_መመሪያ፡120】 ማከማቻ / አስተዳደር | ✅ ተጠናቀቀ |
| ቴሌሜትሪ | `provider_admission` ዳሽቦርዶችን እና ማንቂያዎችን ይግለጹ (የጠፋ እድሳት፣ የኤንቨሎፕ ጊዜ ማብቂያ)። | ታዛቢነት | 🟠 በሂደት ላይ | ቆጣሪ `torii_sorafs_admission_total{result,reason}` አለ; ዳሽቦርዶች/ማንቂያዎች በመጠባበቅ ላይ።【F: crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |
### እድሳት እና መሻር Runbook

#### የታቀደ እድሳት (የካስማ/የቶፖሎጂ ዝመናዎች)
1. የተተኪውን ፕሮፖዛል/ማስታወቂያ ጥንድ ከ`provider-admission proposal` እና `provider-admission sign` ጋር ይገንቡ፣ `--retention-epoch` በመጨመር እና እንደአስፈላጊነቱ የአክሲዮን/የመጨረሻ ነጥቦችን ያዘምኑ።
2. መፈጸም  
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   ትዕዛዙ ያልተለወጠ የችሎታ/የመገለጫ መስኮችን ያረጋግጣል
   `AdmissionRecord::apply_renewal`፣ `ProviderAdmissionRenewalV1` ያወጣል፣ እና የምግብ መፈጨትን ያትማል ለ
   የአስተዳደር መዝገብ
3. የቀደመውን ፖስታ በ`torii.sorafs.admission_envelopes_dir` ይቀይሩት ፣እድሳቱን Norito/JSON በአስተዳደር ማከማቻው ላይ ያድርጉ እና የእድሳት ሀሽ + ማቆየት ዘመንን ከI18NI000001111X ጋር ጨምሩ።
4. አዲሱ ፖስታ የቀጥታ ስርጭት መሆኑን ለኦፕሬተሮች ያሳውቁ እና `torii_sorafs_admission_total{result="accepted",reason="stored"}` መያዙን ያረጋግጡ።
5. በ `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` በኩል የቀኖና ዕቃዎችን እንደገና ማደስ እና ማከናወን; CI (`ci/check_sorafs_fixtures.sh`) የNorito ውፅዓቶች ተረጋግተው እንዲቆዩ ያረጋግጣል።

#### የአደጋ መሻር
1. የተጠለፈውን ኤንቨሎፕ ይለዩ እና መሻሪያውን ይስጡ፡-
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   CLI `ProviderAdmissionRevocationV1` ይፈርማል፣ የተቀመጠውን ፊርማ በ በኩል ያረጋግጣል
   `verify_revocation_signatures`፣ እና የስረዛ መፍቻውን ሪፖርት ያደርጋል።【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/አቅራቢ_አድሚሽን.rs#486
2. ፖስታውን ከ`torii.sorafs.admission_envelopes_dir` ያስወግዱ፣ መሻሪያውን Norito/JSON ወደ መግቢያ መሸጎጫዎች ያሰራጩ እና ምክንያቱን ሃሽ በአስተዳደር ደቂቃዎች ውስጥ ይመዝግቡ።
3. መሸጎጫዎች የተሻረውን ማስታወቂያ መጣሉን ለማረጋገጥ `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` ይመልከቱ። የተሰረዙ ቅርሶችን ወደ ኋላ መለስ ብለው ያቆዩት።

## ሙከራ እና ቴሌሜትሪ- ለመግቢያ ፕሮፖዛል እና ኤንቨሎፕ ለታች ወርቃማ ዕቃዎችን ይጨምሩ
  `fixtures/sorafs_manifest/provider_admission/`.
- ፕሮፖዛሎችን ለማደስ እና ፖስታዎችን ለማረጋገጥ CI (`ci/check_sorafs_fixtures.sh`) ያራዝሙ።
- የመነጩ መጫዎቻዎች `metadata.json` ከቀኖናዊ የምግብ መፍጫዎች ጋር; የታችኛው ተፋሰስ ሙከራዎች ያረጋግጣሉ
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- የውህደት ሙከራዎችን ያቅርቡ;
  - Torii የጎደሉ ወይም ጊዜው ያለፈባቸው የመግቢያ ኤንቨሎፖች ማስታወቂያዎችን ውድቅ ያደርጋል።
  - CLI የዙር ጉዞዎች ፕሮፖዛል → ኤንቨሎፕ → ማረጋገጫ።
  - የአስተዳደር መታደስ የአቅራቢ መታወቂያ ሳይለውጥ የመጨረሻ ነጥብ ማረጋገጫን ይሽከረከራል።
- የቴሌሜትሪ መስፈርቶች;
  - ኢምት `provider_admission_envelope_{accepted,rejected}` ቆጣሪዎች በ I18NT0000020X። ✅ `torii_sorafs_admission_total{result,reason}` አሁን ወደላይ ቀርቧል/የተቀበሉት ውጤቶች።
  - የማለፊያ ማስጠንቀቂያዎችን ወደ ታዛቢነት ዳሽቦርዶች ያክሉ (በ7 ቀናት ውስጥ መታደስ ያስፈልጋል)።

## ቀጣይ እርምጃዎች

1. ✅ የNorito የመርሃግብር ለውጦችን አጠናቅቋል እና የማረጋገጫ አጋዥዎችን በ
   `sorafs_manifest::provider_admission`. ምንም የባህሪ ባንዲራዎች አያስፈልግም።
2. ✅ CLI የስራ ፍሰቶች (`proposal`, `sign`, `verify`, `renewal`, `revoke`) በመዋሃድ ፈተናዎች አማካይነት ተመዝግበው ይሠራሉ; የአስተዳደር ስክሪፕቶችን ከ runbook ጋር ማመሳሰል።
3. ✅ Torii መግቢያ/ግኝት ፖስታዎቹን አስገብቶ የቴሌሜትሪ ቆጣሪዎችን ለመቀበል/ለመቀበል ያጋልጣል።
4. በታዛቢነት ላይ ያተኩሩ፡ የመግቢያ ዳሽቦርዶችን/ማስጠንቀቂያዎችን ይጨርሱ ስለዚህ በሰባት ቀናት ውስጥ የሚደረጉ እድሳት ማስጠንቀቂያዎችን ያሳድጋል (`torii_sorafs_admission_total`፣ ጊዜው ያለፈበት መለኪያዎች)።