---
lang: hy
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

:::note Կանոնական աղբյուր
:::

## SoraFS Chunker պրոֆիլների գրանցամատյան (SF-2a)

SoraFS կույտը համաձայնեցնում է բեկորային վարքագիծը փոքր, անվանատարածքով ռեեստրի միջոցով:
Յուրաքանչյուր պրոֆիլ հատկացնում է դետերմինիստական CDC պարամետրեր, semver մետատվյալներ և
ակնկալվող դիջեստ/մուլտիկոդեկ, որն օգտագործվում է մանիֆեստներում և CAR արխիվներում:

Պրոֆիլների հեղինակները պետք է խորհրդակցեն
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
նախապես անհրաժեշտ մետատվյալների, վավերացման ստուգաթերթի և առաջարկի ձևանմուշի համար
նոր գրառումներ ներկայացնելը. Երբ կառավարությունը հաստատի փոփոխությունը, հետևեք
[ռեգիստրի թողարկման ստուգաթերթ] (./chunker-registry-rollout-checklist.md) և
[բեմականացման մանիֆեստի խաղագիրք](./staging-manifest-playbook) խթանելու համար
հարմարանքները բեմադրության և արտադրության միջոցով:

### Պրոֆիլներ

| Անվանատարածք | Անունը | ՍեմՎեր | Անձնագիր ID | Min (բայթ) | Թիրախ (բայթ) | Առավելագույն (բայթ) | Կոտրել դիմակ | Մուլտիհեշ | Անանուններ | Ծանոթագրություններ |
|-----------|------|-------|------------|------------|--------- -------|---------------------------|-----------|---------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0"]` | Կանոնական պրոֆիլ, որն օգտագործվում է SF-1 հարմարանքներում |

Ռեեստրն աշխատում է `sorafs_manifest::chunker_registry` կոդով (կառավարվում է [`chunker_registry_charter.md`](./chunker-registry-charter.md) կողմից): Յուրաքանչյուր մուտք
արտահայտվում է որպես `ChunkerProfileDescriptor`՝

* `namespace` – հարակից պրոֆիլների տրամաբանական խմբավորում (օրինակ՝ `sorafs`):
* `name` – մարդու կողմից ընթեռնելի պրոֆիլի պիտակ (`sf1`, `sf1-fast`, …):
* `semver` – պարամետրերի հավաքածուի իմաստային տարբերակի տող:
* `profile` – իրական `ChunkProfile` (րոպե/թիրախ/առավելագույն/դիմակ):
* `multihash_code` – մուլտիհեշ, որն օգտագործվում է կտորների մարսողության ժամանակ (`0x1f`
  SoraFS կանխադրվածի համար):

Մանիֆեստը սերիականացնում է պրոֆիլները `ChunkingProfileV1`-ի միջոցով: Կառուցվածքը գրանցում է
ռեեստրի մետատվյալները (անունների տարածություն, անուն, սեմվեր) չմշակված CDC-ի հետ մեկտեղ
պարամետրերը և վերը նշված այլանունների ցանկը: Սպառողները նախ պետք է փորձեն ա
ռեեստրի որոնում `profile_id`-ի կողմից և ետ ընկնելով ներկառուցված պարամետրերին, երբ
հայտնվում են անհայտ ID-ներ: Ռեեստրի կանոնադրության կանոնները պահանջում են կանոնական բռնակ
(`namespace.name@semver`) լինել `profile_aliases`-ի առաջին մուտքը:

Ռեեստրը գործիքավորումից ստուգելու համար գործարկեք օգնական CLI.

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
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

You can select a profile by numeric id (`--profile-id=1`) or by registry handle
(`--profile=sorafs.sf1@1.0.0`); the handle form is convenient for scripts that
thread namespace/name/semver directly from governance metadata.

Use `--promote-profile=<handle>` to emit a JSON metadata block (including all
registered aliases) that can be pasted into `chunker_registry_data.rs` when
promoting a new default profile:

```
$ բեռնափոխադրումներ -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

The main report (and optional proof file) include the root digest, the sampled
leaf bytes (hex-encoded), and the segment/chunk sibling digests so verifiers can
rehash the 64 KiB/4 KiB layers against the `por_root_hex` value.

To validate an existing proof against a payload, pass the path via
`--por-proof-verify` (the CLI adds `"por_proof_verified": true` when the witness
matches the computed root):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

For batch sampling, use `--por-sample=<count>` and optionally provide a seed/
output path. The CLI guarantees deterministic ordering (`splitmix64` seeded)
and will transparently truncate when the request exceeds the available leaves:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json

Մանիֆեստի կոճակը արտացոլում է նույն տվյալները, ինչը հարմար է խողովակաշարերում `--chunker-profile-id` ընտրությունը գրելիս: Երկու կտոր պահեստային CLI-ներն էլ ընդունում են կանոնական բռնակի ձևը (`--profile=sorafs.sf1@1.0.0`), այնպես որ կառուցվող սկրիպտները կարող են խուսափել կոշտ կոդավորման թվային ID-ներից.

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

`handle` դաշտը (`namespace.name@semver`) համապատասխանում է նրան, ինչ CLI-ներն ընդունում են միջոցով
`--profile=…`, դարձնելով այն անվտանգ պատճենումը անմիջապես ավտոմատացման մեջ:

### Բանակցություններ Չունկերներ

Դարպասները և հաճախորդները գովազդում են աջակցվող պրոֆիլները մատակարարի գովազդների միջոցով.

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implicit via registry)
    capabilities: [...]
}
```

Բազմաղբյուրի կտորների պլանավորումը հայտարարվում է `range` հնարավորության միջոցով: Այն
CLI-ն այն ընդունում է `--capability=range[:streams]`-ով, որտեղ ընտրովի թվային է
վերջածանցը կոդավորում է մատակարարի նախընտրած միջակայքի առբերման համաժամանակությունը (օրինակ՝
`--capability=range:64`-ը գովազդում է 64 հոսքային բյուջե): Երբ բաց թողնված, սպառողներ
վերադառնանք `max_streams` ընդհանուր ակնարկին, որը հրապարակվել է գովազդի մեկ այլ վայրում:

Մեքենայի տվյալներ պահանջելիս հաճախորդները պետք է ուղարկեն `Accept-Chunker` վերնագրի ցուցակը
Աջակցված `(namespace, name, semver)` tuples ըստ նախապատվության կարգի.

```
Accept-Chunker: sorafs.sf1;version=1.0.0
```

Դարպասները ընտրում են փոխադարձ աջակցվող պրոֆիլ (կանխադրված՝ `sorafs.sf1@1.0.0`)
և արտացոլեք որոշումը `Content-Chunker` պատասխանի վերնագրի միջոցով: Դրսեւորվում է
Տեղադրեք ընտրված պրոֆիլը, որպեսզի ներքևի հանգույցները կարողանան վավերացնել կտորի դասավորությունը
առանց հենվելու HTTP բանակցությունների վրա:

### Համապատասխանություն

* `sorafs.sf1@1.0.0` պրոֆիլը քարտեզագրվում է հանրային հարմարանքներին
  `fixtures/sorafs_chunker` և ներքո գրանցված կորպուսները
  `fuzz/sorafs_chunker`. End-to-end հավասարությունը իրականացվում է Rust, Go և Node-ում
  տրամադրված թեստերի միջոցով:
* `chunker_registry::lookup_by_profile` պնդում է, որ նկարագրիչի պարամետրերը
  համընկնում է `ChunkProfile::DEFAULT`՝ պատահական շեղումը պաշտպանելու համար:
* `iroha app sorafs toolkit pack` և `sorafs_manifest_stub` կողմից արտադրված մանիֆեստները ներառում են ռեեստրի մետատվյալները: