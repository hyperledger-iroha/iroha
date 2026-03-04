---
lang: uz
direction: ltr
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-12-29T18:16:35.079313+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS trening slayd shabloni

Ushbu Markdown konturi fasilitatorlar moslashishi kerak bo'lgan slaydlarni aks ettiradi
ularning til kogortalari. Ushbu bo'limlarni Keynote/PowerPoint/Google-ga nusxalang
Belgilangan nuqtalarni, ekran tasvirlarini va diagrammalarni kerak bo'lganda slaydlar va mahalliylashtirish.

## Sarlavhali slayd
- Dastur: "Sora Name xizmatini ishga tushirish"
- Subtitr: suffiks + siklni belgilang (masalan, `.sora — 2026‑03`)
- Taqdimotchilar + mansublik

## KPI yo'nalishi
- `docs/portal/docs/sns/kpi-dashboard.md` skrinshoti yoki o'rnatilishi
- Suffiks filtrlarini tushuntiruvchi o'qlar ro'yxati, ARPU jadvali, muzlatish kuzatuvchisi
- PDF/CSV eksporti uchun qo'ng'iroqlar

## Manifest hayot aylanishi
- Diagramma: registrator → Torii → boshqaruv → DNS/shlyuz
- `docs/source/sns/registry_schema.md`-ga havola qilingan qadamlar
- Annotatsiyalar bilan manifest ko'chirmasiga misol

## Mashqlarni bahslash va muzlatish
- Vasiyning aralashuvi uchun oqim diagrammasi
- `docs/source/sns/governance_playbook.md` havolasi bo'yicha nazorat ro'yxati
- Chipta vaqt jadvalini muzlatishga misol

## Ilovani suratga olish
- `cargo xtask sns-annex ... --portal-entry ...` ko'rsatilgan buyruq parchasi
- `artifacts/sns/regulatory/<suffix>/<cycle>/` ostida Grafana JSON-ni arxivlash uchun eslatma
- `docs/source/sns/reports/.<suffix>/<cycle>.md` ga havola

## Keyingi qadamlar
- Trening bo'yicha fikr-mulohaza havolasi (qarang `docs/examples/sns_training_eval_template.md`)
- Slack/Matrix kanal tutqichlari
- Kelgusi muhim bosqich sanalari