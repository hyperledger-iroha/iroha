---
lang: am
direction: ltr
source: docs/source/docker_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a5ac4be3d387269898112d465ec404490f67c6c2b9267c0a0781d0de70cf783d
source_last_modified: "2025-12-29T18:16:35.951567+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Docker ገንቢ ምስል

ይህ መያዣ በ`Dockerfile.build` ውስጥ ይገለጻል እና ሁሉንም የመሳሪያ ሰንሰለት ይጠቀለላል
ለ CI እና ለአካባቢያዊ ልቀት ግንባታዎች የሚያስፈልጉ ጥገኝነቶች። ምስሉ አሁን እንደ ሀ
ሥር ያልሆነ ተጠቃሚ በነባሪ፣ ስለዚህ Git ክወናዎች ከአርክ ሊኑክስ ጋር መስራታቸውን ቀጥለዋል።
የ`libgit2` ጥቅል ወደ አለምአቀፉ `safe.directory` መፍትሄ ሳይጠቀም።

## ክርክሮችን ይገንቡ

- `BUILDER_USER` - የመግቢያ ስም በመያዣው ውስጥ ተፈጠረ (ነባሪ፡ `iroha`)።
- `BUILDER_UID` - የቁጥር ተጠቃሚ መታወቂያ (ነባሪ፡ `1000`)።
- `BUILDER_GID` - ዋና የቡድን መታወቂያ (ነባሪ፡ `1000`)።

የስራ ቦታውን ከአስተናጋጅዎ ላይ ሲሰቅሉ የሚዛመዱ የUID/GID እሴቶችን ይለፉ
የተፈጠሩ ቅርሶች ሊጻፉ ይችላሉ፡-

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

የመሳሪያ ሰንሰለት ማውጫዎች (`/usr/local/rustup`፣ `/usr/local/cargo`፣ `/opt/poetry`)
የካርጎ፣ ሩትስፕ እና የግጥም ትዕዛዞች ሙሉ በሙሉ እንዲቆዩ በተዋቀረው ተጠቃሚ የተያዙ ናቸው።
መያዣው የስር መብቶችን ከጣለ በኋላ ይሠራል።

## ሩጫ ይገነባል።

የስራ ቦታዎን በሚጠሩበት ጊዜ ከ `/workspace` (መያዣው `WORKDIR`) ጋር ያያይዙት።
ምስል. ምሳሌ፡-

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

ምስሉ የ`docker` ቡድን አባልነትን ያቆያል ስለዚህ Docker ትዕዛዞችን (ለምሳሌ፦
`docker buildx bake`) አስተናጋጁን PID ለሚሰቅሉ ለ CI የስራ ፍሰቶች ዝግጁ ሆኖ ይቆያል
እና ሶኬት. ለአካባቢዎ እንደ አስፈላጊነቱ የቡድን ካርታዎችን ያስተካክሉ።

## Iroha 2 vs Iroha 3 ቅርሶች

የስራ ቦታ አሁን ግጭቶችን ለማስወገድ በየልቀት መስመር የተለያዩ ሁለትዮሾችን ይለቃል፡
`iroha3`/`iroha3d` (ነባሪ) እና `iroha2`/`iroha2d` (Iroha 2)። ረዳቶቹን ይጠቀሙ
ተፈላጊውን ጥንድ ማምረት;

- `make build` (ወይም `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3`) ለ Iroha 3
- `make build-i2` (ወይም `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`) ለ Iroha 2

መራጩ የባህሪ ስብስቦችን (`telemetry` + `schema-endpoint` እና
መስመር-ተኮር `build-i{2,3}` ባንዲራ) ስለዚህ Iroha 2 ግንባታዎች በድንገት ማንሳት አይችሉም
Iroha 3-ብቻ ነባሪዎች።

በ`scripts/build_release_bundle.sh` በኩል የተገነቡ ጥቅሎችን ይልቀቁ ትክክለኛውን ሁለትዮሽ ይምረጡ
`--profile` ወደ `iroha2` ወይም `iroha3` ሲዋቀር በራስ ሰር ስሞች።