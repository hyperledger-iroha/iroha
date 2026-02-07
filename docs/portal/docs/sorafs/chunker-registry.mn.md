---
lang: mn
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

::: Каноник эх сурвалжийг анхаарна уу
:::

## SoraFS Chunker профайлын бүртгэл (SF-2a)

SoraFS стек нь жижиг нэрийн зайтай бүртгэлээр дамжуулан ангилах үйлдлийг хэлэлцдэг.
Профайл бүр нь CDC-ийн тодорхойлогч параметрүүд, северийн мета өгөгдөл болон
манифест болон CAR архивт ашигладаг хүлээгдэж буй дижест/multicodec.

Профайл зохиогчидтой зөвлөлдөх хэрэгтэй
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
өмнө шаардлагатай мета өгөгдөл, баталгаажуулалтын хяналтын хуудас, саналын загвар
шинэ бичлэг оруулах. Засаглал өөрчлөлтийг баталсны дараа дараахыг дагаж мөрдөөрэй
[бүртгэлийг нэвтрүүлэх шалгах хуудас](./chunker-registry-rollout-checklist.md) болон
[Manifest playbook](./staging-manifest-playbook) сурталчлах
тайз болон үйлдвэрлэлээр дамжуулан бэхэлгээний .

### Профайл

| Нэрийн орон зай | Нэр | SemVer | Профайл ID | Мин (байт) | Зорилтот (байт) | Макс (байт) | Маск эвдэх | Multihash | Гадна нэр | Тэмдэглэл |
|----------|------|--------|------------|-------------|----------------|-------------|------------|-----------|---------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0"]` | SF-1 бэхэлгээнд ашигласан каноник профиль |

Бүртгэл нь `sorafs_manifest::chunker_registry` кодоор ажилладаг ([`chunker_registry_charter.md`](./chunker-registry-charter.md)-ээр зохицуулагддаг). Оруулга бүр
нь `ChunkerProfileDescriptor` хэлбэрээр илэрхийлэгдэнэ:

* `namespace` – холбогдох профайлын логик бүлэглэл (жишээ нь, `sorafs`).
* `name` – хүний ​​унших боломжтой профайл шошго (`sf1`, `sf1-fast`, …).
* `semver` – параметрийн багцад зориулсан семантик хувилбарын мөр.
* `profile` – бодит `ChunkProfile` (мин/зорилт/макс/маск).
* `multihash_code` – хэсэгчилсэн найруулга (`0x1f`) гаргахад ашигладаг олон талт хэрэглүүр
  SoraFS анхдагч хувьд).

Манифест нь профайлыг `ChunkingProfileV1`-ээр дамжуулан цуваа болгодог. Бүтцийн бүртгэл
түүхий CDC-ийн хажууд бүртгэлийн мета өгөгдөл (нэрийн зай, нэр, семвер)
параметрүүд болон дээр дурдсан нэрсийн жагсаалт. Хэрэглэгч эхлээд оролдох ёстой a
`profile_id`-ээр бүртгэлийн хайлт хийж, дараа нь шугамын параметрүүд рүү буцна.
үл мэдэгдэх ID гарч ирнэ. Бүртгэлийн дүрмийн дүрэм нь каноник бариулыг шаарддаг
(`namespace.name@semver`) нь `profile_aliases`-ийн анхны оруулга байх болно.

Бүртгэлийг багаж хэрэгсэлээс шалгахын тулд туслах CLI-г ажиллуулна уу:

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
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
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

Манифест stub нь ижил өгөгдлийг тусгадаг бөгөөд энэ нь дамжуулах хоолойд `--chunker-profile-id` сонголтыг скрипт хийхэд тохиромжтой. CLI-ууд хоёулаа каноник бариулын хэлбэрийг (`--profile=sorafs.sf1@1.0.0`) хүлээн авдаг тул скриптүүд нь хатуу кодлох тоон ID-аас зайлсхийх боломжтой:

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

`handle` талбар (`namespace.name@semver`) нь CLI-ийн хүлээн зөвшөөрсөн зүйлтэй таарч байна.
`--profile=…` нь автоматжуулалт руу шууд хуулж авахад аюулгүй болгодог.

### Хэлэлцээ хийж буй хуйвалдаанууд

Гарцууд болон үйлчлүүлэгчид үйлчилгээ үзүүлэгчийн сурталчилгаагаар дэмжигдсэн профайлыг сурталчилдаг:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implicit via registry)
    capabilities: [...]
}
```

Олон эх сурвалжийн хуваарийг `range` боломжоор дамжуулан зарладаг. The
CLI нь үүнийг `--capability=range[:streams]`-ээр хүлээн авдаг бөгөөд энд нэмэлт тоон
дагавар нь үйлчилгээ үзүүлэгчийн сонгосон муж-татаалтын зэрэгцлийг кодлодог (жишээлбэл,
`--capability=range:64` нь 64 урсгалтай төсвийг сурталчилдаг). орхигдуулсан тохиолдолд хэрэглэгчид
Зар сурталчилгааны өөр газар нийтлэгдсэн ерөнхий `max_streams` зөвлөмж рүү буцна уу.

CAR-ийн мэдээлэл хүсэх үед үйлчлүүлэгчид `Accept-Chunker` толгойн жагсаалтыг илгээх ёстой.
дэмжигдсэн `(namespace, name, semver)` tuple-уудыг давуу эрх олгох дарааллаар нь:

```
Accept-Chunker: sorafs.sf1;version=1.0.0
```

Гарцууд нь харилцан дэмжигдсэн профайлыг сонгоно (өгөгдмөл нь `sorafs.sf1@1.0.0`)
мөн `Content-Chunker` хариу толгойгоор дамжуулан шийдвэрийг тусгана. Манифестууд
Сонгосон профайлаа оруулснаар доод талын зангилаа нь хэсэгчилсэн байршлыг баталгаажуулах болно
HTTP хэлэлцээрт найдахгүйгээр.

### Тохиромжтой байдал

* `sorafs.sf1@1.0.0` профайл нь олон нийтийн эд хогшлын газрын зураг
  `fixtures/sorafs_chunker` ба дор бүртгүүлсэн корпораци
  `fuzz/sorafs_chunker`. Rust, Go, Node дээр төгсгөл хоорондын паритетыг хэрэгжүүлдэг
  өгсөн тестүүдээр дамжуулан.
* `chunker_registry::lookup_by_profile` нь тодорхойлогч параметрүүдийг баталж байна
  санамсаргүй зөрүүг хамгаалахын тулд `ChunkProfile::DEFAULT`-тай таарна.
* `iroha app sorafs toolkit pack` болон `sorafs_manifest_stub`-ийн гаргасан манифестууд нь бүртгэлийн мета өгөгдлийг агуулдаг.