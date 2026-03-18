---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d43c9ac18ba6b9e4e7941325b2ebdec672b01627747e3272927557faf82957af
source_last_modified: "2026-01-22T14:35:36.798331+00:00"
translation_last_reviewed: 2026-02-07
id: chunker-registry
title: SoraFS Chunker Profile Registry
sidebar_label: Chunker Registry
description: Profile IDs, parameters, and negotiation plan for the SoraFS chunker registry.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

## SoraFS Chunker መገለጫ መዝገብ ቤት (SF-2a)

የSoraFS ቁልል በትንሽ ስም በተሰየመ መዝገብ የመቁረጥ ባህሪን ይደራደራል።
እያንዳንዱ መገለጫ የሚወስን የሲዲሲ መለኪያዎችን፣ ሴምቨር ሜታዳታን እና የ
የሚጠበቀው ዳይጀስት/መልቲኮዴክ በማኒፌስት እና በ CAR መዛግብት ውስጥ ጥቅም ላይ ይውላል።

የመገለጫ ደራሲዎች ማማከር አለባቸው
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
ከዚህ በፊት ለሚፈለገው ሜታዳታ፣ የማረጋገጫ ዝርዝር እና የፕሮፖዛል አብነት
አዲስ ግቤቶችን ማስገባት. አንዴ አስተዳደር ለውጡን ካፀደቀ፣ ይከተሉ
[የመዝገቢያ ልቀት ዝርዝር](./chunker-registry-rollout-checklist.md) እና የ
[ማሳያ ማጫወቻ መጽሐፍ](./staging-manifest-playbook) ለማስተዋወቅ
እቃዎቹን በማዘጋጀት እና በማምረት.

### መገለጫዎች

| የስም ቦታ | ስም | SemVer | የመገለጫ መታወቂያ | ደቂቃ (ባይት) | ዒላማ (ባይት) | ከፍተኛ (ባይት) | ጭንብል መስበር | መልቲሃሽ | ተለዋጭ ስሞች | ማስታወሻ |
|--------|-------|--------|------------|------------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0"]` | በ SF-1 ዕቃዎች ውስጥ ጥቅም ላይ የዋለ ቀኖናዊ መገለጫ |

መዝገቡ እንደ I18NI0000022X (በ[`chunker_registry_charter.md`](./chunker-registry-charter.md) የሚተዳደር በኮድ ነው የሚኖረው። እያንዳንዱ ግቤት
እንደ `ChunkerProfileDescriptor` ተገልጿል፡

* `namespace` - ተዛማጅ መገለጫዎች አመክንዮአዊ ስብስብ (ለምሳሌ `sorafs`)።
* `name` - በሰው ሊነበብ የሚችል የመገለጫ መለያ (`sf1`፣ `sf1-fast`፣ …)።
* `semver` - ለትርጉም ስብስብ የፍቺ ስሪት ሕብረቁምፊ።
* `profile` - ትክክለኛው `ChunkProfile` (ደቂቃ / ዒላማ / ከፍተኛ / ጭንብል).
* `multihash_code` - የ chunk digestes ለማምረት ጥቅም ላይ የሚውለው መልቲሃሽ (`0x1f`)
  ለ SoraFS ነባሪ)።

አንጸባራቂው መገለጫዎችን በ`ChunkingProfileV1` በኩል ተከታታይ ያደርገዋል። መዋቅሩ ይመዘግባል
የመዝገብ ሜታዳታ (ስም ቦታ፣ ስም፣ ሴምቨር) ከጥሬው ሲዲሲ ጋር
መለኪያዎች እና ከላይ የሚታየው ተለዋጭ ዝርዝር። ሸማቾች መጀመሪያ መሞከር አለባቸው ሀ
የመመዝገቢያ ፍለጋ በI18NI0000036X እና ወደ የመስመር ውስጥ መለኪያዎች ይመለሱ
ያልታወቁ መታወቂያዎች ይታያሉ። የመመዝገቢያ ቻርተር ደንቦች ቀኖናዊውን እጀታ ይጠይቃሉ
(`namespace.name@semver`) በ `profile_aliases` ውስጥ የመጀመሪያው ግቤት ለመሆን።

መዝገቡን ከመሳሪያነት ለመመርመር፣ አጋዥውን CLI ያሂዱ፡-

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]

All of the CLI flags that write JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) accept `-` as the path, which streams the payload to stdout instead of
creating a file. This makes it easy to pipe the data into tooling while still keeping the
default behaviour of printing the main report.

To inspect a specific PoR witness, provide chunk/segment/leaf indices and
optionally persist the proof to disk:

```
$ የካርጎ ሩጫ -p sorafs_manifest --ቢን sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

You can select a profile by numeric id (`--profile-id=1`) or by registry handle
(`--profile=sorafs.sf1@1.0.0`); the handle form is convenient for scripts that
thread namespace/name/semver directly from governance metadata.

Use `--promote-profile=<handle>` to emit a JSON metadata block (including all
registered aliases) that can be pasted into `chunker_registry_data.rs` when
promoting a new default profile:

```
$ የካርጎ ሩጫ -p sorafs_manifest --ቢን sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

The main report (and optional proof file) include the root digest, the sampled
leaf bytes (hex-encoded), and the segment/chunk sibling digests so verifiers can
rehash the 64 KiB/4 KiB layers against the `por_root_hex` value.

To validate an existing proof against a payload, pass the path via
`--por-proof-verify` (the CLI adds `"por_proof_verified": true` when the witness
matches the computed root):

```
$ የካርጎ ሩጫ -p sorafs_manifest --ቢን sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

For batch sampling, use `--por-sample=<count>` and optionally provide a seed/
output path. The CLI guarantees deterministic ordering (`splitmix64` seeded)
and will transparently truncate when the request exceeds the available leaves:

```
$ የካርጎ ሩጫ -p sorafs_manifest --ቢን sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json

አንጸባራቂው ስቱብ ተመሳሳይ ውሂብን ያንጸባርቃል፣ ይህም `--chunker-profile-id` ምርጫን በቧንቧ ሲጽፉ ምቹ ነው። ሁለቱም የመደብር ማከማቻ CLIዎች ቀኖናዊውን መያዣ ቅጽ (`--profile=sorafs.sf1@1.0.0`) ይቀበላሉ ስለዚህ ስክሪፕቶችን ይገንቡ ጠንካራ ኮድ የቁጥር መታወቂያዎችን ያስወግዳሉ፡

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "profile_id": 1,
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

የ`handle` መስክ (`namespace.name@semver`) CLIs ከሚቀበሉት ጋር ይዛመዳል።
`--profile=…` ፣ በቀጥታ ወደ አውቶሜትድ ለመቅዳት ደህንነቱ የተጠበቀ ያደርገዋል።

### ቸንከር መደራደር

መግቢያ መንገዶች እና ደንበኞች የሚደገፉ መገለጫዎችን በአቅራቢ ማስታወቂያዎች ያስተዋውቃሉ፡-

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implicit via registry)
    capabilities: [...]
}
```

ባለብዙ-ምንጭ ቻንክ መርሐግብር በ`range` አቅም በኩል ይፋ ሆኗል። የ
CLI ከ I18NI0000045X ጋር ይቀበላል፣የአማራጭ ቁጥር
ቅጥያ የአቅራቢውን ተመራጭ ክልል-ማምጣትን (ለምሳሌ፦
`--capability=range:64` ባለ 64-ዥረት በጀት ያስተዋውቃል)። ሲቀሩ ሸማቾች
በማስታወቂያው ላይ ሌላ ቦታ ወደታተመው አጠቃላይ I18NI0000047X ፍንጭ ይመለሱ።

የCAR ውሂብን ሲጠይቁ ደንበኞች የ`Accept-Chunker` አርዕስት ዝርዝር መላክ አለባቸው።
የሚደገፉ `(namespace, name, semver)` tuples በምርጫ ቅደም ተከተል፡

```
Accept-Chunker: sorafs.sf1;version=1.0.0
```

ጌትዌይስ በጋራ የሚደገፍ መገለጫን ምረጥ (ወደ `sorafs.sf1@1.0.0` ነባሪ)
እና ውሳኔውን በI18NI0000051X ምላሽ ራስጌ በኩል ያንጸባርቁ። ይገለጣል
የታችኛው አንጓዎች የተቆራረጠውን አቀማመጥ ማረጋገጥ እንዲችሉ የተመረጠውን መገለጫ ይክቱ
በኤችቲቲፒ ድርድር ላይ ሳንተማመን።

### ስምምነት

* የ `sorafs.sf1@1.0.0` የመገለጫ ካርታዎች በ ውስጥ ላሉ የህዝብ መገልገያዎች
  `fixtures/sorafs_chunker` እና ኮርፖራ በስር ተመዝግቧል
  `fuzz/sorafs_chunker`. ከጫፍ እስከ ጫፍ ያለው እኩልነት በሩስት፣ ሂድ እና መስቀለኛ መንገድ ላይ ይሰራል
  በቀረቡት ፈተናዎች በኩል.
* `chunker_registry::lookup_by_profile` ገላጭ መለኪያዎችን ያረጋግጣል
  የአጋጣሚ ልዩነትን ለመጠበቅ `ChunkProfile::DEFAULT` ግጥሚያ።
* በI18NI0000057X እና `sorafs_manifest_stub` የተሰሩ መግለጫዎች የመመዝገቢያ ሜታዳታ ያካትታሉ።