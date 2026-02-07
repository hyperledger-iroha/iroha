---
lang: am
direction: ltr
source: docs/source/governance_vote_tally.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ebff8477d06e2aac8840988d31762704d05ded353d3f900a87db3ea5091e718
source_last_modified: "2026-01-04T08:19:26.508527+00:00"
translation_last_reviewed: 2026-02-07
title: Governance ZK Vote Tally
translator: machine-google-reviewed
---

## አጠቃላይ እይታ

የIroha የአስተዳደር አጠቃላይ ፍሰት በHalo2/IPA ወረዳ ላይ የሚመረኮዝ ሲሆን ይህም ትንሽ የድምፅ ቁርጠኝነት እና ብቁ በሆነው የመራጮች ስብስብ ውስጥ ያለውን አባልነት ያረጋግጣል። ይህ ማስታወሻ ገምጋሚዎች በፈተናዎች ውስጥ ጥቅም ላይ የዋለውን የማረጋገጫ ቁልፍ እና ማረጋገጫ ማደስ እንዲችሉ የወረዳ መለኪያዎችን፣ የህዝብ ግብአቶችን እና የኦዲት መሳሪያዎችን ይይዛል።

## የወረዳ ማጠቃለያ

- ** የወረዳ መለያ ***: `halo2/pasta/vote-bool-commit-merkle8-v1`
- ** ትግበራ ***: `VoteBoolCommitMerkle::<8>` በ `iroha_core::zk::depth`
- ** የጎራ መጠን ***: `k = 6`
- ** ጀርባ**፡ ግልፅ Halo2/IPA በፓስታ ላይ (ZK1 ፖስታ፡ `IPAK` + `H2VK` ለVKs፣ `PROF` + `I10P` ለማስረጃዎች)
- ** የምስክሮች ቅርጽ ***:
  - የድምጽ መስጫ ቢት `v ∈ {0,1}`
  - የዘፈቀደ scalar `ρ`
  - ለመርክል መንገድ ስምንት የወንድም እህት ስካላር
  - አቅጣጫ ቢት (በማጣቀሻ ምስክሮች ውስጥ ሁሉም ዜሮ)
- ** Merkle compressor ***: `H(x, y) = 2·(x + 7)^5 + 3·(y + 13)^5 (mod p)` የት `p` የፓስታ ስካላር ሞጁል ነው
- ** ይፋዊ ግብዓቶች ***
  - አምድ 0: `commit`
  - አምድ 1: Merkle root
  - በ`I10P` TLV (`cols = 2`፣ `rows = 1`) ተጋልጧል።

### የወረዳ አቀማመጥ

- ** የምክር አምዶች ***:
  - `v` - የድምጽ መስጫ ቢት ቡሊያን ለመሆን ተገድቧል።
  - `ρ` - በድምጽ ቁርጠኝነት ውስጥ ጥቅም ላይ የሚውል ዓይነ ስውር scalar።
  - `sibling[i]` ለ `i ∈ [0, 7]` – Merkle path element በ `i` ጥልቀት።
  - `dir[i]` ለ `i ∈ [0, 7]` - አቅጣጫ ቢት ወደ ግራ (`0`) ወይም ቀኝ (`1`) ቅርንጫፍ.
  - `node[i]` ለ `i ∈ [0, 7]` - Merkle accumulator ከጥልቅ በኋላ `i`.
- ** ምሳሌ ዓምዶች ***:
  - `commit` - የህዝብ ቁርጠኝነት በመራጭ የታተመ።
  - `root` – የመራጮች ስብስብ Merkle ሥር።
- ** መራጭ ***: `s_vote` በነጠላ ህዝብ በተሞላው ረድፍ ላይ ያለውን በር ያስችለዋል.

ሁሉም የምክር ሴሎች በክልሉ የመጀመሪያ (እና ብቻ) ረድፍ ውስጥ ይመደባሉ; ወረዳው `SimpleFloorPlanner` ይጠቀማል።

### የበር ስርዓት

`H` ከላይ የተገለፀው መጭመቂያ እና `prev_0 = H(v, ρ)` ይሁን። በሩ ያስገድዳል:

1. `s_vote · v · (v - 1) = 0` - ቡሊያን የድምጽ መስጫ ቢት.
2. `s_vote · (H(v, ρ) - commit) = 0` - ቁርጠኝነት ወጥነት.
3. ለእያንዳንዱ ጥልቀት `i`፡
   - `s_vote · dir[i] · (dir[i] - 1) = 0` - የቦሊያን መንገድ አቅጣጫ።
   - `left = H(prev_i, sibling[i])`
   - `right = H(sibling[i], prev_i)`
   - `expected = (1 - dir[i]) · left + dir[i] · right`
   - `s_vote · (node[i] - expected) = 0`
   - `prev_{i+1} = node[i]`
4. `s_vote · (prev_8 - root) = 0` - accumulator የህዝብ Merkle ስርወ ጋር እኩል ነው.

መጭመቂያው የኩዊቲክ ቅርጾችን ብቻ ይጠቀማል; ምንም የመፈለጊያ ጠረጴዛዎች አያስፈልጉም. ሁሉም አርቲሜቲክስ በፓስታ ስካላር መስክ ውስጥ ይከናወናሉ, እና የረድፉ ብዛት `k = 6` `2^k = 64` ረድፎችን ይመድባል - የረድፍ ዜሮ ብቻ ነው የሚኖረው።

### ቀኖናዊ መጫዎቻ

የመወሰኛ ማሰሪያው (`zk_testkit::vote_merkle8_bundle`) ምስክሩን በሚከተሉት ይሞላል፡-

- `v = 1`
- `ρ = 12345`
- `sibling[i] = 10 + i` ለ `i ∈ [0, 7]`
- `dir[i] = 0`
- `node[i] = H(node[i-1], sibling[i])` ከ `node[-1] = H(v, ρ)` ጋር

ይህ የህዝብ እሴቶችን ይፈጥራል-

```text
commit = 0x20574662a58708e02e0000000000000000000000000000000000000000000000
root   = 0xb63752ff429362c3a9b3cd5966c23567fdb757ce3b38af724b9303a5ea2f5817
```

በማረጋገጫ ቁልፍ መዝገብ ውስጥ የተመዘገበው `public_inputs_schema_hash` ነው፡

```text
public_inputs_schema_hash = 0xfae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3
```

### ቁልፍ መዝገብ በማረጋገጥ ላይ

አስተዳደር አረጋጋጭን በሚከተለው ስር ይመዘግባል፡-- `backend = "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1"`
- `circuit_id = "halo2/pasta/vote-bool-commit-merkle8-v1"`
- `backend tag = BackendTag::Halo2IpaPasta`
- `curve = "pallas"`
- `public_inputs_schema_hash = 0xfae4…64d3`
- `commitment = sha256(backend || vk_bytes)` (32-ባይት መፍጨት)

ቀኖናዊው ጥቅል የመስመር ውስጥ ማረጋገጫ ቁልፍ (`key = Some(VerifyingKeyBox { … })`) ከማስረጃ ኤንቨሎፕ ጋር ያካትታል። `vk_len`፣ `max_proof_bytes`፣ እና አማራጭ ሜታዳታ ዩአርአይዎች ከተፈጠሩት ቅርሶች ተሞልተዋል።

## የማጣቀሻ እቃዎች

በውህደት ሙከራዎች የሚበላውን የመስመር ውስጥ ማረጋገጫ ቁልፍ እና ማረጋገጫ ጥቅል ለማደስ `cargo xtask zk-vote-tally-bundle --print-hashes` ይጠቀሙ (ውጤቶች በነባሪ በ`fixtures/zk/vote_tally/` ውስጥ ይገኛሉ)። ትዕዛዙ አጭር ማጠቃለያ (`backend`፣ `commit`፣ `root`፣ schema hash፣ ርዝመቶች) እና እንደ አማራጭ የፋይል hashes ኦዲተሮች የማረጋገጫ ማስታወሻዎችን እንዲይዙ ያትማል። ልክ እንደ JSON ተመሳሳይ ውሂብ ለማውጣት `--summary-json -` ይለፉ (ወይም ወደ ዲስክ ለመፃፍ ዱካ ያቅርቡ)። `--attestation attestation.json` (ወይም `-` ለ stdout) ይለፉ Norito JSON ዝርዝር መግለጫው ማጠቃለያውን እና Blake2b-256 ማጠቃለያዎችን እና መጠኖችን የያዘ ለእያንዳንዱ የጥቅል ቅርስ ስለዚህ የማረጋገጫ እሽጎች ከመሳሪያው ጋር በማህደር ሊቀመጡ ይችላሉ። በ`--verify` ሲሮጡ የ`--attestation <path>` ቼኮች የማስታወሻ ደብተር ዲበዳታ እና የቅርስ ርዝማኔዎች አዲስ ከታደሰው ጥቅል ጋር የሚዛመዱ መሆናቸውን ያረጋግጣል (በየሩጫ የማረጋገጫ መፍጫውን አይወዳደርም፣ ይህም ከግልባጭ የዘፈቀደነት ለውጥ ጋር)።

ቀኖናዊ መገልገያዎችን ያድሱ እና ይገለጡ፡

```bash
cargo xtask zk-vote-tally-bundle \
  --out fixtures/zk/vote_tally \
  --print-hashes \
  --attestation fixtures/zk/vote_tally/bundle.attestation.json
```

ተመዝግበው የገቡት ቅርሶች አሁን እንደሆኑ መቆየታቸውን ያረጋግጡ (የመነሻ ቅርቅቡን እንዲይዝ የቋሚ ማውጫው ይፈልጋል)

```bash
cargo xtask zk-vote-tally-bundle \
  --out fixtures/zk/vote_tally \
  --verify \
  --attestation fixtures/zk/vote_tally/bundle.attestation.json
```

ምሳሌ አንጸባራቂ፡-

```jsonc
{
  "generated_unix_ms": 3513801751697071715,
  "hash_algorithm": "blake2b-256",
  "bundle": {
    "backend": "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1",
    "circuit_id": "halo2/pasta/vote-bool-commit-merkle8-v1",
    "commit_hex": "20574662a58708e02e0000000000000000000000000000000000000000000000",
    "root_hex": "b63752ff429362c3a9b3cd5966c23567fdb757ce3b38af724b9303a5ea2f5817",
    "public_inputs_schema_hash_hex": "fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3",
    "vk_commitment_hex": "6f4749f5f75fee2a40880d4798123033b2b8036284225bad106b04daca5fb10e",
    "vk_len": 66,
    "proof_len": 2748
  },
  "artifacts": [
    {
      "file": "vote_tally_meta.json",
      "len": 522,
      "blake2b_256": "5d0030856f189033e5106415d885fbb2e10c96a49c6115becbbff8b7fd992b77"
    },
    {
      "file": "vote_tally_proof.zk1",
      "len": 2748,
      "blake2b_256": "01449c0599f9bdef81d45f3be21a514984357a0aa2d7fcf3a6d48be6307010bb"
    },
    {
      "file": "vote_tally_vk.zk1",
      "len": 66,
      "blake2b_256": "2fd5859365f1d9576c5d6836694def7f63149e885c58e72f5c4dff34e5005d6b"
    }
  ]
}
```

የአሁኑን አንጸባራቂ ከቀኖናዊ ቅርሶችዎ አጠገብ ያከማቹ (ለምሳሌ በ`fixtures/zk/vote_tally/bundle.attestation.json`)። የላይኛው ተፋሰስ ማከማቻ ትልቅ ሁለትዮሽ ቅርቅቦችን ላለመፈጸም ይህንን ማውጫ ባዶ ያደርገዋል፣ ስለዚህ በ`--verify` ላይ ከመተማመንዎ በፊት በአገር ውስጥ ዘሩ።

`generated_unix_ms` ከቁርጠኝነት/ከማረጋገጫ ቁልፍ የጣት አሻራ የተገኘ ነው ስለዚህ በዳግም መወለድ የተረጋጋ ይሆናል። ጀነሬተሩ ቋሚ የChaCha20 ግልባጭ ይጠቀማል፣ ስለዚህ ሜታዳታ፣ የማረጋገጫ ቁልፍ እና የማረጋገጫ ኤንቨሎፕ ሃሽ ሊባዙ ይችላሉ። ማንኛውም የምግብ መፍጨት አለመመጣጠን አሁን መመርመር ያለበትን መንሸራተት ያሳያል። ኦዲተሮች የሚለቀቁትን እሴቶች ከሚመሰክሩት ቅርስ ጋር መመዝገብ አለባቸው።

የስራ ፍሰት አስታዋሽ፡

1. ጥቅሉን በአገር ውስጥ ለመዝራት `cargo xtask zk-vote-tally-bundle --out fixtures/zk/vote_tally --print-hashes --attestation fixtures/zk/vote_tally/bundle.attestation.json` ን ያሂዱ።
2. እንደ አስፈላጊነቱ የተገኙ ቅርሶችን ግባ ወይም በማህደር ያስቀምጡ።
3. ማረጋገጫው ከቀኖናዊው ጥቅል ጋር የሚዛመድ መሆኑን ለማረጋገጥ `--verify` በሚቀጥሉት እድሳት ላይ ይጠቀሙ።

ከውስጥ ተግባሩ የሚወስነውን ጄኔሬተር በ`xtask/src/vote_tally.rs` ያካሂዳል፡

1. ምስክሮቹን (`v = 1`, `ρ = 12345`, እህትማማቾች `10..17`) ናሙናዎች.
2. `keygen_vk`/`keygen_pk` ይሰራል
3. የHalo2 ማስረጃን አምርቶ በZK1 ኤንቨሎፕ (የህዝብ ጉዳዮችን ጨምሮ) ይጠቀለላል።
4. የማረጋገጫ ቁልፍ መዝገቡን በተገቢው `public_inputs_schema_hash` ያወጣል።

## የመነካካት ሽፋን

`crates/iroha_core/tests/zk_vote_tally_audit.rs` ጥቅሉን ጭኖ ይፈትሻል፡

- እውነተኛው ማስረጃ በተጠቀለለው የመስመር ላይ ቪኬ ላይ ያረጋግጣል።
- በቁርጠኝነት አምድ ውስጥ ያለውን ማንኛውንም ባይት መገልበጥ ማረጋገጫው እንዳይሳካ ያደርጋል።
- ማንኛውንም ባይት በስር አምድ ውስጥ መገልበጥ ማረጋገጫው እንዳይሳካ ያደርጋል።እነዚህ የመልሶ ማቋቋሚያ ሙከራዎች ለTorii (እና አስተናጋጆች) የህዝብ ግብአቶች ከማስረጃ ማመንጨት በኋላ የተበላሹ ኤንቨሎፖችን እንደማይቀበሉ ዋስትና ይሰጣሉ።

ድግግሞሹን በአካባቢው ያሂዱ በ፡

```bash
cargo test -p iroha_core zk_vote_tally_audit -- --nocapture
```

## የኦዲት ማረጋገጫ ዝርዝር

1. `VoteBoolCommitMerkle::<8>` የግዴታ ሙሉነት እና የማያቋርጥ ምርጫን ይገምግሙ።
2. ቪኬ/ማስረጃውን ለማባዛት እና የተቀዳውን ሃሽ ለማረጋገጥ `cargo xtask zk-vote-tally-bundle --verify --print-hashes`ን እንደገና ያሂዱ።
3. ያረጋግጡ የTorii የቁመት ተቆጣጣሪ ተመሳሳዩን የኋላ ለዪ እና የፖስታ አቀማመጥ ይጠቀማል።
4. የተለወጡ ማረጋገጫዎች ማረጋገጥ አለመሳካቱን ለማረጋገጥ የቴምፐር ሪግሬሽንን ያስፈጽሙ።
5. ሃሽ እና የ`bundle.attestation.json` ውፅዓት (Blake2b-256) ገምግመው ገምጋሚዎች ቀኖናዊውን ማኒፌክት ከማስረጃዎቻቸው ጋር መመዝገብ ይችላሉ።