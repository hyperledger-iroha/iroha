---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f0e832438aa843ed1ce6d018055a67bd1688ccb4cead9f73b87033469256015d
source_last_modified: "2026-01-22T16:26:46.526983+00:00"
translation_last_reviewed: 2026-02-07
title: Reserve Ledger Digest & Dashboards
description: How to turn `sorafs reserve ledger` output into telemetry, dashboards, and alerts for the Reserve+Rent policy.
translator: machine-google-reviewed
---

Нөөц+Түрээсийн бодлого (замын зураглалын зүйл **SFM‑6**) одоо `sorafs reserve`-г нийлүүлж байна
CLI туслахууд дээр нэмэх нь `scripts/telemetry/reserve_ledger_digest.py` орчуулагч
Төрийн сангийн гүйлгээ нь тодорхойлогдсон түрээс/нөөцийн шилжүүлгийг гаргаж чаддаг. Энэ хуудас толин тусгал
ажлын урсгалыг `docs/source/sorafs_reserve_rent_plan.md`-д тодорхойлсон бөгөөд тайлбарлав
Шинэ шилжүүлгийн хангамжийг Grafana + Alertmanager руу хэрхэн холбох вэ, ингэснээр эдийн засаг болон
засаглалын хянагч нар тооцооны мөчлөг бүрт аудит хийх боломжтой.

## Төгсгөлийн ажлын урсгал

1. **Үнийн санал + дэвтэр проекц**
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

  sorafs reserve ledger \
    --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
    --provider-account <katakana-i105-account-id> \
    --treasury-account <katakana-i105-account-id> \
    --reserve-account <katakana-i105-account-id> \
    --asset-definition 61CtjvNd9T3THAR65GsMVHr82Bjc \
    --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   Бүртгэлийн туслах нь `ledger_projection` блок (түрээсийн хугацаа, нөөц) хавсаргана.
   хомсдол, цэнэглэх дельта, андеррайтинг логик) дээр нэмэх нь Norito `Transfer`
   ISI-ууд XOR-г төрийн сан болон нөөцийн данс хооронд шилжүүлэх шаардлагатай байв.

2. **Дижест + Prometheus/NDJSON гаралтыг үүсгэх**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   Хоол боловсруулах туслагч нь микро-XOR-ын нийлбэрийг XOR болгон хэвийн болгож, байгаа эсэхийг бүртгэдэг
   проекц нь андеррайтингтэй таарч, **шилжүүлэх тэжээл** хэмжигдэхүүнийг ялгаруулдаг
   `sorafs_reserve_ledger_transfer_xor` ба
   `sorafs_reserve_ledger_instruction_total`. Олон дэвтэр байх шаардлагатай үед
   боловсруулсан (жишээ нь, үйлчилгээ үзүүлэгчийн багц), `--ledger`/`--label` хосуудыг давтаж,
   Туслагч нь дижест бүрийг агуулсан ганц NDJSON/Prometheus файл бичдэг.
   Хяналтын самбар нь захиалгаар цавуу хэрэглэхгүйгээр бүхэл бүтэн циклийг шингээдэг. `--out-prom`
   файл нь зангилаа экспортлогч текст файл цуглуулагчийг чиглүүлдэг—`.prom` файлыг оруулаарай.
   экспортлогчийн үзсэн лавлах эсвэл телеметрийн хувин руу байршуулна уу
   `--ndjson-out` ижил тэжээгддэг бол Нөөцийн хяналтын самбарын ажил хэрэглэдэг.
   өгөгдлийн дамжуулах хоолой руу ачааллыг .

3. **Одвор + нотлох баримтыг нийтлэх**
   - `artifacts/sorafs_reserve/ledger/<provider>/` доор дижестийг хадгалж, холбоно уу
     Таны долоо хоног тутмын эдийн засгийн тайлангийн Markdown-ийн хураангуй.
   - JSON тоймыг түрээсийн шаталтад хавсаргана (ингэснээр аудиторууд
     математик) ба хяналтын нийлбэрийг засаглалын нотлох баримтын багцад оруулна.
   - Хэрэв дайжест нь цэнэглэх эсвэл андеррайтерийн зөрчлийн дохио байвал сэрэмжлүүлгийг лавлана уу
     ID (`SoraFSReserveLedgerTopUpRequired`,
     `SoraFSReserveLedgerUnderwritingBreach`) ба ямар дамжуулалт ISI байсныг тэмдэглэ
     хэрэглэсэн.

## Метрик → хяналтын самбар → анхааруулга

| Эх сурвалжийн хэмжүүр | Grafana самбар | Анхааруулга / бодлогын дэгээ | Тэмдэглэл |
|-------------|---------------|---------------------|-------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | `dashboards/grafana/sorafs_capacity_health.json` дахь “DA Rent Distribution (XOR/цаг)” | Долоо хоног тутмын төрийн сангийн тоймыг тэжээх; нөөцийн урсгалын огцом өсөлт нь `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`) руу тархдаг. |
| `torii_da_rent_gib_months_total` | “Хүчин чадлын ашиглалт (ГиБ-сар)” (ижил хяналтын самбар) | Нэхэмжлэхийн хадгалалт нь XOR шилжүүлэгтэй тохирч байгааг нотлохын тулд дэвтэр тоймтой хослуулаарай. |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, `sorafs_reserve_ledger_top_up_shortfall_xor` | `dashboards/grafana/sorafs_reserve_economics.json` доторх "Нөөцийн агшин зуурын зураг (XOR)" + төлөвийн картууд | `SoraFSReserveLedgerTopUpRequired` `requires_top_up=1` үед асах; `SoraFSReserveLedgerUnderwritingBreach` `meets_underwriting=0` үед асна. |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | `dashboards/grafana/sorafs_reserve_economics.json` доторх "Төрлөөр шилжүүлэг", "Хамгийн сүүлийн үеийн шилжүүлгийн задаргаа" болон хамрах хүрээний картууд | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing`, `SoraFSReserveLedgerTopUpTransferMissing` нь түрээс/нэмэлт шаардлагатай байсан ч шилжүүлгийн хангамж байхгүй эсвэл тэглэгдсэн тохиолдолд анхааруулдаг; Хамрах хүрээний картууд ижил тохиолдолд 0% хүртэл буурдаг. |

Түрээсийн мөчлөг дуусахад Prometheus/NDJSON агшин агшныг шинэчилж, баталгаажуулна уу.
Grafana хавтангууд нь шинэ `label`-ийг авч, дэлгэцийн агшинг хавсаргана +
Түрээсийн засаглалын багцын дохиоллын менежер ID. Энэ нь CLI төсөөллийг баталж байна.
телеметр, засаглалын олдворууд бүгд **ижил** дамжуулалтын тэжээл болон
Замын зургийн эдийн засгийн хяналтын самбарыг Нөөц+Түрээстэй нийцүүлэн байлгадаг
автоматжуулалт. Хамрах хүрээний картууд нь 100% (эсвэл 1.0) болон шинэ сэрэмжлүүлгийг унших ёстой
Түрээсийн төлбөр болон нөөц цэнэглэх шилжүүлэг мэдээллийн санд байгаа бол түүнийг арилгах ёстой.