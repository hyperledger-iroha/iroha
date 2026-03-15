---
lang: mn
direction: ltr
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 39cbd5e448c8a868c50401c15466d7159cb08ad7be52dafb9e9dc66d5bba979d
source_last_modified: "2025-12-29T18:16:35.105044+00:00"
translation_last_reviewed: 2026-02-07
id: norito-rpc-adoption
title: Norito-RPC Adoption Schedule
sidebar_label: Norito-RPC adoption
description: Cross-SDK rollout plan, evidence checklist, and automation hooks for roadmap item NRPC-4.
translator: machine-google-reviewed
---

> Каноник төлөвлөлтийн тэмдэглэлүүд `docs/source/torii/norito_rpc_adoption_schedule.md`-д байдаг.  
> Энэхүү портал хуулбар нь SDK зохиогчид, операторууд болон хянагчдад зориулсан нэвтрүүлэх хүлээлтийг өөрчилдөг.

## Зорилтууд

- AND4-ийн үйлдвэрлэлийн сэлгэн залгахаас өмнө SDK (Rust CLI, Python, JavaScript, Swift, Android) бүрийг Norito-RPC хоёртын тээвэрт тохируулаарай.
-Засаглал нэвтрүүлэхэд аудит хийх боломжтой болохын тулд фазын хаалга, нотолгооны багц, телеметрийн дэгээг тодорхой байлгах.
- NRPC-4-ийн замын зураглалд заасан нийтлэг туслахуудын тусламжтайгаар бэхэлгээ, канарын нотлох баримтуудыг олж авахыг хялбар болго.

## Үе шат

| Үе шат | Цонх | Хамрах хүрээ | Гарах шалгуур |
|-------|--------|-------|---------------|
| **P0 – Лабораторийн паритет** | Q22025 | Rust CLI + Python утааны иж бүрдэлүүд нь CI-д `/v1/norito-rpc`-г ажиллуулдаг, JS туслах нь нэгжийн шалгалтыг давж, Android хуурамч морины хэрэгсэл давхар тээвэрлэлт хийдэг. | CI-д `python/iroha_python/scripts/run_norito_rpc_smoke.sh` ба `javascript/iroha_js/test/noritoRpcClient.test.js` ногоон; Android утас нь `./gradlew test` руу холбогдсон. |
| **P1 – SDK урьдчилан үзэх** | Q32025 | Хуваалцсан бэхэлгээний багцыг шалгасан, `scripts/run_norito_rpc_fixtures.sh --sdk <label>` бүртгэлүүд + `artifacts/norito_rpc/` дахь JSON, SDK дээжид илэрсэн нэмэлт Norito тээврийн тугуудыг бүртгэдэг. | Засварын манифест гарын үсэг зурсан, README шинэчлэлтүүд нь бүртгүүлэх хэрэглээг харуулж байна, Swift урьдчилан харах API нь IOS2 тугийн ард байдаг. |
| **P2 – Үзүүлэлт / AND4 урьдчилан үзэх** | Q12026 | Torii шатлалын сангууд нь Norito, Android AND4 урьдчилан үзэх үйлчлүүлэгчид болон Swift IOS2 паритын иж бүрдэлүүдийг хоёртын дамжуулалтаас илүүд үздэг, телеметрийн хяналтын самбар `dashboards/grafana/torii_norito_rpc_observability.json` дээр суурилдаг. | `docs/source/torii/norito_rpc_stage_reports.md` канарыг барьж, `scripts/telemetry/test_torii_norito_rpc_alerts.sh` дамжуулалт, Android хуурамч морины дахин тоглуулах нь амжилт/алдааны тохиолдлыг бүртгэдэг. |
| **P3 – Үйлдвэрлэлийн GA** | Q42026 | Norito нь бүх SDK-ийн анхдагч тээвэрлэлт болдог; JSON нь буцалтгүй тусламж хэвээр байна. Ажлын архивын паритын олдворуудыг шошго болгон гарга. | Rust/JS/Python/Swift/Android-д зориулсан Norito утааны гаралтын хяналтын хуудасны багцуудыг гаргах; Norito болон JSON алдааны түвшний SLO-уудын дохиоллын босго; `status.md` болон хувилбарын тэмдэглэлд GA нотолгоог иш татсан болно. |

## SDK нийлүүлэлт ба CI дэгээ

- **Rust CLI ба нэгдсэн бэхэлгээ** – `iroha_cli pipeline` утааны туршилтыг сунгаж, `cargo xtask norito-rpc-verify` газардсаны дараа Norito тээвэрлэлтийг албадан гаргана. `artifacts/norito_rpc/` дор олдворуудыг хадгалдаг `cargo test -p integration_tests -- norito_streaming` (лаборатори) болон `cargo xtask norito-rpc-verify` (үе шатлал/GA) бүхий хамгаалалт.
- **Python SDK** – ялгарах утааг (`python/iroha_python/scripts/release_smoke.sh`) анхдагчаар Norito RPC болгож, `run_norito_rpc_smoke.sh`-г CI нэвтрэх цэг болгон үлдээж, `python/iroha_python/README.md`-д паритет зохицуулалтыг баримтжуулна. CI зорилтот: `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`.
- **JavaScript SDK** – `NoritoRpcClient`-г тогтворжуулж, `toriiClientConfig.transport.preferred === "norito_rpc"` үед засаглал/асуулгад туслах ажилтнуудыг Norito гэж өгөгдмөл болгож, `javascript/iroha_js/recipes/` дээр төгсгөл хоорондын дээж авах боломжтой. CI нь нийтлэхээс өмнө `npm test` дээр нэмэх нь докержуулсан `npm run test:norito-rpc` ажлыг ажиллуулах ёстой; Provanance `javascript/iroha_js/artifacts/` дор Norito утааны бүртгэлийг байршуулдаг.
- **Swift SDK** – Norito гүүрний тээврийг IOS2 тугийн ард холбож, бэхэлгээний хэмжигдэхүүнийг тусгаж, Connect/Norito хосолсон багц нь `docs/source/sdk/swift/index.md`-д дурдсан Buildkite эгнээний дотор ажиллаж байгаа эсэхийг шалгаарай.
- **Android SDK** – AND4 урьдчилан үзэх үйлчлүүлэгчид болон хуурамч Torii утас нь `docs/source/sdk/android/networking.md`-д баримтжуулсан дахин оролдох/буцах телеметрийн хамт Norito-ийг ашигладаг. Морь нь `scripts/run_norito_rpc_fixtures.sh --sdk android`-ээр дамжуулан бусад SDK-уудтай бэхэлгээг хуваалцдаг.

## Нотлох баримт ба автоматжуулалт

- `scripts/run_norito_rpc_fixtures.sh` нь `cargo xtask norito-rpc-verify`-г ороож, stdout/stderr-г барьж, `fixtures.<sdk>.summary.json` ялгаруулдаг тул SDK эзэмшигчид `status.md`-д хавсаргах тодорхой олдвортой байдаг. CI багцуудыг эмх цэгцтэй байлгахын тулд `--sdk <label>` болон `--out artifacts/norito_rpc/<stamp>/`-г ашиглана уу.
- `cargo xtask norito-rpc-verify` схемийн хэш паритыг (`fixtures/norito_rpc/schema_hashes.json`) хэрэгжүүлдэг ба Torii `X-Iroha-Error-Code: schema_mismatch`-г буцаавал амжилтгүй болно. Бүтэлгүйтэл бүрийг дибаг хийхэд зориулж JSON нөөц хураагууртай хослуул.
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` болон `dashboards/grafana/torii_norito_rpc_observability.json` нь NRPC-2-ын дохиоллын гэрээг тодорхойлдог. Хяналтын самбарын засвар бүрийн дараа скриптийг ажиллуулж, `promtool` гаралтыг канарын багцад хадгална.
- `docs/source/runbooks/torii_norito_rpc_canary.md` нь үе шат, үйлдвэрлэлийн дасгалуудыг тайлбарласан; бэхэлгээний хэш эсвэл дохиоллын хаалга өөрчлөгдөх бүрт үүнийг шинэчил.

## Шүүгчийн хяналтын хуудас

NRPC-4 чухал үе шатыг тэмдэглэхээс өмнө дараах зүйлийг баталгаажуулна уу:

1. Хамгийн сүүлийн үеийн бэхэлгээний багцын хэшүүд нь `fixtures/norito_rpc/schema_hashes.json` болон `artifacts/norito_rpc/<stamp>/` дор бүртгэгдсэн харгалзах CI олдвортой таарч байна.
2. SDK README / портал баримтууд нь JSON-ийг хэрхэн буцаах талаар тайлбарлаж, Norito анхдагч тээвэрлэлтийг иш татдаг.
3. Телеметрийн хяналтын самбар нь дохиоллын холбоос бүхий давхар стекийн алдааны түвшний самбаруудыг харуулдаг бөгөөд Alertmanager хуурай гүйлт (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) нь трекерт хавсаргасан байна.
4. Үрчлэлтийн хуваарь нь мөрдөгч бичлэгтэй (`docs/source/torii/norito_rpc_tracker.md`) таарч байгаа бөгөөд замын зураглал (NRPC-4) нь ижил нотлох баримтын багцыг заадаг.

Хуваарийн дагуу сахилга баттай байх нь SDK хоорондын зан үйлийг урьдчилан таамаглах боломжтой байлгаж, Norito-RPC-ийг захиалгаар хийх хүсэлтгүйгээр засаглалын аудит хийх боломжийг олгодог.