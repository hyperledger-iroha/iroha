---
lang: mn
direction: ltr
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-12-29T18:16:35.079313+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS сургалтын слайдын загвар

Энэхүү Markdown тойм нь сургагч багш нарын дасан зохицох ёстой слайдуудыг тусгасан болно
тэдний хэлний бүлэг. Эдгээр хэсгийг Keynote/PowerPoint/Google руу хуулна уу
Шаардлагатай бол сумны цэг, дэлгэцийн агшин, диаграммуудыг слайд хийж, нутагшуулна.

## Гарчгийн слайд
- Хөтөлбөр: "Sora Name Service-д элсэх"
- Хадмал: дагавар + мөчлөгийг зааж өгөх (жишээ нь, `.sora — 2026‑03`)
- Илтгэгчид + харьяалал

## KPI чиг баримжаа
- `docs/portal/docs/sns/kpi-dashboard.md` дэлгэцийн агшин эсвэл оруулах
- Суффикс шүүлтүүрийг тайлбарласан сумны жагсаалт, ARPU хүснэгт, хөлдөлт хянагч
- PDF/CSV экспортлох мэдээлэл

## Илэрхий амьдралын мөчлөг
- Диаграм: бүртгэгч → Torii → засаглал → DNS/гарц
- `docs/source/sns/registry_schema.md`-д хамаарах алхамууд
- Тэмдэглэл бүхий манифестын жишээ

## Маргаан болон хөлдөөх дасгалууд
- Асран хамгаалагчийн оролцооны урсгалын диаграмм
- `docs/source/sns/governance_playbook.md` лавлагааны хяналтын хуудас
- Тасалбарын цагийн хуваарийг царцаах жишээ

## Хавсралтын зураг
- `cargo xtask sns-annex ... --portal-entry ...`-г харуулсан командын хэсэг
- `artifacts/sns/regulatory/<suffix>/<cycle>/` доор Grafana JSON-г архивлах сануулга
- `docs/source/sns/reports/.<suffix>/<cycle>.md` холбоос

## Дараагийн алхамууд
- Сургалтын санал хүсэлтийн холбоос (`docs/examples/sns_training_eval_template.md`-г үзнэ үү)
- Slack/Matrix сувгийн бариул
- Удахгүй болох чухал өдрүүд