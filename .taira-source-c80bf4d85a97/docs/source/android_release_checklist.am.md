---
lang: am
direction: ltr
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-05T09:28:11.999717+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# አንድሮይድ መልቀቂያ ማረጋገጫ ዝርዝር (AND6)

ይህ የማረጋገጫ ዝርዝር የ **AND6 — CI & Compliance Hardening** በሮችን ይይዛል
`roadmap.md` (§ቅድሚያ 5)። የአንድሮይድ ኤስዲኬ ልቀቶችን ከ Rust ጋር ያስተካክላል
የ CI ስራዎችን ፣ የተሟሉ ቅርሶችን በመፃፍ የ RFC ፍላጎቶችን መልቀቅ ፣
ከጂኤ በፊት መያያዝ ያለባቸው የመሣሪያ-ላብራቶሪ ማስረጃ እና የፕሮቬንሽን ጥቅሎች፣
LTS፣ ወይም hotfix ባቡር ወደ ፊት ይሄዳል።

ይህንን ሰነድ አብረው ይጠቀሙ፡-

- `docs/source/android_support_playbook.md` - የመልቀቂያ ቀን መቁጠሪያ ፣ SLAs እና
  የማሳደግ ዛፍ.
- `docs/source/android_runbook.md` - ቀን-ወደ-ቀን የሚሠሩ የሩጫ መጽሐፍት።
- `docs/source/compliance/android/and6_compliance_checklist.md` - ተቆጣጣሪ
  artefacts inventory.
- `docs/source/release_dual_track_runbook.md` - ባለሁለት ትራክ ልቀት አስተዳደር።

## 1. የመድረክ በሮች በጨረፍታ

| መድረክ | አስፈላጊ በሮች | ማስረጃ |
|-------------|------|
| **T−7 ቀናቶች (ቅድመ-ቀዝቃዛ)** | በምሽት `ci/run_android_tests.sh` አረንጓዴ ለ 14 ቀናት; `ci/check_android_fixtures.sh`፣ `ci/check_android_samples.sh`፣ እና `ci/check_android_docs_i18n.sh` ማለፍ; lint / ጥገኝነት ስካን ወረፋ. | የBuildkite ዳሽቦርዶች፣ የቋሚ ልዩነት ዘገባ፣ የናሙና ቅጽበታዊ ገጽ እይታዎች። |
| **T−3 ቀን (አርሲ ማስተዋወቅ)** | የመሣሪያ-ላብራቶሪ ቦታ ማስያዝ ተረጋግጧል; StrongBox ማረጋገጫ CI ሩጫ (`scripts/android_strongbox_attestation_ci.sh`); በተያዘለት ሃርድዌር ላይ የሚለማመዱ የሮቦሌክትሪክ/የመሳሪያ ስብስቦች; `./gradlew lintRelease ktlintCheck detekt dependencyGuard` ንጹህ። | የመሣሪያ ማትሪክስ CSV፣ የማረጋገጫ ቅርቅብ አንጸባራቂ፣ Gradle ሪፖርቶች በ`artifacts/android/lint/<version>/` ስር ተቀምጠዋል። |
| **T−1 ቀን (ሂድ/አለመሄድ)** | የቴሌሜትሪ ማሻሻያ ሁኔታ ቅርቅብ ታድሷል (`scripts/telemetry/check_redaction_status.py --write-cache`); በ `and6_compliance_checklist.md` የተሻሻሉ ተገዢነት ቅርሶች; የፕሮቬንሽን ልምምድ ተጠናቀቀ (`scripts/android_sbom_provenance.sh --dry-run`)። | `docs/source/compliance/android/evidence_log.csv`፣ የቴሌሜትሪ ሁኔታ JSON፣ የፕሮቬንሽን ደረቅ አሂድ ሎግ |
| ** T0 (GA/LTS መቁረጫ)** | `scripts/publish_android_sdk.sh --dry-run` ተጠናቅቋል; provenance + SBOM የተፈረመ; የመልቀቂያ ማረጋገጫ ዝርዝር ወደ ውጭ የተላከ እና ወደ go/no-ሂድ ደቂቃዎች ተያይዟል; `ci/sdk_sorafs_orchestrator.sh` የጭስ ሥራ አረንጓዴ. | የRFC አባሪዎችን፣ Sigstore ጥቅልን፣ የጉዲፈቻ ቅርሶችን በ`artifacts/android/` ይልቀቁ። |
| **T+1 ቀን (ድህረ-መቁረጥ)** | Hotfix ዝግጁነት የተረጋገጠ (`scripts/publish_android_sdk.sh --validate-bundle`); ዳሽቦርድ ልዩነቶች ተገምግመዋል (`ci/check_android_dashboard_parity.sh`); የማስረጃ ፓኬት ወደ `status.md` ተሰቅሏል። | ዳሽቦርድ ልዩነት ወደ ውጪ መላክ፣ ወደ `status.md` ግቤት አገናኝ፣ በማህደር የተቀመጠ የመልቀቂያ ጥቅል። |

## 2. CI & Quality Gate Matrix| በር | ትዕዛዝ(ዎች) / ስክሪፕት | ማስታወሻ |
|-------|------------|---|
| ክፍል + ውህደት ሙከራዎች | `ci/run_android_tests.sh` (ጥቅል `ci/run_android_tests.sh`) | Emits `artifacts/android/tests/test-summary.json` + የሙከራ ምዝግብ ማስታወሻ። Norito ኮዴክ፣ ወረፋ፣ StrongBox fallback እና Torii የደንበኛ ማሰሪያ ሙከራዎችን ያካትታል። በምሽት እና ከመለያው በፊት ያስፈልጋል። |
| ቋሚ እኩልነት | `ci/check_android_fixtures.sh` (ጥቅል `scripts/check_android_fixtures.py`) | የታደሰ Norito መጫዎቻዎች ከሩስት ቀኖናዊ ስብስብ ጋር እንደሚዛመዱ ያረጋግጣል። በሩ ሳይሳካ ሲቀር የJSON ልዩነትን ያያይዙ። |
| የናሙና መተግበሪያዎች | `ci/check_android_samples.sh` | `examples/android/{operator-console,retail-wallet}` ይገነባል እና በ`scripts/android_sample_localization.py` በኩል የተተረጎሙ ቅጽበታዊ ገጽ እይታዎችን ያረጋግጣል። |
| ሰነዶች/I18N | `ci/check_android_docs_i18n.sh` | ጠባቂዎች README + የተተረጎሙ ፈጣን ጅምሮች። በመልቀቂያ ቅርንጫፍ ውስጥ ከዶክ አርትዖቶች በኋላ እንደገና ያሂዱ። |
| ዳሽቦርድ እኩልነት | `ci/check_android_dashboard_parity.sh` | CI/ ወደ ውጭ የተላኩ መለኪያዎች ከ Rust ባልደረባዎች ጋር መስማማታቸውን ያረጋግጣል። በT+1 ማረጋገጫ ጊዜ ያስፈልጋል። |
| SDK የማደጎ ጭስ | `ci/sdk_sorafs_orchestrator.sh` | የባለብዙ ምንጭ የሶራፍስ ኦርኬስትራ ማሰሪያዎችን አሁን ካለው ኤስዲኬ ጋር ይለማመዳል። የተደረደሩ ቅርሶችን ከመጫንዎ በፊት ያስፈልጋል። |
| የማረጋገጫ ማረጋገጫ | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | የ StrongBox/TEE ማረጋገጫ ቅርቅቦችን በ`artifacts/android/attestation/**` ስር ይሰበስባል; ማጠቃለያውን ከ GA ፓኬቶች ጋር ያያይዙት። |
| የመሣሪያ-ላብራቶሪ ማስገቢያ ማረጋገጫ | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` | እሽጎችን ለመልቀቅ ማስረጃን ከማያያዝዎ በፊት የመሳሪያ ጥቅሎችን ያረጋግጣል; CI በ `fixtures/android/device_lab/slot-sample` (ቴሌሜትሪ/ማስረጃ/ወረፋ/ምዝግብ ማስታወሻዎች + `sha256sum.txt`) ካለው የናሙና ማስገቢያ ጋር ይሮጣል። |

> ** ጠቃሚ ምክር:** እነዚህን ስራዎች ወደ `android-release` Buildkite ቧንቧ በማከል
> የቀዘቀዙ ሳምንታት በተለቀቀው የቅርንጫፍ ጫፍ እያንዳንዱን በር በራስ ሰር እንደገና ያሂዱ።

የተጠናከረው `.github/workflows/android-and6.yml` ስራው ሊንትን ያካሂዳል፣
የሙከራ-ስብስብ፣ የማረጋገጫ-ማጠቃለያ እና የመሣሪያ-ላብ ማስገቢያ ፍተሻዎች በእያንዳንዱ PR/ግፊት
አንድሮይድ ምንጮችን በመንካት፣በ`artifacts/android/{lint,tests,attestation,device_lab}/` ስር ማስረጃ በመስቀል ላይ።

## 3. የሊንት እና ጥገኝነት ቅኝቶች

`scripts/android_lint_checks.sh --version <semver>` ከ repo root ያሂዱ። የ
ስክሪፕት ያስፈጽማል፡-

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- ሪፖርቶች እና የጥገኛ-ጠባቂ ውጽዓቶች ስር በማህደር ተቀምጠዋል
  `artifacts/android/lint/<label>/` እና `latest/` ሲምሊንክ ለመልቀቅ
  የቧንቧ መስመሮች.
- ያልተሳኩ ግኝቶች ማረም ወይም በተለቀቀው ውስጥ መግባትን ይፈልጋሉ
  RFC ተቀባይነት ያለውን አደጋ በመመዝገብ (በተለቀቀው ምህንድስና + ፕሮግራም የጸደቀ
  መሪ)።
- `dependencyGuardBaseline` የጥገኛ መቆለፊያን ያድሳል; ልዩነቱን ያያይዙ
  ወደ መሄድ/አለመሄድ ፓኬት።

## 4. የመሣሪያ ቤተ ሙከራ እና የስትሮንግቦክስ ሽፋን

1. በ ውስጥ የተጠቀሰውን የአቅም መከታተያ በመጠቀም የመጠባበቂያ ፒክስል + ጋላክሲ መሳሪያዎች
   `docs/source/compliance/android/device_lab_contingency.md`. ልቀቶችን ያግዳል።
   ከ` የማረጋገጫ ዘገባውን ለማደስ።
3. የመሳሪያውን ማትሪክስ ያሂዱ (በመሳሪያው ውስጥ ያለውን የሱይት / ABI ዝርዝርን ይመዝግቡ
   መከታተያ)። ድጋሚ ሙከራዎች ቢሳካላቸውም በክስተቱ መዝገብ ውስጥ አለመሳካቶችን ያንሱ።
4. ወደ ፋየርቤዝ የሙከራ ላብራቶሪ መመለስ የሚያስፈልግ ከሆነ ትኬት ያስመዝግቡ። ቲኬቱን ያገናኙ
   ከታች ባለው የማረጋገጫ ዝርዝር ውስጥ.

## 5. Compliance & Telemetry Artefacts- ለአውሮፓ ህብረት `docs/source/compliance/android/and6_compliance_checklist.md` ተከተል
  እና JP ማቅረቢያዎች. `docs/source/compliance/android/evidence_log.csv` ያዘምኑ
  በ hashes + Buildkite የስራ ዩአርኤሎች።
- የቴሌሜትሪ ማሻሻያ ማስረጃን በ በኩል ያድሱ
  `ስክሪፕቶች/ቴሌሜትሪ/የቼክ_redaction_status.py --write-cache
   --status-url https://android-observability.example/status.json`።
  የተገኘውን JSON ስር ያከማቹ
  `artifacts/android/telemetry/<version>/status.json`.
- የመርሃግብር ልዩነት ውጤቱን ይመዝግቡ
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  ከዝገት ላኪዎች ጋር እኩልነት ለማረጋገጥ።

## 6. ፕሮቨንሽን፣ SBOM እና ህትመት

1. የህትመት ቧንቧ መስመርን ማድረቅ;

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. SBOM + Sigstore provenance ፍጠር፡

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. `artifacts/android/provenance/<semver>/manifest.json` ያያይዙ እና ተፈርመዋል
   `checksums.sha256` ወደ ተለቀቀው RFC.
4. ወደ እውነተኛው Maven ማከማቻ ሲያስተዋውቁ፣ እንደገና ያሂዱ
   `scripts/publish_android_sdk.sh` ያለ `--dry-run`፣ ኮንሶሉን ያንሱ
   ይመዝገቡ እና የተገኙትን ቅርሶች ወደ `artifacts/android/maven/<semver>` ይስቀሉ።

## 7. የማስረከቢያ ፓኬት አብነት

እያንዳንዱ የGA/LTS/hotfix ልቀት የሚከተሉትን ማካተት አለበት፦

1. **የተጠናቀቀ የፍተሻ ዝርዝር** — የዚህን ፋይል ሰንጠረዥ ይቅዱ፣ እያንዳንዱን ንጥል ነገር ምልክት ያድርጉ እና አገናኝ
   ቅርሶችን ለመደገፍ (Buildkite run, logs, doc diffs)።
2. **የመሣሪያ ላብራቶሪ ማስረጃ** - የማረጋገጫ ዘገባ ማጠቃለያ፣ የቦታ ማስያዣ ምዝግብ ማስታወሻ እና
   ማንኛውም የአደጋ ጊዜ እንቅስቃሴዎች.
3. **የቴሌሜትሪ ፓኬት** — የማሻሻያ ሁኔታ JSON፣ schema diff፣ link to
   `docs/source/sdk/android/telemetry_redaction.md` ዝመናዎች (ካለ)።
4. **የማስተካከያ ቅርሶች** — በማስታወሻ ማህደር ውስጥ የተጨመሩ/የተዘመኑ ግቤቶች
   በተጨማሪም የታደሰው የማስረጃ መዝገብ CSV።
5. ** የፕሮቨንስ ጥቅል *** - SBOM፣ Sigstore ፊርማ እና `checksums.sha256`።
6. **የልቀት ማጠቃለያ** — አንድ-ገጽ አጠቃላይ እይታ ከ`status.md` ማጠቃለያ ጋር ተያይዟል።
   ከላይ ያለው (ቀን, ስሪት, ማንኛውም የተወገዱ በሮች ድምቀት).

ፓኬጁን በ `artifacts/android/releases/<version>/` ስር ያከማቹ እና ያጣቅሱት።
በ `status.md` እና በተለቀቀው RFC.

- `scripts/run_release_pipeline.py --publish-android-sdk ...` በራስ ሰር
  የቅርብ ጊዜውን የሊንት መዝገብ (`artifacts/android/lint/latest`) እና የ
  የታዛዥነት ማስረጃ ወደ `artifacts/android/releases/<version>/` ይግቡ
  የማስረከቢያ ፓኬት ሁል ጊዜ ቀኖናዊ ቦታ አለው።

---

** አስታዋሽ፡** አዲስ የCI ስራዎች፣ ተገዢ የሆኑ ቅርሶች፣
ወይም የቴሌሜትሪ መስፈርቶች ተጨምረዋል. የመንገድ ካርታ ንጥል AND6 እስከ እ.ኤ.አ. ድረስ ክፍት እንደሆነ ይቆያል
የማረጋገጫ ዝርዝር እና ተያያዥነት ያለው አውቶማቲክ ለሁለት ተከታታይ ልቀት የተረጋጋ መሆኑን ያረጋግጣል
ባቡሮች.