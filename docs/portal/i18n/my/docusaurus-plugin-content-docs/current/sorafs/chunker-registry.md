---
id: chunker-registry
lang: my
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

::: Canonical Source ကို သတိပြုပါ။
:::

## SoraFS Chunker Profile Registry (SF-2a)

SoraFS stack သည် သေးငယ်ပြီး namespaced registry တစ်ခုမှတစ်ဆင့် အတုံးလိုက်အပြုအမှုကို ညှိနှိုင်းသည်။
ပရိုဖိုင်တစ်ခုစီသည် သတ်မှတ်ထားသော CDC ကန့်သတ်ချက်များ၊ semver မက်တာဒေတာနှင့် တို့ကို သတ်မှတ်ပေးသည်။
မန်နီးဖက်စ်များနှင့် CAR မော်ကွန်းတိုက်များတွင် အသုံးပြုသော မျှော်လင့်ထားသော ချေဖျက်မှု/ဘက်စုံကုဒ်။

ပရိုဖိုင်ရေးသားသူများ တိုင်ပင်သင့်သည်။
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
လိုအပ်သော မက်တာဒေတာ၊ တရားဝင်စစ်ဆေးချက်စာရင်းနှင့် အဆိုပြုချက်ပုံစံပုံစံအတွက် မတိုင်မီ
အသစ်တင်သွင်းခြင်း။ အုပ်ချုပ်ရေးက အပြောင်းအလဲတစ်ခု အတည်ပြုပြီးသည်နှင့် လိုက်နာပါ။
[registry rollout checklist](./chunker-registry-rollout-checklist.md) နှင့်
မြှင့်တင်ရန် [staging manifest playbook](./staging-manifest-playbook)
ဇာတ်ခုံနှင့် ထုတ်လုပ်မှုအားဖြင့် ဆက်စပ်ပစ္စည်းများ။

### ပရိုဖိုင်များ

| Namespace | အမည် | SemVer | ပရိုဖိုင် ID | မင်း (ဘိုက်) | ပစ်မှတ် (bytes) | မက်(ဘိုက်) | မျက်နှာဖုံးကွဲ | Multihash | နာမည်များ | မှတ်စုများ |
|-----------|------|--------|----------------------|----------------|----------------|------------------------------------------------|----------------|---------|------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0"]` | SF-1 fixtures | တွင်သုံးသော Canonical ပရိုဖိုင်

မှတ်ပုံတင်ခြင်းတွင် `sorafs_manifest::chunker_registry` ([`chunker_registry_charter.md`](./chunker-registry-charter.md)) အဖြစ် ကုဒ်ဖြင့် နေထိုင်ပါသည်။ ဝင်ခွင့်တိုင်း
`ChunkerProfileDescriptor` အဖြစ် ဖော်ပြသည်-

* `namespace` – ဆက်စပ်ပရိုဖိုင်များ၏ ယုတ္တိနည်းဖြင့် အုပ်စုဖွဲ့ခြင်း (ဥပမာ၊ `sorafs`)။
* `name` – လူသားဖတ်နိုင်သော ပရိုဖိုင်တံဆိပ် (`sf1`၊ `sf1-fast`၊ …)။
* `semver` - ကန့်သတ်ဘောင်အတွက် semantic ဗားရှင်းစာကြောင်း။
* `profile` – တကယ့် `ChunkProfile` (min/target/max/mask)။
* `multihash_code` - အတုံးအတုံးအခဲများထုတ်ရာတွင် အသုံးပြုသည့် multihash (`0x1f`
  SoraFS အတွက် မူရင်း)။

မန်နီးဖက်စ်သည် `ChunkingProfileV1` မှတစ်ဆင့် ပရိုဖိုင်များကို အတွဲလိုက်ပြုလုပ်သည်။ ဖွဲ့စည်းပုံ မှတ်တမ်းများ
ကုန်ကြမ်း CDC နှင့်အတူ registry metadata (namespace၊ name၊ semver)
ဘောင်များ နှင့် အထက်ဖော်ပြပါ alias စာရင်း။ စားသုံးသူများ ဦးစွာကြိုးစားသင့်သည်။
`profile_id` ဖြင့် registry lookup လုပ်ပြီး inline parameters သို့ ပြန်ရောက်သွားသည် ။
အမည်မသိ ID များပေါ်လာသည်။ Registry charter rules များသည် canonical handle လိုအပ်ပါသည်။
(`namespace.name@semver`) သည် `profile_aliases` တွင် ပထမဆုံးဝင်ရောက်မှုဖြစ်သည်။

tooling မှ registry ကိုစစ်ဆေးရန်၊ helper CLI ကို run ပါ။

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
$ ကုန်တင်ကုန်ချ -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

You can select a profile by numeric id (`--profile-id=1`) or by registry handle
(`--profile=sorafs.sf1@1.0.0`); the handle form is convenient for scripts that
thread namespace/name/semver directly from governance metadata.

Use `--promote-profile=<handle>` to emit a JSON metadata block (including all
registered aliases) that can be pasted into `chunker_registry_data.rs` when
promoting a new default profile:

```
$ ကုန်တင်ကုန်ချလုပ်ငန်း -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

The main report (and optional proof file) include the root digest, the sampled
leaf bytes (hex-encoded), and the segment/chunk sibling digests so verifiers can
rehash the 64 KiB/4 KiB layers against the `por_root_hex` value.

To validate an existing proof against a payload, pass the path via
`--por-proof-verify` (the CLI adds `"por_proof_verified": true` when the witness
matches the computed root):

```
$ ကုန်တင်ကုန်ချ -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

For batch sampling, use `--por-sample=<count>` and optionally provide a seed/
output path. The CLI guarantees deterministic ordering (`splitmix64` seeded)
and will transparently truncate when the request exceeds the available leaves:

```
$ ကုန်တင်ကုန်ချ -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json

Manifest stub သည် ပိုက်လိုင်းများတွင် `--chunker-profile-id` ရွေးချယ်မှုကို ဇာတ်ညွှန်းရေးသားရာတွင် အဆင်ပြေသည့် တူညီသောဒေတာကို ထင်ဟပ်စေသည်။ အတုံးလိုက်စတိုးဆိုင် CLI နှစ်ခုလုံးသည် canonical handle form (`--profile=sorafs.sf1@1.0.0`) ကိုလည်း လက်ခံသောကြောင့် script များသည် hard-coding ဂဏန်း ID များကို ရှောင်ရှားနိုင်သည်-

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

`handle` အကွက် (`namespace.name@semver`) သည် CLI များမှ လက်ခံသည့်အရာနှင့် ကိုက်ညီသည်
`--profile=…`၊ ၎င်းကို အလိုအလျောက်စနစ်သို့ တိုက်ရိုက်ကူးယူရန် ဘေးကင်းစေသည်။

### ညှိနှိုင်းနေသော Chunkers

Gateway များနှင့် သုံးစွဲသူများသည် ပံ့ပိုးပေးသူကြော်ငြာများမှတစ်ဆင့် ထောက်ခံထားသော ပရိုဖိုင်များကို ကြော်ငြာသည်-

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implicit via registry)
    capabilities: [...]
}
```

Multi-source chunk scheduling ကို `range` စွမ်းရည်မှတစ်ဆင့် ကြေညာသည်။ ဟိ
ရွေးချယ်နိုင်သောဂဏန်းများရှိရာ CLI သည် ၎င်းကို `--capability=range[:streams]` ဖြင့် လက်ခံသည်။
suffix သည် ပံ့ပိုးပေးသူ၏ နှစ်သက်ရာ အပိုင်းအခြား-ယူဆောင်မှု ပေါင်းစပ်ငွေကို ကုဒ်ဝှက်သည် (ဥပမာ၊
`--capability=range:64` သည် 64-စီးကြောင်းဘတ်ဂျက်) ကို ကြော်ငြာသည်။ ချန်လှပ်ထားသည့်အခါ စားသုံးသူများ
ကြော်ငြာတွင် အခြားနေရာတွင် လွှင့်တင်ထားသော ယေဘူယျ `max_streams` အရိပ်အမြွက်ကို ပြန်ပြောင်းပါ။

CAR ဒေတာကို တောင်းဆိုသည့်အခါ၊ သုံးစွဲသူများသည် `Accept-Chunker` ခေါင်းစီးစာရင်းကို ပေးပို့သင့်သည်
ဦးစားပေးအစီအစဉ်တွင် `(namespace, name, semver)` tuples ကို ပံ့ပိုးထားသည်-

```
Accept-Chunker: sorafs.sf1;version=1.0.0
```

Gateways သည် အပြန်အလှန်ပံ့ပိုးထားသော ပရိုဖိုင်ကို ရွေးချယ်ပါ (`sorafs.sf1@1.0.0` သို့ ပုံသေသတ်မှတ်ထားသည်)
`Content-Chunker` တုံ့ပြန်မှု ခေါင်းစီးမှတစ်ဆင့် ဆုံးဖြတ်ချက်ကို ရောင်ပြန်ဟပ်ပါ။ ထင်ရှားသည်။
ရွေးချယ်ထားသော ပရိုဖိုင်ကို မြှုပ်နှံထားသောကြောင့် downstream node များသည် အပိုင်းလိုက်အပြင်အဆင်ကို အတည်ပြုနိုင်သည်။
HTTP ညှိနှိုင်းမှုကို အားမကိုးဘဲ၊

### ညီညွတ်ခြင်း။

* `sorafs.sf1@1.0.0` ပရိုဖိုင်သည် အများသူငှာ ပွဲစဉ်များအတွက် မြေပုံများပြသည်။
  `fixtures/sorafs_chunker` နှင့် corpora အောက်တွင် မှတ်ပုံတင်ထားသည်။
  `fuzz/sorafs_chunker`။ End-to-end parity ကို Rust၊ Go နှင့် Node တွင် ကျင့်သုံးသည်။
  ပေးထားသော စမ်းသပ်မှုများမှတဆင့်
* `chunker_registry::lookup_by_profile` သည် descriptor parameters များကို အခိုင်အမာဖော်ပြထားသည်။
  မတော်တဆ ကွဲလွဲမှုကို ကာကွယ်ရန် `ChunkProfile::DEFAULT` ကို ယှဉ်ပြိုင်ပါ။
* `iroha app sorafs toolkit pack` နှင့် `sorafs_manifest_stub` မှထုတ်လုပ်သော Manifest များတွင် registry metadata ပါဝင်သည်။