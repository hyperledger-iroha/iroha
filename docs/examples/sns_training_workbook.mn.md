---
lang: mn
direction: ltr
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-12-29T18:16:35.080069+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS сургалтын ажлын дэвтрийн загвар

Энэ дасгалын номыг сургалтын бүлэг тус бүрийн каноник тараах материал болгон ашиглаарай. Солих
Оролцогчдод тараахаас өмнө орлуулагч (`<...>`).

## Сеансын дэлгэрэнгүй
- дагавар: `<.sora | .nexus | .dao>`
- Цикл: `<YYYY-MM>`
- Хэл: `<ar/es/fr/ja/pt/ru/ur>`
- Чиглүүлэгч: `<name>`

## Лаборатори 1 — KPI экспорт
1. Порталын KPI хяналтын самбарыг нээнэ үү (`docs/portal/docs/sns/kpi-dashboard.md`).
2. `<suffix>` дагавар болон `<window>` цагийн хязгаараар шүүнэ.
3. PDF + CSV хормын хувилбаруудыг экспортлох.
4. Экспортолсон JSON/PDF-ийн SHA-256-г энд бичнэ үү: `______________________`.

## Лаборатори 2 — Манифест дасгал
1. `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json`-аас дээжийн манифестыг татаж авна уу.
2. `cargo run --bin sns_manifest_check -- --input <file>`-р баталгаажуулна уу.
3. `scripts/sns_zonefile_skeleton.py`-тай резолюторын араг ясыг үүсгэ.
4. Ялгааны хураангуйг буулгана уу:
   ```
   <git diff output>
   ```

## Лаборатори 3 — Маргааны загварчлал
1. Хөлдөлтийг эхлүүлэхийн тулд асран хамгаалагч CLI-г ашиглана уу (тохиолдлын ID `<case-id>`).
2. Маргааны хэшийг тэмдэглэ: `______________________`.
3. Нотлох баримтын бүртгэлийг `artifacts/sns/training/<suffix>/<cycle>/logs/` руу оруулна уу.

## Лаборатори 4 — Хавсралтын автоматжуулалт
1. Grafana хяналтын самбарыг JSON-г экспортлоод `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json` руу хуулна уу.
2. Ажиллуулах:
   ```bash
   cargo xtask sns-annex \
     --suffix <suffix> \
     --cycle <cycle> \
     --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --output docs/source/sns/reports/<suffix>/<cycle>.md \
     --regulatory-entry docs/source/sns/regulatory/<memo>.md \
     --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```
3. Хавсралтын зам + SHA-256 гаралтыг буулгана уу: `________________________________`.

## Санал хүсэлтийн тэмдэглэл
-Юу нь тодорхойгүй байсан бэ?
- Цаг хугацаа өнгөрөхөд ямар лаборатори ажиллаж байсан бэ?
- Багажны алдаа ажиглагдсан уу?

Дууссан ажлын номыг сургагч багшид буцааж өгөх; дор харьяалагддаг
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`.