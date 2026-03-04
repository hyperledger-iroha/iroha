---
id: nexus-elastic-lane
lang: mn
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Elastic lane provisioning (NX-7)
sidebar_label: Elastic Lane Provisioning
description: Bootstrap workflow for creating Nexus lane manifests, catalog entries, and rollout evidence.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Каноник эх сурвалжийг анхаарна уу
Энэ хуудас нь `docs/source/nexus_elastic_lane.md`-ийг толилуулж байна. Орчуулга портал дээр буух хүртэл хоёр хуулбарыг зэрэгцүүлэн хадгална уу.
:::

# Уян эгнээ бэлтгэх хэрэгслийн хэрэгсэл (NX-7)

> **Замын зураглалын зүйл:** NX-7 — Уян эгнээний зохицуулалт хийх хэрэгсэл  
> **Төлөв:** Хэрэглүүр дууссан — манифест, каталогийн хэсэг, Norito ачаалал, утааны тест,
> ба ачааллын тестийн багцын туслагч одоо үүрний хоцролтыг залгаж байна + нотлох баримтууд тул баталгаажуулагч
> ачааллын гүйлтүүдийг захиалгат скриптгүйгээр нийтлэх боломжтой.

Энэхүү гарын авлага нь операторуудыг автоматжуулсан шинэ `scripts/nexus_lane_bootstrap.sh` туслах хэрэгслээр дамжуулж өгдөг.
эгнээний манифест үүсгэх, эгнээ/өгөгдлийн орон зайн каталогийн хэсэг, нэвтрүүлэх нотолгоо. Зорилго нь хийх явдал юм
олон файлыг гараар засварлахгүйгээр шинэ Nexus эгнээг (нийтийн эсвэл хувийн) эргүүлэхэд хялбар байдаг.
каталогийн геометрийг гараар дахин гаргах.

## 1. Урьдчилсан нөхцөл

1. Эгнээний нэр, өгөгдлийн орон зай, баталгаажуулагчийн багц, алдааг тэсвэрлэх чадвар (`f`), төлбөр тооцооны бодлого зэрэгт зориулсан засаглалын зөвшөөрөл.
2. Баталгаажуулагчийн эцсийн жагсаалт (дансны ID) болон хамгаалагдсан нэрсийн жагсаалт.
3. Үүсгэсэн хэсгүүдийг хавсаргах боломжтой зангилааны тохиргооны репозитор руу нэвтрэх.
4. Эгнээний манифест бүртгэлийн замууд (`nexus.registry.manifest_directory` болон
   `cache_directory`).
5. Эгнээнд зориулсан телеметрийн контактууд/PagerDuty бариултай тул дохиог эгнээнд нэн даруй дамжуулах боломжтой.
   онлайнаар ирдэг.

## 2. Эгнээний олдвор үүсгэх

Репозиторын үндэсээс туслагчийг ажиллуулна уу:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator ih58... \
  --validator ih58... \
  --validator ih58... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Түлхүүр тугнууд:

- `--lane-id` нь `nexus.lane_catalog` дахь шинэ оруулгын индекстэй тохирч байх ёстой.
- `--dataspace-alias` болон `--dataspace-id/hash` өгөгдлийн орон зайн каталогийн оруулгыг хянадаг (өгөгдмөл нь
  орхигдуулсан үед эгнээний id).
- `--validator`-г давтаж эсвэл `--validators-file`-ээс авах боломжтой.
- `--route-instruction` / `--route-account` буулгахад бэлэн чиглүүлэлтийн дүрмийг гаргадаг.
- `--metadata key=value` (эсвэл `--telemetry-contact/channel/runbook`) runbook-н харилцагчдыг авахын тулд
  хяналтын самбарууд нь зөв эзэмшигчдийг нэн даруй жагсаадаг.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` манифестэд ажиллах цагийг шинэчлэх дэгээ нэмнэ
  эгнээ нь уртасгасан операторын хяналт шаардлагатай үед.
- `--encode-space-directory` `cargo xtask space-directory encode`-г автоматаар дууддаг. Үүнийг хослуул
  `--space-directory-out` кодлогдсон `.to` файлыг анхдагчаас өөр газар авахыг хүсвэл.

Скрипт нь `--output-dir` дотор гурван олдвор үүсгэдэг (одоогийн лавлахын өгөгдмөл),
кодчилол идэвхжсэн үед нэмэлт дөрөв дэх нь:

1. `<slug>.manifest.json` — баталгаажуулагч чуулга, хамгаалагдсан нэрийн орон зай, багийг агуулсан эгнээний манифест
   нэмэлт ажиллах цагийн шинэчлэлтийн дэгээ мета өгөгдөл.
2. `<slug>.catalog.toml` — `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` бүхий TOML хэсэг,
   болон хүссэн чиглүүлэлтийн дүрэм. `fault_tolerance`-г өгөгдлийн орон зайн хэмжээтэй тохируулсан эсэхийг шалгана уу.
   эгнээний буухиа хороо (`3f+1`).
3. `<slug>.summary.json` — геометрийг (слаг, сегмент, мета өгөгдөл) тайлбарласан аудитын хураангуй
   шаардлагатай нэвтрүүлэх алхмууд болон яг `cargo xtask space-directory encode` тушаал (доор
   `space_directory_encode.command`). Нотолгоо авахын тулд энэ JSON-г онгоцны тийзэнд хавсаргана уу.
4. `<slug>.manifest.to` — `--encode-space-directory` тохируулагдсан үед ялгардаг; Torii-д бэлэн
   `iroha app space-directory manifest publish` урсгал.

JSON/ хэсгүүдийг файл бичихгүйгээр урьдчилан үзэхийн тулд `--dry-run`, дарж бичихийн тулд `--force`-г ашиглана уу.
одоо байгаа олдворууд.

## 3. Өөрчлөлтүүдийг хэрэгжүүлнэ үү

1. Манифест JSON-г тохируулсан `nexus.registry.manifest_directory` (мөн кэш рүү) хуулна уу.
   Хэрэв бүртгэл нь алсын багцуудыг тусгаж байгаа бол лавлах). Хэрэв манифестууд нь хувилбартай бол файлыг оруулна уу
   таны тохиргооны репо.
2. Каталогийн хэсгийг `config/config.toml` (эсвэл тохирох `config.d/*.toml`) дээр хавсаргана уу. Баталгаажуулах
   `nexus.lane_count` хамгийн багадаа `lane_id + 1` бөгөөд `nexus.routing_policy.rules`-г шинэчилнэ үү.
   шинэ эгнээ рүү чиглүүлэх ёстой.
3. Кодлоод (хэрэв та `--encode-space-directory`-г алгассан бол) манифестийг Space Directory-д нийтлээрэй.
   хураангуйд авсан тушаалыг ашиглан (`space_directory_encode.command`). Энэ нь үүсгэдэг
   `.manifest.to` ачаалал Torii аудиторуудад нотлох баримтыг хүлээж, бүртгэдэг; дамжуулан илгээх
   `iroha app space-directory manifest publish`.
4. `irohad --sora --config path/to/config.toml --trace-config`-г ажиллуулж, ул мөрийн гаралтыг архивлана
   танилцуулах тасалбар. Энэ нь шинэ геометр нь үүссэн slug/kura сегменттэй тохирч байгааг нотолж байна.
5. Манифест/каталогийн өөрчлөлтийг байрлуулсны дараа эгнээнд хуваарилагдсан баталгаажуулагчийг дахин эхлүүлнэ үү. Хадгалах
   ирээдүйн аудитын тасалбар дахь хураангуй JSON.

## 4. Бүртгэлийн түгээлтийн багцыг бүтээх

Үүсгэсэн манифест болон давхаргыг багцалж, операторууд эгнээний засаглалын өгөгдлийг ямар ч мэдээлэлгүйгээр түгээх боломжтой
хост бүр дээрх тохиргоог засварлах. Багцлагчийн туслах нь манифестуудыг каноник байршилд хуулдаг.
`nexus.registry.cache_directory`-д зориулсан засаглалын каталогийн нэмэлт давхаргыг гаргадаг бөгөөд
офлайн дамжуулалтад зориулсан tarball:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Гаралт:

1. `manifests/<slug>.manifest.json` — эдгээрийг тохируулсан руу хуулна
   `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` — `nexus.registry.cache_directory` руу унах. `--module` бүр
   оруулга нь залгах боломжтой модулийн тодорхойлолт болж, засаглалын модулийг солих (NX-2)
   `config.toml` засварлахын оронд кэш давхаргыг шинэчлэх.
3. `summary.json` — хэш, давхарласан мета өгөгдөл, операторын зааварчилгааг агуулдаг.
4. Нэмэлт `registry_bundle.tar.*` — SCP, S3 эсвэл олдворыг хянахад бэлэн.

Бүх лавлахыг (эсвэл архивыг) баталгаажуулагч бүртэй синк хийж, агаарын зайтай хостууд дээр задалж, хуулах
Torii-г дахин эхлүүлэхийн өмнө манифестууд + кэш давхаргыг бүртгэлийн замд оруулна.

## 5. Баталгаажуулагчийн утааны туршилт

Torii дахин ачаалсны дараа `manifest_ready=true` эгнээний мэдээг шалгахын тулд шинэ утаа туслагчийг ажиллуулна уу.
хэмжүүрүүд нь хүлээгдэж буй эгнээний тоог харуулж, битүүмжилсэн хэмжигч нь тодорхой байна. Манифест шаарддаг эгнээ
хоосон биш `manifest_path`-г ил гаргах ёстой; Туслагч одоо зам байхгүй үед шууд бүтэлгүйтдэг
NX-7-ийн байршуулалтын бүртгэл бүрт гарын үсэг зурсан манифест нотлох баримт орно.

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Өөрөө гарын үсэг зурсан орчныг туршихдаа `--insecure` нэмнэ үү. Хэрэв эгнээ байгаа бол скрипт тэгээс өөр гарна
дутуу, битүүмжилсэн эсвэл хүлээгдэж буй утгуудаас хэмжигдэхүүн/телеметрийн зөрүү. -г ашиглана уу
`--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog`, болон
`--max-headroom-events` бариулууд нь эгнээ тус бүрийн блокийн өндөр/эцсийн/хоцрогдол/өрийн зайны телеметрийг хадгалах
үйл ажиллагааны дугтуйндаа багтааж, тэдгээрийг `--max-slot-p95` / `--max-slot-p99`-тэй холбоно уу.
(нэмэх `--min-slot-samples`) нь туслагчийг орхихгүйгээр NX‑18 үүрний үргэлжлэх хугацааны зорилтуудыг хэрэгжүүлэх.

Агаарын зайтай баталгаажуулалтын (эсвэл CI) хувьд та шууд дамжуулалт хийхийн оронд авсан Torii хариултыг дахин тоглуулах боломжтой.
эцсийн цэг:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

`fixtures/nexus/lanes/` доор бүртгэгдсэн бэхэлгээ нь ачааны оосрын үйлдвэрлэсэн эд өлгийн зүйлсийг тусгадаг.
Туслагч тул шинэ манифестуудыг захиалгат скриптгүйгээр зурж болно. CI нь ижил урсгалаар дамжуулан дасгал хийдэг
`ci/check_nexus_lane_smoke.sh` ба `ci/check_nexus_lane_registry_bundle.sh`
NX-7 утаа туслагч нь нийтлэгдсэнтэй нийцэж байгааг нотлохын тулд (хоол нэр: `make check-nexus-lanes`)
ачааны формат болон багцын дижест/давхцлыг дахин гаргах боломжтой байлгахын тулд.

Замын нэрийг өөрчлөх үед `nexus.lane.topology` телеметрийн үйл явдлуудыг (жишээ нь
`journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`) болон тэдгээрийг буцааж оруулах
утааны туслагч. `--telemetry-file/--from-telemetry` туг нь шинэ мөрөөр тусгаарлагдсан лог болон
`--require-alias-migration old:new` нь `alias_migrated` үйл явдал нь нэрийг өөрчилсөн гэж тэмдэглэсэн байна:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

`telemetry_alias_migrated.ndjson` бэхэлгээ нь каноник нэр өөрчлөх дээжийг багцалсан тул CI баталгаажуулах боломжтой.
амьд зангилаатай холбоо барихгүйгээр телеметрийн задлан шинжлэх зам.

## Баталгаажуулагчийн ачааллын туршилт (NX-7 нотолгоо)

Замын зураг **NX-7** нь шинэ эгнээ болгонд дахин давтагдах боломжтой баталгаажуулагч ачааллыг тээвэрлэхийг шаарддаг. Ашиглах
`scripts/nexus_lane_load_test.py` утаа шалгах, үүрний үргэлжлэх хаалга, үүрний багцыг оёхын тулд
засаглалыг давтаж чадах нэг олдвор болгон харуулах:

```bash
scripts/nexus_lane_load_test.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --slot-range 81200-81600 \
  --workload-seed NX7-PAYMENTS-2026Q2 \
  --require-alias-migration core:payments \
  --out-dir artifacts/nexus/load/payments-2026q2
```

Туслагч нь ашигласан ижил DA чуулга, oracle, тооцооны буфер, TEU болон слот үргэлжлэх хаалгануудыг хэрэгжүүлдэг.
утааны туслагчаар бичиж, `smoke.log`, `slot_summary.json`, үүрний багц манифест болон
`load_test_manifest.json`-ийг сонгосон `--out-dir`-д оруулснаар ачааллын гүйлтийг шууд холбох боломжтой.
захиалгат скриптгүйгээр тасалбарыг гаргах.

## 6. Телеметр ба засаглалын дагалт

- Замын хяналтын самбарыг (`dashboards/grafana/nexus_lanes.json` болон холбогдох давхаргууд) шинэчилнэ үү.
  шинэ эгнээний id болон мета өгөгдөл. Үүсгэсэн мета өгөгдлийн түлхүүрүүд (`contact`, `channel`, `runbook` гэх мэт)
  шошгыг урьдчилан бөглөхөд хялбар байдаг.
- Элсэлтийг идэвхжүүлэхийн өмнө шинэ эгнээний PagerDuty/Alertmanager-ийн дүрмийг заана уу. `summary.json`
  дараагийн алхамуудын массив нь [Nexus үйлдлүүд](./nexus-operations) доторх хяналтын хуудсыг тусгадаг.
- Баталгаажуулагчийн багц ажиллаж эхэлмэгц манифест багцыг Space Directory-д бүртгүүлнэ үү. Үүнтэй адил хэрэглээрэй
  засаглалын дэвтэрийн дагуу гарын үсэг зурсан туслагчийн үүсгэсэн манифест JSON.
- Утааны шинжилгээнд хамрагдахын тулд [Sora Nexus операторыг суулгаж байна](./nexus-operator-onboarding)-г дагаж утааны туршилт (FindNetworkStatus, Torii)
  хүртээмжтэй байх) ба дээр үйлдвэрлэсэн олдворын иж бүрдэл бүхий нотлох баримтыг цуглуул.

## 7. Хуурай гүйлтийн жишээ

Файл бичихгүйгээр олдворуудыг урьдчилан харахын тулд:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator ih58... \
  --validator ih58... \
  --dry-run
```

Энэ тушаал нь JSON хураангуй болон TOML хэсгийг stdout руу хэвлэж, тухайн үед хурдан давталт хийх боломжийг олгодог.
төлөвлөлт.

---

Нэмэлт контекстийг үзнэ үү:- [Nexus үйл ажиллагаа](./nexus-operations) — үйл ажиллагааны хяналтын хуудас болон телеметрийн шаардлага.
- [Sora Nexus операторыг суулгаж байна](./nexus-operator-onboarding) — холболтын дэлгэрэнгүй урсгал
  шинэ туслагч.
- [Nexus эгнээний загвар](./nexus-lane-model) — уг хэрэгсэлд ашигладаг эгнээний геометр, slugs, хадгалах байршил.