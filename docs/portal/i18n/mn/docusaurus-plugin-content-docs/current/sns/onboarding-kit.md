---
lang: mn
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS metrics & onboarding kit
description: Dashboard, pricing, and automation artifacts referenced by roadmap item SN-8.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SNS хэмжигдэхүүн ба суулгах хэрэгсэл

Замын зургийн **SN-8** зүйлд хоёр амлалт багтсан болно:

1. Бүртгэл, сунгалт, ARPU, маргаан, болон
   `.sora`, `.nexus`, `.dao`-ийн цонхыг царцаах.
2. Бүртгэгчид болон даамал нар DNS, үнэ болон утсаар холбогдож болохын тулд суулгах иж бүрдлийг илгээнэ үү.
   API-ууд нь ямар ч дагавар ажиллахаас өмнө тогтмол байдаг.

Энэ хуудас нь эх хувилбарыг толилуулж байна
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
тиймээс гадны тоймчид ижил журмыг дагаж мөрдөх боломжтой.

## 1. Метрийн багц

### Grafana хяналтын самбар ба портал оруулах

- `dashboards/grafana/sns_suffix_analytics.json`-г Grafana (эсвэл өөр) руу импортлох
  аналитик хост) стандарт API-ээр дамжуулан:

```bash
curl -H "Content-Type: application/json" \
     -H "Authorization: Bearer ${GRAFANA_TOKEN}" \
     -X POST https://grafana.sora.net/api/dashboards/db \
     --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- Ижил JSON нь энэ портал хуудасны iframe-г идэвхжүүлдэг (**SNS KPI хяналтын самбар**-г үзнэ үү).
  Хяналтын самбарыг мөргөх бүртээ гүй
  `npm run build && npm run serve-verified-preview` дотор `docs/portal` хүртэл
  Grafana болон суулгацыг синхрончлолд байлгахыг баталгаажуул.

### Самбар ба нотлох баримт

| Самбар | хэмжүүр | Засаглалын нотолгоо |
|-------|---------|---------------------|
| Бүртгэл ба сунгалт | `sns_registrar_status_total` (амжилт + шинэчлэх шийдэгч шошго) | Дагаврын дамжуулалт + SLA хянах. |
| ARPU / цэвэр нэгж | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | Санхүү нь бүртгэгчийн манифестийг орлоготой тааруулж болно. |
| Маргаан ба царцаалт | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | Идэвхтэй хөлдөлт, арбитрын хэмнэл, асран хамгаалагчийн ажлын ачааллыг харуулна. |
| SLA/алдааны түвшин | `torii_request_duration_seconds`, `sns_registrar_status_total{status="error"}` | Хэрэглэгчдэд нөлөөлөхөөс өмнө API регрессийг онцлон харуулна. |
| Бөөн манифест трекер | `sns_bulk_release_manifest_total`, `manifest_id` шошготой төлбөрийн хэмжүүр | CSV уналтыг төлбөр тооцооны тасалбартай холбодог. |

Сар бүрийн KPI-д Grafana (эсвэл суулгагдсан iframe)-аас PDF/CSV экспортлох
хянаж үзээд доорх хавсралтад хавсаргана
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Удирдагчид мөн SHA-256-г барьж авдаг
`docs/source/sns/reports/` дагуу экспортлогдсон багцын (жишээ нь,
`steward_scorecard_2026q1.md`) тул аудитууд нотлох баримтын замыг дахин харуулах боломжтой.

### Хавсралтын автоматжуулалт

Хяналтын самбарын экспортоос хавсралтын файлуудыг шууд үүсгэснээр хянагчдад
тууштай задаргаа:

```bash
cargo xtask sns-annex \
  --suffix .sora \
  --cycle 2026-03 \
  --dashboard dashboards/grafana/sns_suffix_analytics.json \
  --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json \
  --output docs/source/sns/reports/.sora/2026-03.md \
  --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md \
  --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- Туслах нь экспортыг хэш болгож, UID/tags/panel-ийн тоог барьж, бичнэ.
  Markdown хавсралт `docs/source/sns/reports/.<suffix>/<cycle>.md` (харна уу
  `.sora/2026-03` дээжийг энэ баримт бичигтэй хамт хийсэн).
- `--dashboard-artifact` экспортыг хуулдаг
  `artifacts/sns/regulatory/<suffix>/<cycle>/` тул хавсралтад эш татсан болно
  каноник нотлох зам; Зөвхөн зааж өгөх шаардлагатай үед `--dashboard-label`-г ашиглана уу
  хамтлагаас гадуурх архивт.
- `--regulatory-entry` удирдлагын санамж бичигт оноож байна. Туслагч оруулдаг (эсвэл
  орлоно) хавсралтын зам, хяналтын самбарыг бүртгэдэг `KPI Dashboard Annex` блок
  олдвор, хайш, цаг тэмдэглэгээ хийх тул дахин ажиллуулсны дараа нотлох баримтууд синхрончлогддог.
- `--portal-entry` Docusaurus хуулбарыг хадгалдаг (`docs/portal/docs/sns/regulatory/*.md`)
  зэрэгцүүлсэн тул тоймчид хавсралтын хураангуйг тусад нь гараар ялгах шаардлагагүй болно.
- Хэрэв та `--regulatory-entry`/`--portal-entry`-г алгасвал үүсгэсэн файлыг хавсаргана уу.
  санамж бичгийг гараар оруулах ба Grafana-ээс авсан PDF/CSV агшин агшинг байршуулсан хэвээр байна.
- Давтагдах экспортын хувьд дагавар/мөчлөгийн хосуудыг жагсаана уу
  `docs/source/sns/regulatory/annex_jobs.json` ба ажиллуулна уу
  `python3 scripts/run_sns_annex_jobs.py --verbose`. Туслах хүн орох болгонд алхаж,
  хяналтын самбарын экспортыг хуулдаг (анхдагчаар `dashboards/grafana/sns_suffix_analytics.json`
  тодорхойгүй бол) ба зохицуулалт бүрийн дотор хавсралтын блокыг шинэчилнэ (болон,
  боломжтой үед, портал) нэг дамжуулалтаар тэмдэглэл.
- Ажлын жагсаалт эрэмбэлэгдсэн/хуучирсан хэвээр үлдэж, санамж бүр `sns-annex` тэмдэглэгээтэй, хавсралт хавсралт байгаа эсэхийг батлахын тулд `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports` (эсвэл `make check-sns-annex`) ажиллуулна уу. Туслагч нь `artifacts/sns/annex_schedule_summary.json` гэж засаглалын пакетуудад хэрэглэгддэг хэл/хэш хураангуйн хажууд бичдэг.
Энэ нь гараар хуулах/буулгах алхмуудыг арилгаж, SN-8 хавсралтын нотлох баримтыг тогтвортой байлгах болно
CI дахь хамгаалалтын хуваарь, тэмдэглэгээ, нутагшуулах шилжилт.

## 2. Онгоцны иж бүрдэл хэсгүүд

### дагавар утас

- Бүртгэлийн схем + сонгогчийн дүрэм:
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  ба [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md).
- DNS араг ясны туслах:
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  -д баригдсан бэлтгэлийн урсгалтай
  [гарц/DNS runbook](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md).
- Бүртгүүлэгч ажиллуулж байгаа бол доор богино тэмдэглэл бичээрэй
  `docs/source/sns/reports/` сонгогчийн дээж, GAR нотолгоо болон DNS хэшүүдийг нэгтгэн харуулав.

### Үнийн мэдээллийн хуудас

| Шошгоны урт | Суурь хураамж (доллартай тэнцэх) |
|-------------|--------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6–9 | $12 |
| 10+ | $8 |

дагавар коэффициент: `.sora` = 1.0×, `.nexus` = 0.8×, `.dao` = 1.3×.  
Хугацааны үржүүлэгч: 2‑жил −5%, 5‑жил −12%; нигүүлслийн цонх = 30 хоног, гэтэлгэл
= 60 хоног (20% хураамж, хамгийн багадаа 5 доллар, дээд тал нь 200 доллар). -д тохиролцсон хазайлтыг тэмдэглэ
бүртгэгчийн тасалбар.

### Дээд зэрэглэлийн дуудлага худалдаа ба сунгалт

1. **Дээд зэрэглэлийн сан** — битүүмжилсэн тендерийн амлалт/илчлэх (SN-3). Тендерийг дараахаар хянах
   `sns_premium_commit_total` ба манифестыг доор нийтлээрэй
   `docs/source/sns/reports/`.
2. **Голланд дахин нээгдэнэ** — хөнгөлөлт + гэтэлгэлийн хугацаа дууссаны дараа 7 хоногийн Голланд хямдрал эхэлнэ.
   10 × үед энэ нь өдөрт 15% буурдаг. Шошго нь `manifest_id`-ээр илэрдэг тул
   хяналтын самбар нь гадаргуугийн ахиц дэвшлийг харуулж чадна.
3. **Renewals** — `sns_registrar_status_total{resolver="renewal"}` болон монитор
   автоматаар шинэчлэх хяналтын хуудсыг авах (мэдэгдэл, SLA, нөхөн төлбөрийн зам)
   бүртгэгчийн тасалбар дотор.

### Хөгжүүлэгчийн API ба автоматжуулалт

- API гэрээнүүд: [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md).
- Бөөн туслагч ба CSV схем:
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md).
- Жишээ команд:

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --ndjson artifacts/sns/releases/2026q2/requests.ndjson \
  --submission-log artifacts/sns/releases/2026q2/submissions.log \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token
```

KPI хяналтын самбарын шүүлтүүрт манифест ID (`--submission-log` гаралт) оруулах
Тиймээс санхүү нь хувилбар бүрийн орлогын самбарыг нэгтгэх боломжтой.

### Нотлох баримтын багц

1. Холбоо барих хаяг, дагавар хамрах хүрээ, төлбөрийн rails бүхий бүртгэлийн тасалбар.
2. DNS/resolver нотлох баримт (бүсийн араг яс + GAR баталгаа).
3. Үнийн ажлын хуудас + засаглалын баталсан аливаа хүчингүй зүйл.
4. API/CLI утааны туршилтын олдворууд (`curl` дээж, CLI транскрипт).
5. Сар бүрийн хавсралтад хавсаргасан KPI хяналтын самбарын дэлгэцийн агшин + CSV экспорт.

## 3. Хяналтын хуудсыг эхлүүлэх

| Алхам | Эзэмшигч | Олдвор |
|------|-------|----------|
| Хяналтын самбарыг импортолсон | Бүтээгдэхүүний аналитик | Grafana API хариулт + хяналтын самбар UID |
| Портал оруулах баталгаажуулсан | Docs/DevRel | `npm run build` бүртгэлүүд + урьдчилан үзэх дэлгэцийн агшин |
| DNS давтлага дууссан | Сүлжээ/Үйл ажиллагаа | `sns_zonefile_skeleton.py` гаралт + runbook log |
| Бүртгэлийн автоматжуулалтын хуурай гүйлт | Бүртгэгч Eng | `sns_bulk_onboard.py` мэдүүлгийн бүртгэл |
| Засаглалын нотлох баримт бүрдүүлсэн | Засаглалын зөвлөл | Хавсралтын холбоос + Экспортолсон хяналтын самбарын SHA-256 |

Бүртгүүлэгч эсвэл дагаварыг идэвхжүүлэхийн өмнө шалгах хуудсыг бөглөнө үү. Гарын үсэг зурсан
bundle нь SN-8 замын газрын зургийн хаалгыг цэвэрлэж, аудиторуудад хэзээ нэг лавлагаа өгдөг
зах зээлийн нээлтийг хянаж байна.