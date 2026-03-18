---
lang: uz
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

::: Eslatma Kanonik manba
:::

## SoraFS Chunker profil registri (SF-2a)

SoraFS stek kichik, nomlar oralig'i bo'lgan registr orqali bo'linish xatti-harakatlarini muhokama qiladi.
Har bir profil deterministik CDC parametrlarini, semver metama'lumotlarini va
manifestlar va CAR arxivlarida ishlatiladigan kutilgan dayjest/multikodek.

Profil mualliflari bilan maslahatlashish kerak
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
zarur metadata, tekshirish roʻyxati va taklif shablonidan oldin
yangi yozuvlarni yuborish. Boshqaruv o'zgartirishni ma'qullagandan so'ng, amal qiling
[ro'yxatga olish kitobini ochishni tekshirish ro'yxati](./chunker-registry-rollout-checklist.md) va
[staging manifest playbook](./staging-manifest-playbook) targ'ib qilish
dastgohlarni sahnalashtirish va ishlab chiqarish orqali.

### profillar

| Ismlar maydoni | Ism | SemVer | Profil ID | Min (bayt) | Maqsad (baytlar) | Maks (bayt) | Break niqob | Multihash | Taxalluslar | Eslatmalar |
|----------|------|--------|------------|-------------|----------------|-------------|------------|-----------|---------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0"]` | SF-1 armaturalarida ishlatiladigan kanonik profil |

Ro'yxatga olish kitobi `sorafs_manifest::chunker_registry` kodida yashaydi ([`chunker_registry_charter.md`] (./chunker-registry-charter.md) tomonidan boshqariladi). Har bir kirish
`ChunkerProfileDescriptor` sifatida ifodalanadi:

* `namespace` - tegishli profillarni mantiqiy guruhlash (masalan, `sorafs`).
* `name` - odam o'qiy oladigan profil yorlig'i (`sf1`, `sf1-fast`, …).
* `semver` - parametrlar to'plami uchun semantik versiya qatori.
* `profile` - haqiqiy `ChunkProfile` (min/maqsad/maks/niqob).
* `multihash_code` – parcha dayjestlarni ishlab chiqarishda ishlatiladigan multihash (`0x1f`)
  standart SoraFS uchun).

Manifest profillarni `ChunkingProfileV1` orqali seriyalashtiradi. Struktura yozuvlari
xom CDC bilan birga registr metama'lumotlari (nom maydoni, nom, semver)
parametrlar va yuqorida ko'rsatilgan taxalluslar ro'yxati. Iste'molchilar birinchi navbatda a
`profile_id` tomonidan ro'yxatga olish kitobini qidiring va keyin inline parametrlariga qayting.
noma'lum identifikatorlar paydo bo'ladi. Registr nizomi qoidalari kanonik tutqichni talab qiladi
(`namespace.name@semver`) `profile_aliases` da birinchi yozuv bo'lishi.

Ro'yxatga olish kitobini asboblardan tekshirish uchun CLI yordamchisini ishga tushiring:

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
$ yuk tashish -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

You can select a profile by numeric id (`--profile-id=1`) or by registry handle
(`--profile=sorafs.sf1@1.0.0`); the handle form is convenient for scripts that
thread namespace/name/semver directly from governance metadata.

Use `--promote-profile=<handle>` to emit a JSON metadata block (including all
registered aliases) that can be pasted into `chunker_registry_data.rs` when
promoting a new default profile:

```
$ yuk tashish -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

The main report (and optional proof file) include the root digest, the sampled
leaf bytes (hex-encoded), and the segment/chunk sibling digests so verifiers can
rehash the 64 KiB/4 KiB layers against the `por_root_hex` value.

To validate an existing proof against a payload, pass the path via
`--por-proof-verify` (the CLI adds `"por_proof_verified": true` when the witness
matches the computed root):

```
$ yuk tashish -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

For batch sampling, use `--por-sample=<count>` and optionally provide a seed/
output path. The CLI guarantees deterministic ordering (`splitmix64` seeded)
and will transparently truncate when the request exceeds the available leaves:

```
$ yuk tashish -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json

Manifest stub bir xil ma'lumotlarni aks ettiradi, bu quvur liniyalarida `--chunker-profile-id` tanlovini skript qilishda qulaydir. Ikkala do'kon CLI ham kanonik tutqich shaklini (`--profile=sorafs.sf1@1.0.0`) qabul qiladi, shuning uchun yaratish skriptlari qattiq kodlangan raqamli identifikatorlardan qochishi mumkin:

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

`handle` maydoni (`namespace.name@semver`) CLI tomonidan qabul qilingan narsaga mos keladi.
`--profile=…`, to'g'ridan-to'g'ri avtomatlashtirishga nusxalashni xavfsiz qiladi.

### Muzokaralar

Gateways va mijozlar qo'llab-quvvatlanadigan profillarni provayder reklamalari orqali reklama qiladilar:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implicit via registry)
    capabilities: [...]
}
```

Ko'p manbali bo'laklarni rejalashtirish `range` qobiliyati orqali e'lon qilinadi. The
CLI uni `--capability=range[:streams]` bilan qabul qiladi, bu erda ixtiyoriy raqamli
suffiks provayderning afzal ko'rgan diapazonni qabul qilish moslashuvini kodlaydi (masalan,
`--capability=range:64` 64 oqimli byudjetni reklama qiladi). O'tkazib yuborilganda, iste'molchilar
Reklamaning boshqa joyida chop etilgan umumiy `max_streams` maslahatiga qayting.

CAR ma'lumotlarini so'rashda mijozlar `Accept-Chunker` sarlavhalari ro'yxatini yuborishlari kerak
qo'llab-quvvatlanadigan `(namespace, name, semver)` kortejlari afzal tartibda:

```
Accept-Chunker: sorafs.sf1;version=1.0.0
```

Shlyuzlar o'zaro qo'llab-quvvatlanadigan profilni tanlaydi (standart `sorafs.sf1@1.0.0`)
va `Content-Chunker` javob sarlavhasi orqali qarorni aks ettiring. Manifestlar
quyi oqim tugunlari bo'lak tartibini tasdiqlashi uchun tanlangan profilni joylashtiring
HTTP muzokaralariga tayanmasdan.

### Muvofiqlik

* `sorafs.sf1@1.0.0` profili davlat anjomlariga mos keladi
  `fixtures/sorafs_chunker` va ostida ro'yxatdan o'tgan korporatsiya
  `fuzz/sorafs_chunker`. End-to-end pariteti Rust, Go va Node da amalga oshiriladi
  taqdim etilgan testlar orqali.
* `chunker_registry::lookup_by_profile` ta'kidlaydiki, deskriptor parametrlari
  tasodifiy ajralishdan himoya qilish uchun `ChunkProfile::DEFAULT` bilan moslang.
* `iroha app sorafs toolkit pack` va `sorafs_manifest_stub` tomonidan ishlab chiqarilgan manifestlar registr metamaʼlumotlarini oʻz ichiga oladi.