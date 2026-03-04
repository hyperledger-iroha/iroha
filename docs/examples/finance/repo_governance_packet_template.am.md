---
lang: am
direction: ltr
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd018a94197722adfbb9d54bf02f1c486147078174ba4c81f32e9d93b8c3f6d5
source_last_modified: "2026-01-22T16:26:46.473419+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Repo Governance Packet Template (Roadmap F1)

በፍኖተ ካርታ ንጥል የሚፈለገውን የቅርስ ቅርቅብ ሲያዘጋጁ ይህን አብነት ይጠቀሙ
F1 (የሪፖ የህይወት ዑደት ሰነዶች እና መሳሪያዎች)። ግቡ ገምጋሚዎችን መስጠት ነው ሀ
እያንዳንዱን ግብዓት፣ ሃሽ እና የማስረጃ ጥቅል የሚዘረዝር ነጠላ ማርክዳውድ ፋይል
የአስተዳደር ምክር ቤት በአስተያየቱ ውስጥ የተጠቀሱትን ባይት እንደገና ማጫወት ይችላል።

> አብነቱን ወደ እራስዎ የማስረጃ ማውጫ ይቅዱ (ለምሳሌ
> `artifacts/finance/repo/2026-03-15/packet.md`)፣ ቦታ ያዥዎቹን ይተኩ፣ እና
> ከዚህ በታች ከተጠቀሱት ሃሽድ ቅርሶች ቀጥሎ ግባ/ስቀል።

## 1. ሜታዳታ

| መስክ | ዋጋ |
|-------|------|
| ስምምነት/የለውጥ መለያ | `<repo-yyMMdd-XX>` |
| የተዘጋጀው በ / ቀን | `<desk lead> – 2026-03-15T10:00Z` |
| የተገመገመ በ | `<dual-control reviewer(s)>` |
| አይነት ለውጥ | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
| ጠባቂ(ዎች) | `<custodian id(s)>` |
| የተያያዘ ፕሮፖዛል / ሪፈረንደም | `<governance ticket id or GAR link>` |
| የማስረጃ ማውጫ | ``artifacts/finance/repo/<slug>/`` |

## 2. የመመሪያ ክፍያ

ዴስኮች በ በኩል የጀመሩትን በደረጃ I18NT0000001X መመሪያዎችን ይመዝግቡ
`iroha app repo ... --output`. እያንዳንዱ ግቤት የተለቀቀውን ሃሽ ማካተት አለበት።
ፋይል እና ድምጽ አንድ ጊዜ የሚቀርበው ድርጊት አጭር መግለጫ
ያልፋል።

| ድርጊት | ፋይል | SHA-256 | ማስታወሻ |
|--------|-------|-----|------|
| አስጀምር | `instructions/initiate.json` | `<sha256>` | በጠረጴዛ + ተጓዳኝ የጸደቁትን ጥሬ ገንዘብ/መያዣ እግሮችን ይዟል። |
| የኅዳግ ጥሪ | `instructions/margin_call.json` | `<sha256>` | ጥሪውን የቀሰቀሰውን cadence + የአሳታፊ መታወቂያን ይይዛል። |
| ፈታ | `instructions/unwind.json` | `<sha256>` | ሁኔታዎች ከተሟሉ በኋላ የኋላ-እግር ማረጋገጫ። |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json \
  | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 ሞግዚት ምስጋናዎች (ባለሶስት ፓርቲ ብቻ)

repo `--custodian` ሲጠቀም ይህንን ክፍል ያጠናቅቁ። የአስተዳደር ፓኬት
ከእያንዳንዱ ሞግዚት የተፈረመ እውቅና እና ሃሽ ማካተት አለበት።
በ`docs/source/finance/repo_ops.md` §2.8 የተጠቀሰው ፋይል።

| ጠባቂ | ፋይል | SHA-256 | ማስታወሻ |
|--------|-------|-----|-------|
| `<ih58...>` | `custodian_ack_<custodian>.md` | `<sha256>` | የተፈረመ SLA የጥበቃ መስኮት፣ የማዞሪያ መለያ እና የመሰርሰሪያ ግንኙነት። |

> እውቅናውን ከሌላው ማስረጃ (`artifacts/finance/repo/<slug>/`) አጠገብ ያከማቹ
> ስለዚህ `scripts/repo_evidence_manifest.py` ፋይሉን በተመሳሳይ ዛፍ ውስጥ ይመዘግባል
> የታቀዱት መመሪያዎች እና ቅንጥቦችን ያዋቅሩ። ተመልከት
> `docs/examples/finance/repo_custodian_ack_template.md` ለመሙላት ዝግጁ
> ከአስተዳደር ማስረጃ ውል ጋር የሚዛመድ አብነት።

## 3. የማዋቀር ቅንጣቢ

በክላስተር ላይ የሚያርፍ የ`[settlement.repo]` TOML ብሎክ ይለጥፉ (ጨምሮም)
`collateral_substitution_matrix`). ከቅንጣው ቀጥሎ ያለውን ሃሽ ያከማቹ
ኦዲተሮች ሪፖ ቦታ ሲያዙ ንቁ የነበረውን የሩጫ ጊዜ ፖሊሲን ማረጋገጥ ይችላሉ።
ተቀባይነት አግኝቷል።

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 የድህረ ማጽደቅ ውቅረት ቅጽበተ-ፎቶዎች

የሪፈረንደም ወይም የአስተዳደር ድምጽ ከተጠናቀቀ እና `[settlement.repo]`
ለውጥ ተዘርግቷል፣የ `/v1/configuration` ቅጽበተ-ፎቶዎችን ከእያንዳንዱ እኩያ ያንሱ
ኦዲተሮች የጸደቀው ፖሊሲ በክላስተር ውስጥ የቀጥታ መሆኑን ማረጋገጥ ይችላሉ (ተመልከት
`docs/source/finance/repo_ops.md` §2.9 ለማስረጃው የስራ ሂደት)።

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v1/configuration \
  | jq '.' \
  > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

| እኩያ / ምንጭ | ፋይል | SHA-256 | አግድ ቁመት | ማስታወሻ |
|--------|------|-----|-------------|---|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` | የቅጽበታዊ ገጽ እይታ ውቅር ከተለቀቀ በኋላ ወዲያውኑ ተይዟል። |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` | `[settlement.repo]` ከታቀደው TOML ጋር እንደሚመሳሰል ያረጋግጣል። |

በ `hashes.txt` (ወይንም ተመጣጣኝውን) ከእኩያ መታወቂያዎች ጋር መቀላቀልን ይመዝግቡ
ማጠቃለያ) ስለዚህ ገምጋሚዎች ለውጡን ወደ ውስጥ የገቡት የትኞቹ አንጓዎች መፈለግ ይችላሉ። ቅጽበተ-ፎቶዎቹ
ከTOML ቅንጣቢ ቀጥሎ በ`config/peers/` ስር ይኖራሉ እና ይወሰዳል።
በራስ-ሰር በ `scripts/repo_evidence_manifest.py`።

## 4. ቆራጥ የፍተሻ ቅርሶች

የቅርብ ጊዜ ውጤቶችን ከ፡ ያያይዙ፡

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

የፋይል ዱካዎችን + hashes ይቅረጹ ለሎግ ቅርቅቦች ወይም JUnit XML በእርስዎ CI የተሰራ
ስርዓት.

| Artefact | ፋይል | SHA-256 | ማስታወሻ |
|---------|-----|-------|
| የህይወት ዑደት ማረጋገጫ መዝገብ | `tests/repo_lifecycle.log` | `<sha256>` | በI18NI0000055X ውፅዓት ተይዟል። |
| የውህደት ሙከራ መዝገብ | `tests/repo_integration.log` | `<sha256>` | መተኪያ + የኅዳግ ግልጽነት ሽፋንን ያካትታል። |

## 5. የህይወት ዑደት ማረጋገጫ ቅጽበታዊ እይታ

እያንዳንዱ ፓኬት ወደ ውጭ የተላከውን ወሳኝ የህይወት ዑደት ቅጽበታዊ ገጽ እይታ ማካተት አለበት።
`repo_deterministic_lifecycle_proof_matches_fixture`. ማሰሪያውን በ
ገምጋሚዎች የJSON ክፈፉን እንዲለያዩ እና እንዲቃወሙ ወደ ውጭ መላክ ቁልፎች ነቅተዋል።
በ `crates/iroha_core/tests/fixtures/` ውስጥ ተከታትሏል (ተመልከት
`docs/source/finance/repo_ops.md` §2.7)።

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json \
REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt \
cargo test -p iroha_core \
  -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

ወይም የተሰካውን ረዳት ተጠቀም መጫዎቻዎቹን ለማደስ እና ወደ እርስዎ ለመቅዳት
የማስረጃ ጥቅል በአንድ እርምጃ

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain> \
  --bundle-dir artifacts/finance/repo/<slug>
```

| Artefact | ፋይል | SHA-256 | ማስታወሻ |
|---------|-----|-------|
| ቅጽበታዊ ገጽ እይታ JSON | `repo_proof_snapshot.json` | `<sha256>` | በማረጋገጫ መታጠቂያው የተለቀቀው ቀኖናዊ የህይወት ዑደት ፍሬም። |
| ዳይጀስት ፋይል | `repo_proof_digest.txt` | `<sha256>` | አቢይ ሆክስ ዳይጀስት ከ `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest` ተንጸባርቋል; ሳይለወጥ እንኳ አያይዝ. |

## 6. የማስረጃ መግለጫ

ኦዲተሮች ማረጋገጥ እንዲችሉ ለጠቅላላው የማስረጃ መዝገብ ማኒፌክተሩን ያመንጩ
ማህደሩን ሳይከፍቱ hashes. ረዳቱ የተገለጸውን የስራ ሂደት ያንጸባርቃል
በ `docs/source/finance/repo_ops.md` §3.2.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/finance/repo/<slug> \
  --agreement-id <repo-identifier> \
  --output artifacts/finance/repo/<slug>/manifest.json
```

| Artefact | ፋይል | SHA-256 | ማስታወሻ |
|---------|-----|-------|
| ማስረጃ አንጸባራቂ | `manifest.json` | `<sha256>` | በአስተዳደር ትኬት/የህዝበ ውሳኔ ማስታወሻዎች ውስጥ ቼክሱን ያካትቱ። |

## 7. ቴሌሜትሪ እና የክስተት ቅጽበታዊ ገጽ እይታ

ተዛማጅነት ያላቸውን የ`AccountEvent::Repo(*)` ግቤቶችን እና ማንኛውንም ዳሽቦርድ ወይም CSV ወደ ውጪ ላክ
ወደ ውጪ መላክ በI18NI0000070X ተጠቅሷል። ፋይሎቹን ይቅረጹ +
እዚህ hashes ስለዚህ ገምጋሚዎች በቀጥታ ወደ ማስረጃው መዝለል ይችላሉ።

| ወደ ውጭ መላክ | ፋይል | SHA-256 | ማስታወሻ |
|--------|-------|-----|------|
| Repo ክስተቶች JSON | `evidence/repo_events.ndjson` | `<sha256>` | ጥሬ Torii ክስተት ዥረት ወደ ዴስክ መለያዎች ተጣርቶ። |
| ቴሌሜትሪ CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` | Repo Margin ፓነልን በመጠቀም ከI18NT0000000X ወደ ውጭ ተልኳል። |

## 8. ማጽደቂያዎች እና ፊርማዎች

- ** ባለሁለት መቆጣጠሪያ ፈራሚዎች: ** `<names + timestamps>`
- ** GAR / ደቂቃ መፍጨት: ** `<sha256>` የተፈረመ GAR ፒዲኤፍ ወይም ደቂቃዎች ሰቀላ.
- ** የማከማቻ ቦታ: ** `governance://finance/repo/<slug>/packet/`

## 9. የማረጋገጫ ዝርዝር

እያንዳንዱን ንጥል አንዴ እንደተጠናቀቀ ምልክት ያድርጉበት።

- [ ] የመመሪያ ሸክሞች በደረጃ፣ በሃሽድ እና በማያያዝ።
- [ ] የውቅረት ቅንጣቢ ሃሽ ተመዝግቧል።
- [ ] ቆራጥ የሙከራ ምዝግብ ማስታወሻዎች ተይዘዋል + hashed።
- [ ] የህይወት ዑደት ቅጽበታዊ + የምግብ መፍጨት ወደ ውጭ ተልኳል።
- [ ] የመነጨው ማስረጃ እና ሃሽ ተመዝግቧል።
- [ ] ክስተት/ቴሌሜትሪ ወደ ውጭ መላክ ተያዘ + hashed።
- [ ] ባለሁለት ቁጥጥር ምስጋናዎች በማህደር ተቀምጠዋል።
- [] GAR/ደቂቃዎች ተሰቅለዋል; ከላይ ተመዝግቧል መፈጨት.

ይህን አብነት ከእያንዳንዱ ፓኬት ጋር ማቆየት የDAG አስተዳደርን ይጠብቃል።
የሚወስን እና ለሪፖ የህይወት ዑደት ተንቀሳቃሽ አንጸባራቂ ኦዲተሮችን ይሰጣል
ውሳኔዎች.