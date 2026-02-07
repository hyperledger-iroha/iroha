---
id: chunker-registry
lang: dz
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Profile Registry
sidebar_label: Chunker Registry
description: Profile IDs, parameters, and negotiation plan for the SoraFS chunker registry.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::དྲན་ཐོའི་འབྱུང་ཁུངས།
:::

## I18NT00000000 ཆུར་ཀར་གསལ་སྡུད་ཐོ་གཞུང་ (SF-2a)

SoraFS གིས་ མིང་ཐོ་བཀོད་ཆུང་བ་ཅིག་བརྒྱུད་དེ་ སྤྱོད་ལམ་འགྱུར་བའི་ སྤྱོད་ལམ་ཚུ་ གྲོས་བསྟུན་འབདཝ་ཨིན།
གསལ་སྡུད་རེ་རེ་གིས་ གཏན་འབེབས་སི་ཌི་སི་ཚད་བཟུང་དང་ སེམ་ཝར་མེ་ཊ་ཌེ་ཊ་ དེ་ལས་ ཚུ་འགན་སྤྲོད་འབདཝ་ཨིན།
རེ་བ་ཡོད་པའི་ ཟས་བཅུད་/multicode གསལ་སྟོན་དང་ སི་ཨར་ གཏན་མཛོད་ནང་ ལག་ལེན་འཐབ་མི་ བཞུ་བཅོས་འབད་ཡོདཔ།

གསལ་སྡུད་རྩོམ་པ་པོ་ཚུ་གིས་ གྲོས་བསྟུན་འབད་དགོ།
[I18NI0000014X](./chunker-profile-authoring.md)
དགོས་མཁོའི་མེ་ཊ་ཌེ་ཊ་དང་ བདེན་དཔྱད་ཞིབ་དཔྱད་ཐོ་ཡིག་ དེ་ལས་ གྲོས་འཆར་ཊེམ་པེལེཊི་ཚུ་གི་དོན་ལུ་ དེ་གི་ཧེ་མ་ཨིན།
ཐོ་བཀོད་གསརཔ་བཙུགས་དོ། གཞུང་སྐྱོང་གིས་ བསྒྱུར་བཅོས་འབད་ནི་གི་ ཆ་འཇོག་འབད་ཚརཝ་ཅིག་ རྗེས་སུ་འབྲང་དགོ།
[ཐོ་བཀོད་ཀྱི་ བསྐོར་ཐེངས་ཐོ་ཡིག་](./chunker-registry-rollout-checklist.md) དང་།
[འཁྲབ་སྟོན། གསལ་བསྒྲགས།](I18NU0000012X) འཕེལ་རྒྱས་གཏོང་བའི་ཆེད་དུ།
འཁྲབ་སྟོན་དང་ཐོན་སྐྱེད་བརྒྱུད་དེ་ སྒྲིག་བཀོད་ཚུ།

### གསལ་སྡུད།

| མིང་གནས་ས་ | མིང | སེམ་ཝར་ | གསལ་སྡུད་ཨའི་ཌི་ | མིན་ (བཱའིཊི་) | དམིགས་ཚད་ (bytes) | མཐོ་ཤོས་ (བཱའིཊི་) | བརྡབ་གསིག་ | མལ་ཊི་ཧཤ་ | འདྲེན་བཀོལ། | དྲན་ཐོ། |
|-------------------------------------------------------------- ------- ----------------------------------------------------------------------- ------|
| `sorafs` | `sf1` | `1.0.0` | `1` | ༦༥༥༣༦ | ༢༦༢༡༤༤ | ༥༢༤༢༨༨ | `0x0000ffff` | `0x1f` (BLAKE3-256) | I18NI0000021X | ཨེསི་ཨེཕ་-༡ གི་སྒྲིག་བཀོད་ཚུ་ནང་ལག་ལེན་འཐབ་ཡོད་པའི་ ཀེ་ནོ་ནིག་གསལ་སྡུད་ |

ཐོ་བཀོད་འདི་ `sorafs_manifest::chunker_registry` སྦེ་ གསང་ཡིག་ནང་སྡོད་དོ་ཡོདཔ་ཨིན། ( [`chunker_registry_charter.md`](I18NU000000013X). འཛུལ་ཞུགས་རེ་རེ།
འདི་ `ChunkerProfileDescriptor` དང་གཅིག་སྦེ་བཀོད་ཡོདཔ་ཨིན།

* I18NI0000025X – འབྲེལ་ཡོད་གསལ་སྡུད་ཀྱི་ཚད་མའི་སྡེ་ཚན་ (དཔེར་ན་ I18NI0000026X).
* I18NI000000027X – མི་ལྷག་ཚུགས་པའི་གསལ་སྡུད་ཀྱི་ཁ་ཡིག་ (`sf1`, `sf1-fast`, ...)
* `semver` – ཚད་བཟུང་ཆ་ཚན་གྱི་དོན་ལུ་ ཡིག་བརྡའི་ཐོན་རིམ་ཡིག་རྒྱུན།
* I18NI0000031X – I18NI000000032X (min/target/max/mask) ངོ་མ་འདི་ཨིན།
* `multihash_code` – ཆུང་ཆུང་བཟོ་བའི་སྐབས་ multihas འདི་ (`0x1f` བཞུ་བཅུག་མི་ (I18NI0000034X
  SoraFS གི་དོན་ལུ་)

གསལ་སྟོན་འདི་གིས་ `ChunkingProfileV1` བརྒྱུད་དེ་ གསལ་སྡུད་ཚུ་ རིམ་སྒྲིག་འབདཝ་ཨིན། གཞི་བཀོད་དྲན་ཐོ་ཚུ།
ཐོ་བཀོད་མེ་ཊ་ཌེ་ཊ་ (namepace, name, semver) དང་མཉམ་པའི་ སི་ཌི་སི་ མཉམ་དུ།
ཚད་བཟུང་ཚུ་དང་ གོང་ལུ་སྟོན་ཡོད་པའི་མིང་གཞན་ཐོ་ཡིག་འདི། ཉོ་སྤྱོད་པ་ཚུ་གིས་ འགོ་དང་པ་རང་ དཔའ་བཅམ་དགོ།
ཐོ་བཀོད་འཚོལ་ཞིབ་ I18NI000000036X དང་ ནང་ཐིག་ཚད་བཟུང་ཚུ་ལུ་ལོག་འགྱོཝ་ཨིན།
ངོ་ཤེས་མི་ ID ཚུ་འཐོན་དོ་ཡོདཔ། ཐོ་བཀོད་ཆོག་ཐམ་ལམ་ལུགས་ཚུ་ལུ་ ཀེ་ནོ་ནིག་ལག་ལེན་འཐབ་དགོཔ་ཨིན།
(`namespace.name@semver`) འདི་ `profile_aliases` ནང་ ཐོ་བཀོད་འགོ་དང་པ་ཅིག་ཨིན།

ཐོ་བཀོད་འདི་ ལག་ཆས་ཚུ་ནང་ལས་ བརྟག་དཔྱད་འབད་ནི་ལུ་ གྲོགས་རམ་པ་སི་ཨེལ་ཨའི་འདི་ གཡོག་བཀོལ།

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
$ ཅ་ཆས་བང་རྒྱུག་ -p སོ་རཕ་_མན་ཕེསཊ་ --bin sorafs_manifest_chunk_store - - ./docs.tar \
    --por-repoof=༠:༠ ---por-por-reouf-out= འདབ་མ་བདེན་དཔང་.json
I18NF0000004X
$ ཅ་ཆས་བང་རྒྱུག་ -p སོ་རཕ་_མན་ཕེསཊ་ -- བིན་སོ་རཕ་_མན་ཕེསཊི་_ཆུངཀ་_སི་ཊོར་ -- \
    --promote-གསལ་སྡུད་=སོ་རཕ་སི་.sf1@1.0.0.0.
```

The main report (and optional proof file) include the root digest, the sampled
leaf bytes (hex-encoded), and the segment/chunk sibling digests so verifiers can
rehash the 64 KiB/4 KiB layers against the `por_root_hex` value.

To validate an existing proof against a payload, pass the path via
`--por-proof-verify` (the CLI adds `"por_proof_verified": true` when the witness
matches the computed root):

```
$ ཅ་ཆས་བང་རྒྱུག་ -p སོ་རཕ་_མན་ཕེསཊ་ --bin sorafs_manifest_chunk_store - - ./docs.tar \
    --por-por-por-repifify=leaf.porof.json
```

For batch sampling, use `--por-sample=<count>` and optionally provide a seed/
output path. The CLI guarantees deterministic ordering (`splitmix64` seeded)
and will transparently truncate when the request exceeds the available leaves:

```
$ ཅ་ཆས་བང་རྒྱུག་ -p སོ་རཕ་_མན་ཕེསཊ་ --bin sorafs_manifest_chunk_store - - ./docs.tar \
    --por-དཔེ་ཚད་=༨ --por-དཔེ་ཚད་-སེ་ཌི་=༠xfeedface --por-དཔེ་ཚད་-ཨའུཊི་=པོར་.ཇི་སོན།

གསལ་སྟོན་འདི་གིས་ གནས་སྡུད་གཅིགཔོ་འདི་ གསལ་སྟོན་འབདཝ་ཨིནམ་ད་ འདི་ཡང་ པའིཔ་ལའིན་ནང་ལུ་ I18NI000000039X སེལ་འཐུ་འབད་བའི་སྐབས་ སྟབས་བདེ་ཏོག་ཏོ་ཨིན། ཆ་རྐྱེན་ཚོང་ཁང་གིས་ སི་ཨེལ་ཨའི་གཉིས་ཆ་ར་གིས་ ཀེ་ནོ་ནིག་ལག་ལེབ་ཀྱི་རྣམ་པ་ (`--profile=sorafs.sf1@1.0.0`) ཡང་ངོས་ལེན་འབདཝ་ལས་ ཡིག་ཚུགས་ཚུ་བཟོ་བསྐྲུན་འབད་མི་འདི་གིས་ ཧརཌི་ཀོཌིང་ཨང་གྲངས་ཀྱི་ཨའི་ཌི་ཚུ་ བཀག་ཚུགས།

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

`handle` ས་སྒོ་ (I18NI000000042X) གིས་ སི་ཨེལ་ཨའི་ཨེསི་གིས་ ངོས་ལེན་འབད་མི་འདི་དང་མཐུན་སྒྲིག་འབདཝ་ཨིན།
`--profile=…`, ཐད་ཀར་རང་བཞིན་ནང་འདྲ་བཤུས་རྐྱབ་ནི་ལུ་ཉེན་སྲུང་ཡོདཔ་ཨིན།

### གྲོས་བསྟུན་ཆུང་ཀུ།

སྒོ་སྒྲིག་དང་ མཁོ་མངགས་འབད་མི་ཚུ་གིས་ བྱིན་མི་ ཁྱབ་བསྒྲགས་ཚུ་བརྒྱུད་དེ་ རྒྱབ་སྐྱོར་འབད་ཡོད་པའི་ གསལ་སྡུད་ཚུ་ ཁྱབ་བསྒྲགས་འབདཝ་ཨིན།

I18NF0000008X

སྣ་མང་འབྱུང་ཁུངས་ཀྱི་ ཆ་ཤས་དུས་ཚོད་བཟོ་ནི་འདི་ `range` ལྕོགས་གྲུབ་བརྒྱུད་དེ་གསལ་བསྒྲགས་འབདཝ་ཨིན། ཚིག༌ཕྲད
CLI གིས་ I18NI000000045X དང་ཅིག་ཁར་ངོས་ལེན་འབདཝ་ཨིན།
རྗེས་འཇུག་ཨིན་ཀོ་ཨེན་ཀོ་འདི་བྱིན་མི་གི་དགའ་གདམ་ཅན་གྱི་ཁྱབ་ཚད་-ཕེཆ་མཉམ་བསྡོམས་ (དཔེར་ན་།
`--capability=range:64` གིས་ ༦༤ གི་འཆར་དངུལ་ཁྱབ་བསྒྲགས་འབདཝ་ཨིན།) བཀོ་བཞག་པའི་སྐབས་ ཉོ་སྤྱོད་པ་ཚུ།
སྤྱིར་བཏང་ I18NI000000047X ཁྱབ་བསྒྲགས་ནང་ གཞན་ཁར་ དཔར་བསྐྲུན་འབད་ཡོད་པའི་ བརྡ་སྟོན་ལུ་ ལོག་འོང་།

CAR གནད་སྡུད་ཞུ་བ་འབད་བའི་སྐབས་ མཁོ་མངགས་འབད་མི་ཚུ་གིས་ `Accept-Chunker` མགོ་ཡིག་ཐོ་ཡིག་གཏང་དགོ།
རྒྱབ་སྐྱོར། `(namespace, name, semver)` དགའ་གདམ་གྱི་གོ་རིམ་ནང་ ཊུབ་ལི་ཚུ་:

I18NF0000009X

སྒོ་སྒྲིག་ཚུ་གིས་ ཕན་ཚུན་རྒྱབ་སྐྱོར་འབད་ཡོད་པའི་གསལ་སྡུད་ཅིག་ སེལ་འཐུ་འབདཝ་ཨིན་ (`sorafs.sf1@1.0.0` ལུ་སྔོན་སྒྲིག་འབད་ནི།)
དེ་ལས་ `Content-Chunker` ལན་འདེབས་མགོ་ཡིག་བརྒྱུད་དེ་ གྲོས་ཐག་འདི་ བསམ་ཞིབ་འབད། མངོན་གསལ་ཚུ།
སེལ་འཐུ་འབད་ཡོད་པའི་གསལ་སྡུད་འདི་བཙུགས་ཡོདཔ་ལས་ མར་རིམ་ནའུཊི་ཚུ་གིས་ ཅགི་སྒྲིག་བཀོད་འདི་བདེན་དཔྱད་འབད་ཚུགས།
མེད་པར་ HTTP མཐུན་སྒྲིག་ལ་བརྟེན་དགོས།

### མཐུན་སྒྲིལ།

* `sorafs.sf1@1.0.0` གསལ་སྡུད་འདི་གིས་ ༢༠༠༨ ལུ་ མི་མང་སྒྲིག་བཀོད་ཚུ་ལུ་ སབ་ཁྲ་བཟོཝ་ཨིན།
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

``` དང་ ཀོར་པོ་ར་ གཤམ་གསལ་གྱི་ གཤམ་གསལ་གྱི་ གཤམ་གསལ་གྱི་ གཤམ་གསལ་ལྟར་
  `fuzz/sorafs_chunker`. མཐའ་མའི་འདྲ་མཉམ་འདི་ Rust, Go, དང་ Node ནང་ལུ་ལག་ལེན་འཐབ་ཨིན།
  བྱིན་ཡོད་པའི་བརྟག་དཔྱད་ཚུ་བརྒྱུད་དེ་ཨིན།
* `chunker_registry::lookup_by_profile` གིས་ འགྲེལ་བཤད་ཀྱི་ཚད་བཟུང་ཚུ་གིས་ བརྡ་སྟོནམ་ཨིན།
  mattery `ChunkProfile::DEFAULT` རྐྱེན་ངན་གྱི་ཁྱད་པར་སྲུང་ནིའི་དོན་ལུ་ཨིན།
* I18NI000000057X དང་ I18NI000000058X གིས་བཟོ་མི་ མ་ནི་ཕེསི་ཚུ་ ཐོ་བཀོད་མེ་ཊ་ཌེ་ཊ་ ཚུདཔ་ཨིན།