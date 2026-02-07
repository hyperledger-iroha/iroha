---
lang: az
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

:::Qeyd Kanonik Mənbə
:::

## SoraFS Chunker Profil Reyestri (SF-2a)

SoraFS yığını kiçik, ad boşluqlu reyestr vasitəsilə parçalanma davranışını müzakirə edir.
Hər bir profil deterministik CDC parametrlərini, semver metadatasını və
manifestlərdə və CAR arxivlərində istifadə edilən gözlənilən həzm/multikodec.

Profil müəllifləri məsləhətləşməlidir
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
əvvəl tələb olunan metadata, doğrulama yoxlama siyahısı və təklif şablonu üçün
yeni yazıların təqdim edilməsi. İdarəetmə dəyişikliyi təsdiqlədikdən sonra aşağıdakılara əməl edin
[reyestr təqdim edilməsinə nəzarət siyahısı](./chunker-registry-rollout-checklist.md) və
[təşkil etmək üçün manifest oyun kitabı](./staging-manifest-playbook).
quruluş və istehsal vasitəsilə qurğular.

### Profillər

| Ad sahəsi | Adı | SemVer | Profil ID | Min (bayt) | Hədəf (bayt) | Maks (bayt) | Break maskası | Multihash | Təxəllüs | Qeydlər |
|----------|------|--------|------------|-------------|----------------|-------------|------------|-----------|---------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0"]` | SF-1 qurğularında istifadə olunan kanonik profil |

Reyestr `sorafs_manifest::chunker_registry` ([`chunker_registry_charter.md`](./chunker-registry-charter.md) tərəfindən idarə olunur) kimi kodda yaşayır. Hər bir giriş
`ChunkerProfileDescriptor` ilə ifadə edilir:

* `namespace` – əlaqəli profillərin məntiqi qruplaşdırılması (məsələn, `sorafs`).
* `name` – insan tərəfindən oxuna bilən profil etiketi (`sf1`, `sf1-fast`, …).
* `semver` – parametr dəsti üçün semantik versiya sətri.
* `profile` – faktiki `ChunkProfile` (min/hədəf/maks/maska).
* `multihash_code` – yığın həzmləri (`0x1f`) istehsal edərkən istifadə olunan multihash
  SoraFS defolt üçün).

Manifest profilləri `ChunkingProfileV1` vasitəsilə seriallaşdırır. Struktur qeyd edir
xam CDC ilə yanaşı reyestr metadatası (ad sahəsi, ad, semver)
parametrlər və yuxarıda göstərilən ləqəb siyahısı. İstehlakçılar əvvəlcə cəhd etməlidirlər
`profile_id` tərəfindən reyestrdə axtarış edin və daxili parametrlərə qayıdın
naməlum identifikatorlar görünür. Reyestr nizamnamə qaydaları kanonik tutacaq tələb edir
(`namespace.name@semver`) `profile_aliases`-də ilk giriş olacaq.

Reyestrini alətlərdən yoxlamaq üçün CLI köməkçisini işə salın:

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
$ yük qaçışı -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

You can select a profile by numeric id (`--profile-id=1`) or by registry handle
(`--profile=sorafs.sf1@1.0.0`); the handle form is convenient for scripts that
thread namespace/name/semver directly from governance metadata.

Use `--promote-profile=<handle>` to emit a JSON metadata block (including all
registered aliases) that can be pasted into `chunker_registry_data.rs` when
promoting a new default profile:

```
$ yük qaçışı -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

The main report (and optional proof file) include the root digest, the sampled
leaf bytes (hex-encoded), and the segment/chunk sibling digests so verifiers can
rehash the 64 KiB/4 KiB layers against the `por_root_hex` value.

To validate an existing proof against a payload, pass the path via
`--por-proof-verify` (the CLI adds `"por_proof_verified": true` when the witness
matches the computed root):

```
$ yük qaçışı -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

For batch sampling, use `--por-sample=<count>` and optionally provide a seed/
output path. The CLI guarantees deterministic ordering (`splitmix64` seeded)
and will transparently truncate when the request exceeds the available leaves:

```
$ yük qaçışı -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json

Manifest stub eyni məlumatları əks etdirir, bu da boru kəmərlərində `--chunker-profile-id` seçimini skript edərkən əlverişlidir. Hər iki yığın mağazası CLI-ləri həm də kanonik tutacaq formasını (`--profile=sorafs.sf1@1.0.0`) qəbul edir, beləliklə qurmaq skriptləri sərt kodlaşdırılan rəqəmli ID-lərdən qaça bilər:

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

`handle` sahəsi (`namespace.name@semver`) CLI-lərin qəbul etdiyinə uyğun gəlir
`--profile=…`, birbaşa avtomatlaşdırmaya kopyalamağı təhlükəsiz edir.

### Danışıqlar

Şlüzlər və müştərilər provayder reklamları vasitəsilə dəstəklənən profilləri reklam edirlər:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implicit via registry)
    capabilities: [...]
}
```

Çox mənbəli yığın planlaşdırması `range` qabiliyyəti vasitəsilə elan edilir. The
CLI onu `--capability=range[:streams]` ilə qəbul edir, burada isteğe bağlı rəqəmli
şəkilçi provayderin üstünlük verdiyi diapazon-gəlmə paralelliyini kodlayır (məsələn,
`--capability=range:64` 64 axın büdcəsini reklam edir). Buraxıldıqda, istehlakçılar
reklamın başqa yerində dərc olunmuş ümumi `max_streams` göstərişinə qayıdın.

CAR məlumatlarını tələb edərkən, müştərilər `Accept-Chunker` başlıq siyahısını göndərməlidirlər
üstünlük sırasına görə dəstəklənən `(namespace, name, semver)` dəstləri:

```
Accept-Chunker: sorafs.sf1;version=1.0.0
```

Şlüzlər qarşılıqlı dəstəklənən profili seçir (defolt olaraq `sorafs.sf1@1.0.0`)
və `Content-Chunker` cavab başlığı vasitəsilə qərarı əks etdirin. Təzahür edir
seçilmiş profili daxil edin ki, aşağı axın qovşaqları yığın düzümünü təsdiq edə bilsin
HTTP danışıqlarına etibar etmədən.

### Uyğunluq

* `sorafs.sf1@1.0.0` profili ictimai qurğulara xəritə verir
  `fixtures/sorafs_chunker` və altında qeydiyyatdan keçmiş korporasiya
  `fuzz/sorafs_chunker`. Rust, Go və Node proqramlarında uçdan-uca paritet həyata keçirilir
  təqdim olunan testlər vasitəsilə.
* `chunker_registry::lookup_by_profile` təsdiq edir ki, deskriptor parametrləri
  təsadüfi divergensiyanı qorumaq üçün `ChunkProfile::DEFAULT` uyğunlaşdırın.
* `iroha app sorafs toolkit pack` və `sorafs_manifest_stub` tərəfindən hazırlanmış manifestlərə reyestr metadata daxildir.