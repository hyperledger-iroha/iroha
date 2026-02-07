---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 672a5e3a6f0c3e8999400bc6fa8c66cc3be1ba2119431c5fd26f6d9a436f767f
source_last_modified: "2025-12-29T18:16:35.187152+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Gateway & DNS Kickoff Runbook

Энэ портал хуулбар нь каноник runbook-ийг толин тусгадаг
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Энэ нь төвлөрсөн бус DNS & Gateway-ийн үйл ажиллагааны хамгаалалтын хашлагуудыг барьж авдаг
ажлын урсгал, ингэснээр сүлжээ, үйл ажиллагаа, баримтжуулалтын удирдагчид үүнийг давтах боломжтой
2025-03 оны эхлэхээс өмнө автоматжуулалтын стек.

## Хамрах хүрээ ба хүргэх зүйлс

- Тодорхойлолтыг давтаж DNS (SF‑4) болон гарц (SF‑5) үе шатуудыг холбоно.
  хостын гарал үүсэл, шийдүүлэгчийн лавлах хувилбарууд, TLS/GAR автоматжуулалт, нотолгоо
  барих.
- Тоглолтын оролтыг (хэлэлцэх асуудал, урилга, ирц хянагч, GAR телеметр) хадгал
  хормын хувилбар) эзэмшигчийн хамгийн сүүлийн даалгавартай синхрончлогдсон.
- Засаглалын үнэлгээчдэд аудит хийх боломжтой олдворын багцыг гаргах: шийдвэрлэх
  лавлах хувилбарын тэмдэглэл, гарц шалгах лог, тохирлын бэхэлгээний гаралт болон
  Docs/DevRel хураангуй.

## Үүрэг, хариуцлага

| Ажлын урсгал | Хариуцлага | Шаардлагатай олдворууд |
|------------|------------------|--------------------|
| Сүлжээний TL (DNS стек) | Тодорхойлолттой хост төлөвлөгөөг хөтлөх, RAD лавлах хувилбаруудыг ажиллуулах, шийдэгчийн телеметрийн оролтыг нийтлэх. | `artifacts/soradns_directory/<ts>/`, `docs/source/soradns/deterministic_hosts.md`, RAD мета өгөгдлийн ялгаа. |
| Үйл ажиллагааны автоматжуулалтын удирдагч (гарц) | TLS/ECH/GAR автоматжуулалтын дасгалуудыг гүйцэтгэх, `sorafs-gateway-probe` ажиллуулах, PagerDuty дэгээг шинэчлэх. | `artifacts/sorafs_gateway_probe/<ts>/`, probe JSON, `ops/drill-log.md` оруулгууд. |
| QA Guild & Tooling WG | `ci/check_sorafs_gateway_conformance.sh` ажиллуулж, бэхэлгээг засаж, Norito өөрөө баталгаажуулсан багцуудыг архивлаарай. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Докс / DevRel | Минутуудыг авч, дизайныг урьдчилан уншсан + хавсралтыг шинэчилж, нотлох баримтын хураангуйг энэ порталд нийтлээрэй. | `docs/source/sorafs_gateway_dns_design_*.md` файлууд болон танилцуулах тэмдэглэлүүдийг шинэчилсэн. |

## Оролтууд ба урьдчилсан нөхцөл

- Тодорхойлогч хостын үзүүлэлт (`docs/source/soradns/deterministic_hosts.md`) ба
  шийдэгчийн баталгаажуулалтын шат (`docs/source/soradns/resolver_attestation_directory.md`).
- Гарцын олдворууд: операторын гарын авлага, TLS/ECH автоматжуулалтын туслахууд,
  шууд горимын удирдамж, `docs/source/sorafs_gateway_*`-ийн дагуу өөрийгөө баталгаажуулах ажлын урсгал.
- Багаж хэрэгсэл: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, болон CI туслахууд
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Нууц: GAR хувилбарын түлхүүр, DNS/TLS ACME итгэмжлэлүүд, PagerDuty чиглүүлэлтийн түлхүүр,
  Шийдвэрлэгч татан авалтад зориулсан Torii баталгаажуулалтын токен.

## Нислэгийн өмнөх шалгах хуудас

1. Оролцогчид болон хэлэлцэх асуудлыг шинэчлэх замаар баталгаажуулна
   `docs/source/sorafs_gateway_dns_design_attendance.md` ба эргэлдэж буй
   одоогийн хэлэлцэх асуудал (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. зэрэг тайзны олдворын үндэс
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` ба
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Бэхэлгээг шинэчлэх (GAR манифест, RAD баталгаа, гарцын тохирлын багц) болон
   `git submodule` төлөв хамгийн сүүлийн үеийн бэлтгэлийн шошготой таарч байгаа эсэхийг шалгаарай.
4. Нууцыг баталгаажуулах (Ed25519 хувилбарын түлхүүр, ACME бүртгэлийн файл, PagerDuty токен)
   хадгалалтын шалгах нийлбэрийг харуулах ба тааруулах.
5. Утааны туршилтын телеметрийн зорилтууд (Pushgateway төгсгөлийн цэг, GAR Grafana самбар) өмнөх
   өрөм рүү.

## Автоматжуулалтын давталтын үе шатууд

### Тодорхойлогч хостын зураг ба RAD лавлах хувилбар

1. Санал болгож буй манифестын эсрэг детерминист хостын үүсмэл туслагчийг ажиллуул
   тохируулж, ямар ч шилжилт байхгүй гэдгийг баталгаажуулна уу
   `docs/source/soradns/deterministic_hosts.md`.
2. Шийдвэрлэгчийн лавлах багц үүсгэнэ үү:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Хэвлэсэн лавлах ID, SHA-256 болон гаралтын замыг дотор нь бичнэ үү
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` ба эхлэл
   минут.

### DNS телеметрийн зураг авалт

- ≥10 минутын турш сүүлийг шийдэгчийн ил тод байдлын бүртгэлийг ашиглана
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Pushgateway хэмжигдэхүүнийг экспортлох ба NDJSON агшин зуурын агшингуудыг гүйлтийн хажуугаар архивлах
  ID лавлах.

### Гарцын автоматжуулалтын дасгалууд

1. TLS/ECH шалгалтыг гүйцэтгэнэ:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Тохиромжтой бэхэлгээ (`ci/check_sorafs_gateway_conformance.sh`) болон ажиллуулна
   өөрийгөө баталгаажуулах туслах (`scripts/sorafs_gateway_self_cert.sh`) -ийг шинэчлэх
   Norito баталгаажуулалтын багц.
3. PagerDuty/Webhook үйл явдлуудыг зурж автоматжуулалтын зам эцэс хүртэл ажиллана
   төгсгөл.

### Нотлох баримтын савлагаа

- `ops/drill-log.md`-г цагийн тэмдэг, оролцогчид, шалгах хэш зэргээр шинэчил.
- Ажиллах ID лавлах дор олдворуудыг хадгалж, гүйцэтгэлийн хураангуйг нийтлэх
  Docs/DevRel уулзалтын тэмдэглэлд.
- Үзлэг эхлэхээс өмнө засаглалын тасалбар дахь нотлох баримтын багцыг холбоно уу.

## Хурал хөнгөвчлөх, нотлох баримт гардуулах

- **Модераторын цаг:**  
  - T‑24h — Хөтөлбөрийн удирдлага `#nexus-steering` дээр сануулагч + хэлэлцэх асуудал/ирцийн агшин зуурын зургийг нийтэлдэг.  
  - T‑2h — Сүлжээний TL нь GAR телеметрийн агшин зуурын зургийг шинэчилж, `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` дээр дельта бичдэг.  
  - T‑15m — Ops Automation нь датчикийн бэлэн байдлыг шалгаж, идэвхтэй ажиллуулах ID-г `artifacts/sorafs_gateway_dns/current` руу бичдэг.  
  - Дуудлагын үеэр — Зохицуулагч энэ runbook-г хуваалцаж, шууд бичээчийг томилдог; Docs/DevRel файлуудыг шугамаар авах.
- **Минутын загвар:** Араг ясыг хуулах
  `docs/source/sorafs_gateway_dns_design_minutes.md` (мөн портал дээр тусгагдсан
  багц) болон сесс бүрт нэг дүүргэсэн тохиолдлыг гүйцэтгэнэ. Оролцогчдын жагсаалтыг оруулах,
  шийдвэр, үйл ажиллагааны зүйл, нотлох баримт хэш, болон үлдэгдэл эрсдэл.
- **Нотлох баримт байршуулах:** Сургуулилтаас `runbook_bundle/` лавлахыг зип,
  үзүүлсэн протоколыг PDF хавсаргах, SHA-256 хэшийг протоколд бичих + хэлэлцэх асуудал,
  дараа нь байршуулах үед засаглалын хянан шалгагчийн нэр дээр ping хийнэ үү
  `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Нотлох баримтын агшин зураг (2025 оны 3-р сарын эхлэл)

Замын зураг болон засаглалд дурдсан хамгийн сүүлийн үеийн бэлтгэл/амьд олдворууд
минут `s3://sora-governance/sorafs/gateway_dns/` хувин дор амьдардаг. Хэш
доор каноник манифестийг толин тусгана (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **Хуурай гүйлт — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Багц tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - Минут PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Шууд семинар — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Хүлээгдэж буй байршуулалт: `gateway_dns_minutes_20250303.pdf` — Докс/DevRel нь буулгасан PDF багцад орох үед SHA-256-г хавсаргана.)_

## Холбоотой материал

- [Гарц үйлдлийн тоглоомын ном](./operations-playbook.md)
- [SoraFS ажиглалтын төлөвлөгөө](./observability-plan.md)
- [Төвлөрсөн бус DNS & Гарц мөрдөгч](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)