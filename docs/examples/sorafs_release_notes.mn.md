---
lang: mn
direction: ltr
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 303a947895c10c7673b98e9187c3431c4012093c69d899252c121b53f9c48bb1
source_last_modified: "2026-01-05T09:28:11.823299+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS CLI & SDK — Хувилбарын тэмдэглэл (v0.1.0)

## Онцлох үйл явдал
- `sorafs_cli` одоо савлагааны хоолойг бүхэлд нь боож байна (`car pack`, `manifest build`,
  `proof verify`, `manifest sign`, `manifest verify-signature`) тул CI гүйгчид дараахыг дууддаг.
  захиалгат туслагчийн оронд ганц хоёртын. Түлхүүргүй гарын үсэг зурах шинэ урсгал нь өгөгдмөл
  `SIGSTORE_ID_TOKEN`, GitHub Үйлдлүүд OIDC үйлчилгээ үзүүлэгчдийг ойлгож, тодорхойлогч ялгаруулдаг
  гарын үсгийн багцын хажууд хураангуй JSON.
- Олон эх сурвалжаас авах * онооны самбар * нь `sorafs_car`-ийн нэг хэсэг болгон илгээгддэг: энэ нь хэвийн болж байна
  үйлчилгээ үзүүлэгчийн телеметр, чадварын шийтгэлийг хэрэгжүүлэх, JSON/Norito тайлангуудыг үргэлжлүүлэх, мөн
  Оркестрийн симуляторыг (`sorafs_fetch`) хуваалцсан бүртгэлийн бариулаар дамжуулдаг.
  `fixtures/sorafs_manifest/ci_sample/`-ийн дагуу бэхэлгээ нь детерминистикийг харуулж байна
  CI/CD нь ялгаатай байхаар хүлээгдэж буй оролт, гаралт.
- Хувилбарын автоматжуулалтыг `ci/check_sorafs_cli_release.sh` болон кодчилсон
  `scripts/release_sorafs_cli.sh`. Одоо хувилбар бүр нь манифест багцыг архивлаж байна,
  гарын үсэг, `manifest.sign/verify` хураангуй, онооны самбарын агшин зураг нь засаглал
  Шүүгчид дамжуулах хоолойг дахин ажиллуулахгүйгээр олдворуудыг хянах боломжтой.

## Шинэчлэх алхамууд
1. Ажлын талбартаа зэрэгцүүлсэн хайрцгийг шинэчилнэ үү:
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. Fmt/clippy/туршилтын хамрах хүрээг баталгаажуулахын тулд суллах хаалгыг дотооддоо (эсвэл CI-д) дахин ажиллуулна уу:
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. Гарын үсэг зурсан олдворууд болон хураангуйг тохируулсан тохиргоогоор сэргээнэ үү:
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   Хэрэв байгаа бол шинэчилсэн багц/баталгааг `fixtures/sorafs_manifest/ci_sample/` руу хуулна уу
   каноник бэхэлгээний шинэчлэлтүүдийг гаргах.

## Баталгаажуулалт
- Суллах хаалга: `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (Хаалга амжилттай болсны дараа `git rev-parse HEAD`).
- `ci/check_sorafs_cli_release.sh` гаралт: архивлагдсан
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log` (хувилбарын багцад хавсаргасан).
- Манифест багц хураамж: `SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  (`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`).
- Баталгаажуулах хураангуй тойм: `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`).
- Манифест дижест (доорх баталгаажуулалтын хөндлөн шалгалтын хувьд):
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  (`manifest.sign.summary.json`-аас).

## Операторуудад зориулсан тэмдэглэл
- Torii гарц нь одоо `X-Sora-Chunk-Range` чадамжийн толгой хэсгийг хэрэгжүүлж байна. Шинэчлэх
  зөвшөөрөгдсөн жагсаалтууд, ингэснээр шинэ урсгалын жетон хамрах хүрээг танилцуулж буй үйлчлүүлэгчид элсэх болно; хуучин токенууд
  хүрээгүй бол нэхэмжлэлийг хязгаарлах болно.
- `scripts/sorafs_gateway_self_cert.sh` нь манифест баталгаажуулалтыг нэгтгэдэг. Гүйх үед
  өөрийгөө баталгаажуулах оосор, шинээр үүсгэсэн манифест багцыг боодол нь өгөх боломжтой
  signature drift дээр хурдан бүтэлгүйтэх.
- Телеметрийн хяналтын самбарууд шинэ онооны самбарын экспортыг (`scoreboard.json`) оруулах ёстой.
  үйлчилгээ үзүүлэгчийн шаардлага, жингийн хуваарилалт, татгалзсан шалтгааныг нэгтгэх.
- Дөрвөн каноник хураангуйг танилцуулах бүрт архивлах:
  `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`,
  `manifest.verify.summary.json`. Засаглалын тасалбарууд нь эдгээр яг файлуудын хугацаанд иш татдаг
  зөвшөөрөл.

## Талархал
- Хадгалах баг — төгсгөл хоорондын CLI нэгтгэх, хэсэгчилсэн төлөвлөгөө боловсруулагч, онооны самбар
  телеметрийн сантехник.
- Багажны ажлын хэсэг — дамжуулах хоолой (`ci/check_sorafs_cli_release.sh`,
  `scripts/release_sorafs_cli.sh`) болон тодорхойлогч бэхэлгээний багц.
- Gateway Operations — чадавхийг шалгах, урсгалын токен бодлогыг хянаж, шинэчилсэн
  өөрийгөө баталгаажуулах тоглоомын номууд.