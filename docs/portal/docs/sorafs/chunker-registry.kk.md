---
lang: kk
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

:::ескерту Канондық дереккөз
:::

## SoraFS Chunker профилінің тізілімі (SF-2a)

SoraFS стек шағын, аттар кеңістігі бар тізілім арқылы бөлшектеу әрекетін келіседі.
Әрбір профиль детерминирленген CDC параметрлерін, semver метадеректерін және
манифесттерде және CAR мұрағаттарында пайдаланылатын күтілетін дайджест/мультикодек.

Профиль авторларымен кеңесу керек
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
бұрын қажетті метадеректер, тексеруді тексеру тізімі және ұсыныс үлгісі үшін
жаңа жазбаларды жіберу. Басқару өзгерісті мақұлдағаннан кейін мынаны орындаңыз
[тізілімді шығаруды тексеру тізімі](./chunker-registry-rollout-checklist.md) және
[сахналық манифест ойын кітабы](./staging-manifest-playbook) жылжыту үшін
орнату және өндіру арқылы арматура.

### Профильдер

| Атау кеңістігі | Аты | SemVer | Профиль идентификаторы | Мин (байт) | Мақсат (байт) | Макс (байт) | Үзіліс маскасы | Мультихэш | Бүркеншік аттар | Ескертпелер |
|-----------|------|--------|------------|-------------|----------------|-------------|------------|-----------|---------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0"]` | SF-1 қондырғыларында қолданылатын канондық профиль |

Тізілім `sorafs_manifest::chunker_registry` ([`chunker_registry_charter.md`](./chunker-registry-charter.md) арқылы басқарылады) ретінде кодта тұрады. Әрбір жазба
`ChunkerProfileDescriptor` ретінде көрсетіледі:

* `namespace` – қатысты профильдердің логикалық топтастырылуы (мысалы, `sorafs`).
* `name` – адам оқи алатын профиль жапсырмасы (`sf1`, `sf1-fast`, …).
* `semver` – параметр жиынына арналған семантикалық нұсқа жолы.
* `profile` – нақты `ChunkProfile` (мин/мақсат/макс/маска).
* `multihash_code` – кесінді дайджесттерді жасау кезінде қолданылатын мультихэш (`0x1f`
  SoraFS әдепкі үшін).

Манифест профильдерді `ChunkingProfileV1` арқылы сериялайды. Құрылым жазады
шикі CDC қатарында тізілім метадеректері (аттар кеңістігі, атау, semver)
параметрлер мен жоғарыда көрсетілген бүркеншік ат тізімі. Тұтынушылар алдымен әрекет етуі керек
`profile_id` арқылы тізілімді іздеу және кірістірілген параметрлерге оралу
белгісіз идентификаторлар пайда болады. Тіркеу жарғысының ережелері канондық дескрипторды талап етеді
(`namespace.name@semver`) `profile_aliases` ішіндегі бірінші жазба болуы керек.

Құралдардан тізілімді тексеру үшін CLI көмекшісін іске қосыңыз:

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

Манифест түбіртегі бірдей деректерді көрсетеді, бұл конвейерлердегі `--chunker-profile-id` таңдау сценарийін жазу кезінде ыңғайлы. Екі жинақ дүкені CLI да канондық дескриптор пішінін (`--profile=sorafs.sf1@1.0.0`) қабылдайды, сондықтан құрастыру сценарийлері қатаң кодталатын сандық идентификаторларды болдырмайды:

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

`handle` өрісі (`namespace.name@semver`) CLI арқылы қабылдайтын нәрсеге сәйкес келеді.
`--profile=…`, бұл оны автоматтандыруға тікелей көшіруді қауіпсіз етеді.

### Келіссөздер жүргізу

Шлюздер мен клиенттер провайдер жарнамалары арқылы қолдау көрсетілетін профильдерді жарнамалайды:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implicit via registry)
    capabilities: [...]
}
```

Көп көзді бөлікті жоспарлау `range` мүмкіндігі арқылы жарияланады. The
CLI оны `--capability=range[:streams]` арқылы қабылдайды, мұнда қосымша сандық
жұрнақ провайдердің таңдаулы диапазонды алу параллелділігін кодтайды (мысалы,
`--capability=range:64` 64 ағындық бюджетті жарнамалайды). Өткізіп тастаған кезде тұтынушылар
Жарнаманың басқа жерінде жарияланған жалпы `max_streams` кеңесіне қайта оралыңыз.

CAR деректерін сұраған кезде, клиенттер `Accept-Chunker` тақырып тізімін жіберуі керек.
`(namespace, name, semver)` кортеждерін таңдау тәртібімен қолдайтын:

```
Accept-Chunker: sorafs.sf1;version=1.0.0
```

Шлюздер өзара қолдау көрсетілетін профильді таңдайды (әдепкі бойынша `sorafs.sf1@1.0.0`)
және `Content-Chunker` жауап тақырыбы арқылы шешімді көрсетіңіз. Манифесттер
таңдалған профильді ендіріңіз, осылайша төменгі ағындар түйіндердің орналасуын тексере алады
HTTP келіссөздеріне сүйенбестен.

### Сәйкестік

* `sorafs.sf1@1.0.0` профилі жалпыға ортақ құрылғыларға сәйкес келеді
  `fixtures/sorafs_chunker` және корпорациямен тіркелген
  `fuzz/sorafs_chunker`. Rust, Go және Node жүйелерінде ұшын-соңды теңдік орындалады
  берілген сынақтар арқылы.
* `chunker_registry::lookup_by_profile` дескриптор параметрлерін бекітеді
  кездейсоқ дивергенцияны сақтау үшін `ChunkProfile::DEFAULT` сәйкестендіріңіз.
* `iroha app sorafs toolkit pack` және `sorafs_manifest_stub` шығарған манифесттер тізілім метадеректерін қамтиды.