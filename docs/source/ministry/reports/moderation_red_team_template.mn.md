---
lang: mn
direction: ltr
source: docs/source/ministry/reports/moderation_red_team_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 22bfdf5696bf3a58e7899e7d7b2ba77e404a05fa81304f12d6c78eeb1e8035e5
source_last_modified: "2025-12-29T18:16:35.982646+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill Report Template
summary: Copy this file for every MINFO-9 drill to capture metadata, evidence, and remediation actions.
translator: machine-google-reviewed
---

> **Хэрхэн ашиглах вэ:** Өрөмдлөг бүрийн дараа энэ загварыг `docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md` болгон хуулбарлана уу. Файлын нэрийг жижиг үсгээр, зураасаар зурж, Alertmanager-д нэвтэрсэн өрөмдлөгийн ID-тай зэрэгцүүлэн үлдээгээрэй.

# Улаан-Багийн Өрмийн тайлан — `<SCENARIO NAME>`

- **Өрмийн ID:** `<YYYYMMDD>-<scenario>`
- **Огноо, цонх:** `<YYYY-MM-DD HH:MMZ – HH:MMZ>`
- **Хувилбарын анги:** `smuggling | bribery | gateway | ...`
- **Операторууд:** `<names / handles>`
- **Хяналтын самбарыг үйлдлээс царцаасан:** `<git SHA>`
- **Нотлох баримтын багц:** `artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`
- **SoraFS CID (заавал биш):** `<cid>`  
- **Холбогдох замын зураглал:** `MINFO-9`, дээр нь холбоотой тасалбарууд.

## 1. Зорилтууд ба Элсэлтийн нөхцөл

- **Үндсэн зорилтууд**
  - `<e.g. Verify denylist TTL enforcement under smuggling attack>`
- ** Урьдчилсан нөхцөлийг баталгаажуулсан**
  - `emergency_canon_policy.md` хувилбар `<tag>`
  - `dashboards/grafana/ministry_moderation_overview.json` digest `<sha256>`
  - Дуудлагын дагуу эрх мэдлийг хүчингүй болгох: `<name>`

## 2. Гүйцэтгэх хугацаа

| Цагийн тэмдэг (UTC) | Жүжигчин | Үйлдэл / Тушаал | Үр дүн / Тайлбар |
|----------------|-------|------------------|----------------|
|  |  |  |  |

> Torii хүсэлтийн ID, бөөн хэш, хүчингүй болгох зөвшөөрөл, Alertmanager холбоосыг оруулна уу.

## 3. Ажиглалт ба хэмжигдэхүүн

| Метрик | Зорилтот | Ажигласан | Давсан/Бүтэлгүйтсэн | Тэмдэглэл |
|--------|--------|----------|-----------|-------|
| Сэрэмжлүүлгийн хариу саатал | `<X> min` | `<Y> min` | ✅/⚠️ |  |
| Модерацийг илрүүлэх түвшин | `>= <value>` |  |  |  |
| Гарцын гажиг илрүүлэх | `Alert fired` |  |  |  |

- `Grafana export:` `artifacts/.../dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/.../alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `<path>`

## 4. Олдворууд ба засвар

| Хүнд байдал | Олж байна | Эзэмшигч | Зорилтот огноо | Статус / Холбоос |
|----------|---------|-------|-------------|---------------|
| Өндөр |  |  |  |  |

Тохируулга хэрхэн илрэх, жагсаалтаас хасах бодлого эсвэл SDK/хэрэгсэл өөрчлөгдөх ёстойг баримтжуулна уу. GitHub/Jira-тай холбогдож, блоклосон/блоклосон төлөвийг тэмдэглэ.

## 5. Засаглал ба Зөвшөөрөл

- **Осол гарсан командлагчийн гарын үсэг:** `<name / timestamp>`
- **Засаглалын зөвлөлийг хянан шалгах огноо:** `<meeting id>`
- **Дараах шалгах хуудас:** `[ ] status.md updated`, `[ ] roadmap row updated`, `[ ] transparency packet annotated`

## 6. Хавсралт

- `[ ] CLI logbook (`logs/.md`)`
- `[ ] Dashboard JSON export`
- `[ ] Alertmanager history`
- `[ ] SoraFS manifest / CAR`
- `[ ] Override audit log`

Хавсралт бүрийг нотлох баримтын багц болон SoraFS агшин зуурт байршуулсны дараа `[x]` гэж тэмдэглэнэ үү.

---

_Сүүлд шинэчлэгдсэн: {{огноо | анхдагч("2026-02-20") }}_