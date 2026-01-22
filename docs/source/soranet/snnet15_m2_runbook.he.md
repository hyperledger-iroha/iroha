---
lang: he
direction: rtl
source: docs/source/soranet/snnet15_m2_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b3ed72902eb7308805e391c1d03a36f88429093b4edf9f509b445dbb3253d9f6
source_last_modified: "2026-01-30T13:33:43.991472+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/soranet/snnet15_m2_runbook.md -->

# SNNet-15M2 — נוהל בטא לשער

חבילת הבטא M2 שולחת את CDN שער SoraGlobal עם תצוגת DoQ/ODoH, אימות CAR חסר‑אמון,
ייצואי תאימות GAR, וראיות הקשחה.

## צעדים
- יצירת חבילת בטא: `cargo xtask soranet-gateway-m2 --config configs/soranet/gateway_m2/beta_config.json --output-dir artifacts/soranet/gateway_m2/beta`
- אימות ארטיפקטים:
  - `gateway_m2_summary.json` מציין כל PoP עם תצורת edge בטא, נתיב מאמת חסר‑אמון,
    סיכום מוכנות PQ, סיכום הקשחה וחבילת ops.
  - סעיף החיוב נושא `invoice`/`ledger_projection` ומאכף `allow_hard_cap=true`.
  - ייצוא תאימות נמצא ב‑`compliance_summary.{json,md}` שנבנה מ‑GAR receipts/ACKs.
- מוכנות PQ לכל PoP: ספקו `--srcv2`, `--tls-bundle`, ו‑`--trustless-config` בתצורה
  כדי להפיק סיכומי `pq/` תחת כל `pops/<pop>/beta/`.
- הקשחה: אספקת `sbom`, `vuln_report`, `hsm_policy`, ו‑`sandbox_profile` מפיקה
  `gateway_hardening_summary.{json,md}` עם איתות שימור (מזהיר אם >30 ימים).

## פלטים
- `pops/<label>/gateway_edge_beta.yaml` — תצורת H3 + DoQ + ODoH preview עם binding של מאמת חסר‑אמון.
- `billing/` — חשבוניות JSON/CSV/Parquet בתוספת ledger projection וסכומים.
- `compliance_summary.{json,md}` — rollup של אכיפת GAR.
- `gateway_m2_summary.json` — שורש ראיות קנוני לחבילות ממשל.

</div>
