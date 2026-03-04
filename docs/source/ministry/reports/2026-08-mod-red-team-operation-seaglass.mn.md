---
lang: mn
direction: ltr
source: docs/source/ministry/reports/2026-08-mod-red-team-operation-seaglass.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 64cd1112df2f1fc95571ee4ed269e64bde6bf73bd94b19bbf0eaa80a5b43c219
source_last_modified: "2025-12-29T18:16:35.981105+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill — Operation SeaGlass
summary: Evidence and remediation log for the Operation SeaGlass moderation drill (gateway smuggling, governance replay, alert brownout).
translator: machine-google-reviewed
---

# Red-Team Drill — SeaGlass ажиллагаа

- **Өрмийн ID:** `20260818-operation-seaglass`
- **Огноо, цонх:** `2026-08-18 09:00Z – 11:00Z`
- **Хувилбарын анги:** `smuggling`
- **Операторууд:** `Miyu Sato, Liam O'Connor`
- **Хяналтын самбарыг үйлдлээс царцаасан:** `364f9573b`
- **Нотлох баримтын багц:** `artifacts/ministry/red-team/2026-08/operation-seaglass/`
- **SoraFS CID (заавал биш):** `not pinned (local bundle only)`
- **Холбогдох замын зураглал:** `MINFO-9`, дээр нь `MINFO-RT-17` / `MINFO-RT-18` холбоотой дагалт.

## 1. Зорилтууд ба Элсэлтийн нөхцөл

- **Үндсэн зорилтууд**
  - Хууль бусаар хил нэвтрүүлэх оролдлого хийх үед ачааллыг бууруулах дохиоллын үеэр TTL-ийн хэрэгжилтийг үгүйсгэх, гарцын хорио цээрийг баталгаажуулах.
  - Зохицуулах ажлын дэвтэрт засаглалын дахин тоглуулах илрүүлэлтийг баталгаажуулж, харьцах ажиллагааг анхааруулна.
- ** Урьдчилсан нөхцөлийг баталгаажуулсан**
  - `emergency_canon_policy.md` хувилбар `v2026-08-seaglass`.
  - `dashboards/grafana/ministry_moderation_overview.json` digest `sha256:ef5210b5b08d219242119ec4ceb61cb68ee4e42ce2eea8a67991fbff95501cc8`.
  - Дуудлагын дагуу эрх мэдлийг хүчингүй болгох: `Kenji Ito (GovOps pager)`.

## 2. Гүйцэтгэх хугацаа

| Цагийн тэмдэг (UTC) | Жүжигчин | Үйлдэл / Тушаал | Үр дүн / Тайлбар |
|----------------|-------|------------------|----------------|
| 09:00:12 | Мию Сато | `364f9573b` дээрх `scripts/ministry/export_red_team_evidence.py --freeze-only`-ээр дамжуулан хяналтын самбар/сануулгыг царцаасан | Суурь үзүүлэлтийг `dashboards/` | доор авч хадгална
| 09:07:44 | Лиам О'Коннор | Нийтэлсэн үгүйсгэх жагсаалтын агшин зураг + `sorafs_cli ... gateway update-denylist --policy-tier emergency`-тай үе шат руу оруулах GAR хүчингүй болгох | Зургийг хүлээн зөвшөөрсөн; Alertmanager дээр бичигдсэн цонхыг дарах |
| 09:17:03 | Мию Сато | `moderation_payload_tool.py --scenario seaglass` ашиглан тарьсан хууль бус наймааны ачаалал + засаглалын давталт | 3м12 секундын дараа дохио өгсөн; засаглалын давталтыг дарцагласан |
| 09:31:47 | Лиам О'Коннор | Нотлох баримтыг экспорт хийж, битүүмжилсэн манифест `seaglass_evidence_manifest.json` | `manifests/` | доор хадгалагдсан нотлох баримтын багц болон хэшүүд

## 3. Ажиглалт ба хэмжигдэхүүн

| Метрик | Зорилтот | Ажигласан | Дамжаагүй/Бүтэлгүйтсэн | Тэмдэглэл |
|--------|--------|----------|-----------|-------|
| Сэрэмжлүүлгийн хариу саатал | = 0.98 | 0.992 | ✅ | Хууль бусаар хил нэвтрүүлэх, дахин ачаалах ачааг хоёуланг нь илрүүлсэн |
| Гарцын гажиг илрүүлэх | Анхааруулга тавьсан | Сэрэмжлүүлэг асаасан + автомат хорио цээрийн дэглэм | ✅ | Дахин оролдох төсөв дуусахаас өмнө хорио цээрийн дэглэм тогтоосон |

- `Grafana export:` `artifacts/ministry/red-team/2026-08/operation-seaglass/dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/ministry/red-team/2026-08/operation-seaglass/alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `artifacts/ministry/red-team/2026-08/operation-seaglass/manifests/seaglass_evidence_manifest.json`

## 4. Олдворууд ба засвар

| Хүнд байдал | Олж байна | Эзэмшигч | Зорилтот огноо | Статус / Холбоос |
|----------|---------|-------|-------------|---------------|
| Өндөр | Засаглалын дахин тоглуулах сэрэмжлүүлэг идэвхжсэн боловч хүлээлгийн жагсаалт идэвхжсэн үед SoraFS битүүмжлэл 2 м-ээр хойшлогдсон | Засаглалын үйл ажиллагаа (Лиам О'Коннор) | 2026-09-05 | `MINFO-RT-17` нээлттэй — дахин тоглуулах тамга автоматжуулалтыг дамжих замд нэмэх |
| Дунд | Хяналтын самбарыг SoraFS дээр тогтоогоогүй; операторууд орон нутгийн багцад тулгуурласан | Ажиглалт (Мию Сато) | 2026-08-25 | `MINFO-RT-18` нээлттэй — `dashboards/*`-аас SoraFS хүртэл дараагийн дасгалын өмнө гарын үсэг зурсан CID зүү |
| Бага | CLI бүртгэлийн дэвтэр эхний дамжуулалтад Norito манифест хэшийг орхигдуулсан | Яамны үйл ажиллагаа (Кенжи Ито) | 2026-08-22 | Өрөмдлөгийн явцад бэхлэгдсэн; загварын дэвтэрт шинэчлэгдсэн |Тохируулга хэрхэн илрэх, жагсаалтаас хасах бодлого эсвэл SDK/хэрэгсэл өөрчлөгдөх ёстойг баримтжуулна уу. GitHub/Jira-тай холбогдож, блоклосон/блоклосон төлөвийг тэмдэглэ.

## 5. Засаглал ба Зөвшөөрөл

- ** Ослын командлагчийн гарын үсэг:** `Miyu Sato @ 2026-08-18T11:22Z`
- **Засаглалын зөвлөлийг хянан шалгах огноо:** `GovOps-2026-08-22`
- **Дараах шалгах хуудас:** `[x] status.md updated`, `[x] roadmap row updated`, `[x] transparency packet annotated`

## 6. Хавсралт

- `[x] CLI logbook (logs/operation_seaglass.log)`
- `[x] Dashboard JSON export`
- `[x] Alertmanager history`
- `[x] SoraFS manifest / CAR`
- `[ ] Override audit log`

Хавсралт бүрийг нотлох баримтын багц болон SoraFS агшинд байршуулсны дараа `[x]` гэж тэмдэглэнэ үү.

---

_Хамгийн сүүлд шинэчлэгдсэн: 2026-08-18_