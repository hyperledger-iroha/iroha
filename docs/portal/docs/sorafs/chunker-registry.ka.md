---
lang: ka
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

:::შენიშვნა კანონიკური წყარო
:::

## SoraFS Chunker პროფილის რეესტრი (SF-2a)

SoraFS სტეკი მოლაპარაკებებს აწარმოებს დაქუცმაცებულ ქცევას პატარა, სახელების სივრცის რეესტრის მეშვეობით.
თითოეული პროფილი ანიჭებს განმსაზღვრელ CDC პარამეტრებს, სემვერის მეტამონაცემებს და
მოსალოდნელი დაიჯესტი/მულტიკოდეკი გამოიყენება მანიფესტებსა და CAR არქივებში.

პროფილის ავტორებმა უნდა გაიარონ კონსულტაცია
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
ადრე საჭირო მეტამონაცემებისთვის, ვალიდაციის საკონტროლო სიისა და წინადადების შაბლონისთვის
ახალი ჩანაწერების წარდგენა. მას შემდეგ, რაც მმართველობა დაამტკიცებს ცვლილებას, მიჰყევით
[რეესტრის გაშვების საკონტროლო სია] (./chunker-registry-rollout-checklist.md) და
[მანიფესტის სათამაშო წიგნის დადგმა](./staging-manifest-playbook) პოპულარიზაციისთვის
მოწყობილობები დადგმისა და წარმოების გზით.

### პროფილები

| სახელთა სივრცე | სახელი | SemVer | პროფილის ID | მინ (ბაიტი) | სამიზნე (ბაიტი) | მაქს (ბაიტი) | დაარღვიე ნიღაბი | Multihash | მეტსახელები | შენიშვნები |
|-----------|------|-------|-----------|------------|--------- -------|-------------|-----------|-----------|--------|------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0"]` | კანონიკური პროფილი, რომელიც გამოიყენება SF-1 სმარტფონებში |

რეესტრი მუშაობს კოდით, როგორც `sorafs_manifest::chunker_registry` (იმართება [`chunker_registry_charter.md`](./chunker-registry-charter.md)). თითოეული ჩანაწერი
გამოიხატება როგორც `ChunkerProfileDescriptor`:

* `namespace` - დაკავშირებული პროფილების ლოგიკური დაჯგუფება (მაგ., `sorafs`).
* `name` - ადამიანის მიერ წასაკითხი პროფილის ეტიკეტი (`sf1`, `sf1-fast`, ...).
* `semver` – სემანტიკური ვერსიის სტრიქონი პარამეტრების ნაკრებისთვის.
* `profile` – ფაქტობრივი `ChunkProfile` (წთ/სამიზნე/მაქს/ნიღაბი).
* `multihash_code` - მულტიჰაში, რომელიც გამოიყენება ნაწილაკების დისჯესტების წარმოებისას (`0x1f`
  SoraFS ნაგულისხმევად).

მანიფესტი აწარმოებს პროფილების სერიალს `ChunkingProfileV1`-ის საშუალებით. სტრუქტურის ჩანაწერები
რეესტრის მეტამონაცემები (სახელთა სივრცე, სახელი, სემვერი) ნედლეული CDC-სთან ერთად
პარამეტრები და ზემოთ ნაჩვენები ზედმეტსახელების სია. მომხმარებლებმა ჯერ უნდა სცადონ ა
რეესტრის მოძიება `profile_id`-ის მიერ და დაბრუნდება ჩაშენებულ პარამეტრებზე, როდესაც
უცნობი პირადობის მოწმობები გამოჩნდება. რეესტრის წესდების წესები მოითხოვს კანონიკურ სახელურს
(`namespace.name@semver`) იქნება პირველი ჩანაწერი `profile_aliases`-ში.

რეესტრის ინსტრუმენტებიდან შესამოწმებლად, გაუშვით დამხმარე CLI:

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
$ ტვირთის გაშვება -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
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

Manifest stub ასახავს იგივე მონაცემებს, რაც მოსახერხებელია მილსადენებში `--chunker-profile-id` შერჩევის სკრიპტირებისას. ორივე ბლოკის შენახვის CLI ასევე იღებს კანონიკურ სახელურ ფორმას (`--profile=sorafs.sf1@1.0.0`), ასე რომ, სკრიპტებმა თავიდან აიცილონ მყარი კოდირების რიცხვითი ID-ები:

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

`handle` ველი (`namespace.name@semver`) ემთხვევა იმას, რასაც CLI იღებს მეშვეობით
`--profile=…`, რაც უსაფრთხოს ხდის პირდაპირ ავტომატიზაციაში კოპირებას.

### მოლაპარაკება ჩუნკერები

კარიბჭეები და კლიენტები აქვეყნებენ მხარდაჭერილ პროფილებს პროვაიდერის რეკლამების საშუალებით:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implicit via registry)
    capabilities: [...]
}
```

მრავალ წყაროს ბლოკის დაგეგმვა გამოცხადებულია `range` შესაძლებლობით. The
CLI იღებს მას `--capability=range[:streams]`-ით, სადაც არის სურვილისამებრ ციფრული
სუფიქსი დაშიფვრავს პროვაიდერის რჩეულ დიაპაზონის მოპოვების კონკურენტულობას (მაგალითად,
`--capability=range:64` აქვეყნებს 64 ნაკადის ბიუჯეტს). როდესაც გამოტოვებულია, მომხმარებლები
დავუბრუნდეთ ზოგად `max_streams` მინიშნებას, რომელიც გამოქვეყნდა სხვაგან რეკლამაში.

CAR მონაცემების მოთხოვნისას კლიენტებმა უნდა გამოაგზავნონ `Accept-Chunker` სათაურის სია
მხარდაჭერილი `(namespace, name, semver)` ტოპები უპირატესი თანმიმდევრობით:

```
Accept-Chunker: sorafs.sf1;version=1.0.0
```

კარიბჭეები ირჩევენ ორმხრივ მხარდაჭერილ პროფილს (ნაგულისხმევად `sorafs.sf1@1.0.0`)
და აისახეთ გადაწყვეტილება `Content-Chunker` პასუხის სათაურის მეშვეობით. გამოხატავს
ჩადეთ არჩეული პროფილი, რათა ქვედა დინების კვანძებმა შეძლონ ბლოკის განლაგების დადასტურება
HTTP მოლაპარაკებაზე დაყრდნობის გარეშე.

### შესაბამისობა

* `sorafs.sf1@1.0.0` პროფილი ასახავს საჯარო მოწყობილობებს
  `fixtures/sorafs_chunker` და კორპუსები, რომლებიც რეგისტრირებულია ქვეშ
  `fuzz/sorafs_chunker`. ბოლოდან ბოლომდე პარიტეტი ხორციელდება Rust, Go და Node-ში
  მოწოდებული ტესტების საშუალებით.
* `chunker_registry::lookup_by_profile` ამტკიცებს, რომ აღწერის პარამეტრები
  ემთხვევა `ChunkProfile::DEFAULT` შემთხვევითი დივერგენციის დასაცავად.
* `iroha app sorafs toolkit pack` და `sorafs_manifest_stub` მიერ წარმოებული მანიფესტები მოიცავს რეესტრის მეტამონაცემებს.