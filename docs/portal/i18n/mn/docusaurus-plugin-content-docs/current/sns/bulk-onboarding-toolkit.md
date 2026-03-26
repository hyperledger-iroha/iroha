---
lang: mn
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
id: бөөнөөр нь суулгах хэрэгсэл
гарчиг: SNS Bulk Onboarding Toolkit
sidebar_label: Бөөнөөр суулгах хэрэгсэл
тайлбар: SN-3b бүртгэгчийн ажиллуулдаг CSV-н RegisterNameRequestV1 автоматжуулалт.
---

::: Каноник эх сурвалжийг анхаарна уу
`docs/source/sns/bulk_onboarding_toolkit.md` толин тусгалуудыг гадны операторууд хардаг
репозиторыг хувилахгүйгээр ижил SN-3b удирдамж.
:::

# SNS Bulk Onboarding Toolkit (SN-3b)

**Замын зургийн лавлагаа:** SN-3b "Бөөнөөр суулгах хэрэгсэл"  
** Олдворууд:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Томоохон бүртгэгчид олон зуун `.sora` эсвэл `.nexus` бүртгэлийг урьдчилан хийдэг.
ижил засаглалын зөвшөөрөл, тооцооны төмөр замтай. JSON-г гараар бүтээх
Ачаалал их эсвэл CLI-г дахин ажиллуулах нь масштабтай байдаггүй тул SN-3b нь тодорхойлогчийг илгээдэг.
`RegisterNameRequestV1` бүтцийг бэлтгэдэг CSV-аас Norito барилгачин
Torii эсвэл CLI. Туслагч урд талын мөр бүрийг баталгаажуулж, аль алиныг нь ялгаруулдаг
нэгтгэсэн манифест болон нэмэлт шинэ шугамаар тусгаарлагдсан JSON-г илгээх боломжтой
Аудитын зохион байгуулалттай төлбөрийн баримтыг бүртгэх явцад ачааллыг автоматаар гүйцэтгэдэг.

## 1. CSV схем

Шинжилгээнд дараах толгойн мөр шаардлагатай (захиалга нь уян хатан):

| Багана | Шаардлагатай | Тодорхойлолт |
|--------|----------|-------------|
| `label` | Тийм | Хүссэн шошго (холимог тохиолдлыг хүлээн зөвшөөрсөн; багажийг Норм v1 болон UTS-46-д тохируулсан). |
| `suffix_id` | Тийм | Тоон дагавар танигч (аравтын буюу `0x` hex). |
| `owner` | Тийм | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | Тийм | Бүхэл тоо `1..=255`. |
| `payment_asset_id` | Тийм | Төлбөр тооцооны хөрөнгө (жишээ нь `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Тийм | Хөрөнгийн эх нэгжийг төлөөлөх тэмдэггүй бүхэл тоо. |
| `settlement_tx` | Тийм | Төлбөрийн гүйлгээ эсвэл хэшийг тодорхойлсон JSON утга эсвэл шууд утга. |
| `payment_payer` | Тийм | Төлбөрийг зөвшөөрсөн дансны дугаар. |
| `payment_signature` | Тийм | Даамал эсвэл төрийн сангийн гарын үсгийн нотолгоог агуулсан JSON эсвэл үгийн үсэг. |
| `controllers` | Нэмэлт | Хянагчийн дансны хаягуудын цэг таслал эсвэл таслалаар тусгаарлагдсан жагсаалт. Орхигдуулсан тохиолдолд өгөгдмөл нь `[owner]`. |
| `metadata` | Нэмэлт | Inline JSON эсвэл `@path/to/file.json` нь шийдэгчийн зөвлөмж, TXT бичлэг гэх мэтийг өгдөг. `{}`-ийн өгөгдмөл. |
| `governance` | Нэмэлт | Inline JSON эсвэл `@path` `GovernanceHookV1`-г зааж байна. `--require-governance` энэ баганыг хэрэгжүүлдэг. |

Аль ч багана нь нүдний утгыг `@`-ээр угтвар болгон гадаад файлыг лавлаж болно.
Замууд нь CSV файлтай харьцуулахад шийдэгддэг.

## 2. Туслагчийг ажиллуулах

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Гол сонголтууд:

- `--require-governance` нь засаглалын дэгээгүй мөрүүдийг үгүйсгэдэг (хэрэгтэй
  дээд зэрэглэлийн дуудлага худалдаа эсвэл нөөц даалгавар).
- `--default-controllers {owner,none}` хянагч нүднүүд хоосон эсэхийг шийддэг
  эзэмшигчийн данс руу буцах.
- `--controllers-column`, `--metadata-column`, `--governance-column` нэрийг өөрчлөх
  дээд талын экспорттой ажиллахдаа нэмэлт багана.

Амжилттай болсны дараа скрипт нь нэгтгэсэн манифест бичдэг:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "soraカタカナ...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"soraカタカナ...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"soraカタカナ...",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```

Хэрэв `--ndjson` өгөгдвөл `RegisterNameRequestV1` бүрийг мөн адил бичнэ.
автоматжуулалт нь хүсэлтийг шууд дамжуулах боломжтой нэг мөр JSON баримт бичиг юм
Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Автоматжуулсан мэдүүлэг

### 3.1 Torii АМРАХ горим

`--submit-torii-url` дээр нэмэх нь `--submit-token` эсвэл
`--submit-token-file` манифест бүрийг Torii руу шууд оруулах:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Туслагч хүсэлт болгонд нэг `POST /v1/sns/names` гаргаад цуцлах болно
  Эхний HTTP алдаа. Хариултуудыг бүртгэлийн замд NDJSON гэж хавсаргасан
  бичлэгүүд.
- `--poll-status` тус бүрийн дараа `/v1/sns/names/{namespace}/{literal}`-г дахин асуудаг.
  Бичлэг байгаа эсэхийг баталгаажуулахын тулд илгээх (`--poll-attempts` хүртэл, анхдагч 5)
  харагдахуйц. `--suffix-map` (`suffix_id`-аас `"suffix"` хүртэлх JSON утгууд) -ийг өгнө.
  хэрэгсэл нь санал асуулгад зориулж `{label}.{suffix}` литералуудыг гаргаж авах боломжтой.
- Тохируулах: `--submit-timeout`, `--poll-attempts`, `--poll-interval`.

### 3.2 iroha CLI горим

Манифест оруулга бүрийг CLI-ээр дамжуулахын тулд хоёртын замыг оруулна уу:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Хянагч нь `Account` оролттой байх ёстой (`controller_type.kind = "Account"`)
  Учир нь CLI одоогоор зөвхөн бүртгэлд суурилсан хянагчдыг ил гаргаж байна.
- Мета өгөгдөл болон засаглалын блокуудыг хүсэлт болгон түр зуурын файлд бичдэг
  `iroha sns register --metadata-json ... --governance-json ...` руу дамжуулсан.
- CLI stdout болон stderr plus гарах кодыг бүртгэсэн; тэг биш гарах кодууд цуцлагдана
  гүйлт.

Бүртгэлийн хоёр горим хоёулаа хамт ажиллаж (Torii ба CLI) бүртгэгчийг шалгах боломжтой.
байршуулалт эсвэл нөөцийг давтах.

### 3.3 Өргөдлийн баримт

`--submission-log <path>` өгөгдсөн үед скрипт нь NDJSON оруулгуудыг хавсаргана
барьж авах:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Амжилттай Torii хариултууд нь задалсан бүтэцтэй талбаруудыг агуулдаг
`NameRecordV1` эсвэл `RegisterNameResponseV1` (жишээ нь `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) тул хяналтын самбар ба засаглал
тайлангууд нь чөлөөт хэлбэрийн текстийг шалгахгүйгээр логийг задлан шинжлэх боломжтой. Энэ бүртгэлийг хавсаргана уу
хуулбарлах нотлох баримтын манифестын хамт бүртгэгчийн тасалбар.

## 4. Docs портал хувилбарын автоматжуулалт

CI болон портал ажлын байруудыг ороосон `docs/portal/scripts/sns_bulk_release.sh` гэж нэрлэдэг
Туслагч нь `artifacts/sns/releases/<timestamp>/` дор олдворуудыг хадгалдаг:

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

Скрипт:

1. `registrations.manifest.json`, `registrations.ndjson`-г бүтээж, хуулбарлана
   хувилбарын лавлах руу эх CSV.
2. Torii ба/эсвэл CLI (тохируулсан үед) ашиглан манифест илгээх, бичих
   Дээрх бүтэцтэй баримттай `submissions.log`.
3. Хувилбарыг тодорхойлсон `summary.json` ялгаруулдаг (замууд, Torii URL, CLI зам,
   цаг тэмдэг) тиймээс портал автоматжуулалт нь багцыг олдворын санд байршуулах боломжтой.
4. агуулсан `metrics.prom` (`--metrics`-ээр дарах) үйлдвэрлэдэг.
   Нийт хүсэлтийн Prometheus форматын тоолуур, дагавар хуваарилалт,
   хөрөнгийн нийт дүн, ирүүлсэн үр дүн. Хураангуй JSON нь энэ файлтай холбогддог.

Ажлын урсгалууд нь хувилбарын лавлахыг нэг олдвор хэлбэрээр архивлаж байгаа бөгөөд одоо байгаа
аудит хийхэд засаглалын шаардлагатай бүх зүйлийг агуулсан.

## 5. Телеметр ба хяналтын самбар

`sns_bulk_release.sh`-ийн үүсгэсэн хэмжүүрийн файл нь дараахь зүйлийг харуулж байна
цуврал:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

`metrics.prom`-г Prometheus хажуу тэрэг рүү (жишээ нь Promtail эсвэл
багц импортлогч) бүртгэгчид, няравууд болон засаглалын үе тэнгийнхнийг ижил түвшинд байлгах
их хэмжээний ахиц дэвшил. Grafana самбар
`dashboards/grafana/sns_bulk_release.json` нь ижил өгөгдлийг самбартай харуулдаг
дагавар бүрийн тоо, төлбөрийн хэмжээ, илгээх амжилт/бүтэлгүйтлийн харьцаа.
Удирдах зөвлөл нь `release`-ээр шүүдэг тул аудиторууд нэг CSV-г өрөмдөж болно.

## 6. Баталгаажуулалт ба алдааны горимууд

- **Шошгоны каноникчилал:** оролтыг Python IDNA plus-аар хэвийн болгосон
  жижиг үсэг болон Норм v1 тэмдэгт шүүлтүүр. Хүчингүй шошго нь аль нэгний өмнө хурдан бүтэлгүйтдэг
  сүлжээний дуудлага.
- **Тоон хашлага:** дагавар id, хугацааны жил, үнийн зөвлөмжүүд буурах ёстой
  `u16` болон `u8` хязгаар дотор. Төлбөрийн талбарууд аравтын бутархай эсвэл зургаан өнцөгт бүхэл тоог хүлээн зөвшөөрдөг
  `i64::MAX` хүртэл.
- **Мета өгөгдөл эсвэл засаглалын задлан шинжилгээ:** inline JSON-г шууд задлан шинжилдэг; файл
  лавлагаа нь CSV байршилтай харьцуулахад шийдэгддэг. Объект бус мета өгөгдөл
  баталгаажуулалтын алдаа гаргадаг.
- **Хянагч:** хоосон нүднүүд нь `--default-controllers`. Тодорхой өгөх
  эзэмшигч бус этгээдэд шилжүүлэх үед хянагчийн жагсаалт (жишээ нь `soraカタカナ...;soraカタカナ...`)
  жүжигчид.

Алдаа дутагдлыг контекст мөрийн дугаараар мэдээлдэг (жишээ нь
`error: row 12 term_years must be between 1 and 255`). скрипт нь гарч байна
баталгаажуулалтын алдаан дээр `1` код, CSV зам байхгүй үед `2`.

## 7. Туршилт ба гарал үүсэл

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` нь CSV задлан шинжилгээг хамардаг,
  NDJSON ялгаруулалт, засаглалын хэрэгжилт, CLI эсвэл Torii илгээлт
  замууд.
- Туслах нь цэвэр Python (нэмэлт хамааралгүй) бөгөөд хаана ч ажилладаг
  `python3` боломжтой. Үйлдлийн түүхийг CLI-ийн хажууд хянадаг
  нөхөн үржихүйн үндсэн агуулах.

Үйлдвэрлэлийн хувьд үүсгэсэн манифест болон NDJSON багцыг
бүртгэлийн тасалбар, ингэснээр няравууд илгээсэн ачааллыг яг таг дахин тоглуулах боломжтой
Torii хүртэл.