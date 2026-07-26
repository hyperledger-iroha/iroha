---
lang: hy
direction: ltr
source: docs/source/docker_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a5ac4be3d387269898112d465ec404490f67c6c2b9267c0a0781d0de70cf783d
source_last_modified: "2025-12-29T18:16:35.951567+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Docker Builder Image

Այս բեռնարկղը սահմանված է `Dockerfile.build`-ում և միավորում է բոլոր գործիքների շղթան
CI-ի և տեղական թողարկման կառուցումների համար պահանջվող կախվածությունները: Պատկերն այժմ աշխատում է որպես a
Լռելյայնորեն ոչ արմատային օգտվող, այնպես որ Git-ի գործողությունները շարունակում են աշխատել Arch Linux-ի հետ
`libgit2` փաթեթ՝ առանց `safe.directory` գլոբալ լուծումին դիմելու:

## Ստեղծեք փաստարկներ

- `BUILDER_USER` – մուտքի անուն, որը ստեղծվել է կոնտեյների ներսում (կանխադրված՝ `iroha`):
- `BUILDER_UID` – օգտագործողի թվային id (լռելյայն՝ `1000`):
- `BUILDER_GID` – հիմնական խմբի ID (լռելյայն՝ `1000`):

Երբ դուք տեղադրում եք աշխատանքային տարածքը ձեր հոսթից, փոխանցեք համապատասխան UID/GID արժեքներ
Ստեղծված արտեֆակտները մնում են գրավոր.

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

Գործիքների շղթայի գրացուցակներ (`/usr/local/rustup`, `/usr/local/cargo`, `/opt/poetry`)
պատկանում են կազմաձևված օգտագործողին, այնպես որ Cargo, rustup և Poetry հրամանները մնում են ամբողջությամբ
ֆունկցիոնալ, երբ բեռնարկղը հրաժարվում է արմատային արտոնություններից:

## Վազում կառուցումներ

Կցեք ձեր աշխատանքային տարածքը `/workspace`-ին (կոնտեյներ `WORKDIR`), երբ կանչեք
պատկեր. Օրինակ՝

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

Պատկերը պահպանում է `docker` խմբի անդամակցությունը, այսպես տեղադրված Docker հրամանները (օրինակ.
`docker buildx bake`) մնում են հասանելի CI աշխատանքային հոսքերի համար, որոնք տեղադրում են հյուրընկալող PID-ը
և վարդակից: Կարգավորեք խմբային քարտեզագրումները՝ ըստ անհրաժեշտության ձեր միջավայրի համար:

## Iroha 2 vs Iroha 3 արտեֆակտ

Աշխատանքային տարածքն այժմ թողարկում է առանձին երկուականներ յուրաքանչյուր թողարկման տողում՝ բախումներից խուսափելու համար.
`iroha3`/`iroha3d` (կանխադրված) և `iroha2`/`iroha2d` (Iroha 2): Օգտագործեք օգնականները
արտադրել ցանկալի զույգը.

- `make build` (կամ `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3`) Iroha 3-ի համար
- `make build-i2` (կամ `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`) Iroha 2-ի համար

Ընտրիչը ամրացնում է առանձնահատկությունների հավաքածուները (`telemetry` + `schema-endpoint` գումարած
գծի հատուկ `build-i{2,3}` դրոշակ), ուստի Iroha 2 կառուցումները չեն կարող պատահաբար վերցնել
Iroha 3-միայն լռելյայն:

Թողարկեք `scripts/build_release_bundle.sh`-ի միջոցով ստեղծված փաթեթները, ընտրեք ճիշտ երկուականը
անվանում է ավտոմատ կերպով, երբ `--profile`-ը դրված է `iroha2` կամ `iroha3`: