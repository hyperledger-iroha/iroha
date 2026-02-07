---
lang: mn
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8599dbc1a8e4fe846965eed90af128deb5950f83dc61838fea583b326b92a011
source_last_modified: "2025-12-29T18:16:35.104300+00:00"
translation_last_reviewed: 2026-02-07
id: incident-runbooks
title: Incident Runbooks & Rollback Drills
sidebar_label: Incident Runbooks
description: Response guides for failed portal deployments, SoraFS replication degradation, analytics outages, and the quarterly rehearsal cadence required by DOCS-9.
translator: machine-google-reviewed
---

## Зорилго

Замын зургийн **DOCS-9** зүйлд хэрэгжүүлэх боломжтой тоглоомын номууд болон давталтын төлөвлөгөөг шаарддаг.
Порталын операторууд тээвэрлэлтийн доголдлыг тааварлалгүйгээр сэргээх боломжтой. Энэ тэмдэглэл
Дохио ихтэй гурван тохиолдлыг хамарна - бүтэлгүйтсэн байршуулалт, хуулбар
доройтол, аналитикийн тасалдал зэрэг нь улирал тутам хийдэг сургуулилтуудыг баримтжуулдаг
хуурамч нэр буцаах болон синтетик баталгаажуулалт нь төгсгөл хүртэл ажилладаг хэвээр байна.

### Холбогдох материал

- [`devportal/deploy-guide`](./deploy-guide) — савлагаа, гарын үсэг, нэр
  сурталчилгааны ажлын урсгал.
- [`devportal/observability`](./observability) — шошго, аналитик болон
  шалгалтуудыг доор дурдсан болно.
- `docs/source/sorafs_node_client_protocol.md`
  болон [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — бүртгэлийн телеметр ба өсөлтийн босго.
- `docs/portal/scripts/sorafs-pin-release.sh` болон `npm run probe:*` туслахууд
  шалгах хуудасны бүх хэсэгт иш татсан.

### Хуваалцсан телеметр ба багаж хэрэгсэл

| Дохио / Хэрэгсэл | Зорилго |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (хансан/алгасан/хүлээгдэж байгаа) | Хуулбарлах лангуу болон SLA зөрчлийг илрүүлдэг. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Гурав дахь удаашралын гүн болон дуусгах хоцрогдлын хэмжээг тодорхойлдог. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Муу суулгалтыг дагадаг гарцын талын алдааг харуулдаг. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Синтетик датчик нь хаалгыг суллаж, гүйлгээг баталгаажуулдаг. |
| `npm run check:links` | Хагарсан холбоосын хаалга; бууруулах болгоны дараа хэрэглэнэ. |
| `sorafs_cli manifest submit … --alias-*` (`scripts/sorafs-pin-release.sh` ороосон) | Алсыг дэмжих/хувиргах механизм. |
| `Docs Portal Publishing` Grafana самбар (`dashboards/grafana/docs_portal.json`) | Татгалзах/алиас/TLS/хуулбарлах телеметрийг нэгтгэдэг. PagerDuty дохиолол нь эдгээр самбарыг нотлох баримт болгон ашигладаг. |

## Runbook — Амжилтгүй байршуулалт эсвэл муу олдвор

### Өдөөгч нөхцөл

- Урьдчилан харах/үйлдвэрлэлийн датчик амжилтгүй болсон (`npm run probe:portal -- --expect-release=…`).
- Grafana дохио `torii_sorafs_gateway_refusals_total` эсвэл
  Гаргасны дараа `torii_sorafs_manifest_submit_total{status="error"}`.
- Гарын авлагын QA нь эвдэрсэн маршрут эсвэл Try-It прокси алдааг шууд мэдэгддэг
  нэрийн сурталчилгаа.

### Яаралтай саатуулах

1. **Байрлуулалтыг царцаах:** CI дамжуулах хоолойг `DEPLOY_FREEZE=1` (GitHub) ашиглан тэмдэглэнэ үү.
   ажлын урсгалын оролт) эсвэл Женкинсийн ажлыг түр зогсоосноор нэмэлт олдвор гарахгүй.
2. **Артефактуудыг барих:** бүтэлгүйтсэн бүтээцийн `build/checksums.sha256`-г татаж авах,
   `portal.manifest*.{json,to,bundle,sig}`, мөн датчик гаралтыг хийснээр буцах боломжтой
   нарийн задаргаа лавлагаа.
3. **Оролцогч талуудад мэдэгдэх:** хадгалах сангийн SRE, Docs/DevRel удирдагч болон засаглал
   жижүүр (ялангуяа `docs.sora` нөлөөлсөн үед).

### Буцах журам

1. Хамгийн сүүлд мэдэгдэж байгаа сайн (LCG) манифестийг тодорхойлох. Үйлдвэрлэлийн ажлын урсгалыг хадгалдаг
   тэдгээрийг `artifacts/devportal/<release>/sorafs/portal.manifest.to` дор.
2. Тээвэрлэлтийн туслагчийн тусламжтайгаар тухайн манифестийн нэрийг дахин холбоно уу:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. Буцах хураангуйг ослын тасалбарт LKG болон хамт тэмдэглэнэ
   амжилтгүй манифест задаргаа.

### Баталгаажуулалт

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature …` ба `sorafs_cli proof verify …`
   (байршуулах гарын авлагыг үзнэ үү) дахин сурталчилсан манифест таарч байгааг баталгаажуулна уу
   архивлагдсан CAR.
4. `npm run probe:tryit-proxy` Try-It staging прокси буцаж ирснийг баталгаажуулна уу.

### Үйл явдлын дараах

1. Зөвхөн үндсэн шалтгааныг ойлгосны дараа байршуулах дамжуулах хоолойг дахин идэвхжүүлнэ.
2. Буцаан дүүргэх [`devportal/deploy-guide`](./deploy-guide) “Сурах сургамж”
   хэрэв байгаа бол шинэ готчатай оруулгууд.
3. Амжилтгүй болсон туршилтын багцын файлын согог (шинж, холбоос шалгагч гэх мэт).

## Runbook — Хуулбарлах доройтол

### Өдөөгч нөхцөл

- Анхааруулга: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(нийлбэр(torii_sorafs_replication_sla_total{үр дүн=~"сансан|алдсан"}), 1) <
  10 минутын турш 0.95`.
- `torii_sorafs_replication_backlog_total > 10` 10 минутын турш (харна уу
  `pin-registry-ops.md`).
- Засаглал гаргасны дараа нэрийн хүртээмж удаашралтай байгааг мэдээлдэг.

### Гурвалж

1. Баталгаажуулахын тулд [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) хяналтын самбарыг шалгана уу.
   хоцрогдол нь хадгалах анги эсвэл үйлчилгээ үзүүлэгчийн флот руу локалчлагдсан эсэх.
2. Torii бүртгэлээс `sorafs_registry::submit_manifest` анхааруулга байгаа эсэхийг шалгана уу.
   мэдүүлэг өөрөө бүтэлгүйтсэн эсэхийг тодорхойлох.
3. `sorafs_cli manifest status --manifest …` (жагсаалт
   үйлчилгээ үзүүлэгч бүрийн хуулбарын үр дүн).

### Хөнгөвчлөх

1. Манифестыг дахин хуулбарлах тоо (`--pin-min-replicas 7`) ашиглан дахин гаргах
   `scripts/sorafs-pin-release.sh`, ингэснээр хуваарь гаргагч ачааллыг томоор тараадаг
   үйлчилгээ үзүүлэгчийн багц. Шинэ манифестийг тохиолдлын бүртгэлд тэмдэглэ.
2. Хэрэв хоцрогдол нь нэг үйлчилгээ үзүүлэгчтэй холбоотой бол үүнийг дамжуулан түр идэвхгүй болго
   хуулбарлах хуваарь гаргагч (`pin-registry-ops.md`-д баримтжуулсан) болон шинээр илгээх
   манифест нь бусад үйлчилгээ үзүүлэгчдийг хуурамч нэрийг сэргээхийг албаддаг.
3. Хуурай нэрийн шинэлэг байдал нь репликацын паритетаас илүү чухал бол дахин холбоно уу
   аль хэдийн үе шаттай (`docs-preview`) халуун манифестын нэрээр нэрлэж, дараа нь нийтлэх
   SRE хоцрогдол арилгасны дараа дараагийн манифест.

### Сэргээх & хаах

1. баталгаажуулахын тулд `torii_sorafs_replication_sla_total{outcome="missed"}` хяналт тавина
   тэгш өндөрлөгүүдийг тоол.
2. Хуулбар болгон байгаагийн нотолгоо болгон `sorafs_cli manifest status` гаралтыг ав
   дагаж мөрдөх.
3. Үхлийн дараах хуулбарыг дараагийн алхмуудаар файл эсвэл шинэчилнэ үү
   (үйлүүлэгчийн масштаб, chunker тааруулах гэх мэт).

## Runbook — Аналитик эсвэл телеметрийн тасалдал

### Өдөөгч нөхцөл

- `npm run probe:portal` амжилттай болсон ч хяналтын самбарууд залгихаа больсон
  >15 минутын турш `AnalyticsTracker` үйл явдлууд.
- Нууцлалын хяналт нь орхигдсон үйл явдлуудын гэнэтийн өсөлтийг тэмдэглэдэг.
- `npm run probe:tryit-proxy` `/probe/analytics` зам дээр амжилтгүй болсон.

### Хариулт

1. Бүтээх хугацааны оролтыг шалгах: `DOCS_ANALYTICS_ENDPOINT` болон
   Амжилтгүй хувилбар дахь `DOCS_ANALYTICS_SAMPLE_RATE` (`build/release.json`).
2. `DOCS_ANALYTICS_ENDPOINT`-г зааж `npm run probe:portal`-г дахин ажиллуулна уу.
   taging коллектор нь трекер нь даацыг ялгаруулж байгааг баталгаажуулах.
3. Хэрэв коллекторууд ажиллахаа больсон бол `DOCS_ANALYTICS_ENDPOINT=""`-г тохируулаад дахин бүтээгээрэй.
   трекерийн богино холболт; ослын цагийн хуваарьт тасалдсан цонхыг тэмдэглэ.
4. `scripts/check-links.mjs` хурууны хээг баталгаажуулна уу `checksums.sha256`
   (шинжилгээний тасалдал нь сайтын газрын зургийн баталгаажуулалтыг * блоклох ёсгүй).
5. Коллектор сэргэсний дараа `npm run test:widgets`-г ажиллуулж дасгал хийнэ
   аналитик туслах нэгжийг дахин нийтлэхээс өмнө туршиж үздэг.

### Үйл явдлын дараах

1. Шинэ цуглуулагчтай [`devportal/observability`](./observability) шинэчилнэ үү
   хязгаарлалт эсвэл дээж авах шаардлага.
2. Ямар нэгэн аналитик өгөгдөл гадуур хасагдсан эсвэл засварлагдсан тохиолдолд файлын засаглалын мэдэгдэл
   бодлого.

## Улирал тутмын уян хатан байдлын дасгалууд

**Улирал бүрийн эхний Мягмар гарагт** (1/4/7/10) дасгалуудыг хоёуланг нь явуулна.
эсвэл дэд бүтцийн томоохон өөрчлөлтийн дараа шууд. Олдворуудыг доор хадгална
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Өрөмдлөг | Алхам | Нотлох баримт |
| ----- | ----- | -------- |
| Alias ​​буцаах бэлтгэл | 1. "Бүтэлгүйтсэн байршуулалт"-ыг хамгийн сүүлийн үеийн үйлдвэрлэлийн манифест ашиглан дахин тоглуул.<br/>2. Шинжилгээг дамжуулсны дараа үйлдвэрлэлд дахин холбоно.<br/>3. `portal.manifest.submit.summary.json`-г тэмдэглэж, шалгалтын бүртгэлийг өрмийн хавтсанд хийнэ. | `rollback.submit.json`, датчик гаралт, сургуулилтын шошгыг гаргах. |
| Синтетик баталгаажуулалтын аудит | 1. `npm run probe:portal` болон `npm run probe:tryit-proxy`-ийг үйлдвэрлэл, үе шаттай харьцуулан ажиллуул.<br/>2. `npm run check:links` ажиллуулж, `build/link-report.json` архивлана.<br/>3. Шинжилгээний амжилтыг баталгаажуулсан Grafana хавтангийн дэлгэцийн агшин/экспортыг хавсаргана уу. | Шинжилгээний бүртгэл + `link-report.json` ил тод хурууны хээг иш татдаг. |

Алдагдсан дасгалуудыг Docs/DevRel менежер болон SRE-ийн засаглалын тойм руу шилжүүлж,
Учир нь замын зураг нь аль алиных нь аль алиных нь аль алиныг нь тодорхойлох, улирал тутам нотлох баримт шаарддаг
буцаах болон портал пробууд эрүүл хэвээр байна.

## Пэйжер Үүргийн болон дуудлагын зохицуулалт

- PagerDuty үйлчилгээ **Docs Portal Publishing** нь дараахаас үүсгэсэн сэрэмжлүүлгийг эзэмшдэг.
  `dashboards/grafana/docs_portal.json`. Дүрэм `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache` болон `DocsPortal/TLSExpiry` Докс/ДевРел-д хуудаснаа
  хоёрдогчоор Хадгалах SRE-тэй анхдагч.
- Хуудсаны дараа `DOCS_RELEASE_TAG`-г оруулж, нөлөөлөлд өртсөн хүмүүсийн дэлгэцийн агшинг хавсаргана уу.
  Grafana самбар, мөн өмнө тохиолдлын тэмдэглэлд шалгах/холбоос шалгах гаралтыг холбох
  бууруулах ажил эхэлдэг.
- Хялбаршуулсаны дараа (буцах эсвэл дахин байршуулах) `npm run probe:portal`-г дахин ажиллуул,
  `npm run check:links`, хэмжигдэхүүнийг харуулсан шинэ Grafana агшин зуурын агшинг авах
  босгон дотор буцах. Өмнө нь PagerDuty-ийн үйл явдалтай холбоотой бүх нотлох баримтыг хавсаргана уу
  үүнийг шийдэж байна.
- Хэрэв хоёр дохио нэгэн зэрэг асвал (жишээ нь TLS дуусах хугацаа болон хоцрогдол), триаж хийх
  эхлээд татгалзсан (нийтлэхээ зогсоо), буцаах процедурыг гүйцэтгээд дараа нь арилгана
  Гүүрэн дээрх SRE SRE бүхий TLS/хоцрогдсон зүйлс.