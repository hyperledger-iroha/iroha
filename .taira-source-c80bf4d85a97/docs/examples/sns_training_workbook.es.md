---
lang: es
direction: ltr
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-11-15T09:17:29.843566+00:00"
translation_last_reviewed: 2026-01-01
---

# Plantilla de cuaderno de entrenamiento SNS

Usa este cuaderno como el handout canonico para cada cohorte de entrenamiento. Reemplaza los placeholders (`<...>`) antes de distribuirlo a los asistentes.

## Detalles de la sesion
- Sufijo: `<.sora | .nexus | .dao>`
- Ciclo: `<YYYY-MM>`
- Idioma: `<ar/es/fr/ja/pt/ru/ur>`
- Facilitador: `<name>`

## Lab 1 - Exportacion de KPI
1. Abre el dashboard KPI del portal (`docs/portal/docs/sns/kpi-dashboard.md`).
2. Filtra por sufijo `<suffix>` y rango de tiempo `<window>`.
3. Exporta snapshots PDF + CSV.
4. Registra el SHA-256 del JSON/PDF exportado aqui: `______________________`.

## Lab 2 - Drill de manifest
1. Obten el manifest de muestra desde `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json`.
2. Valida con `cargo run --bin sns_manifest_check -- --input <file>`.
3. Genera el esqueleto de resolver con `scripts/sns_zonefile_skeleton.py`.
4. Pega el resumen del diff:
   ```
   <git diff output>
   ```

## Lab 3 - Simulacion de disputa
1. Usa el CLI de guardian para iniciar un freeze (case id `<case-id>`).
2. Registra el hash de la disputa: `______________________`.
3. Sube el log de evidencia a `artifacts/sns/training/<suffix>/<cycle>/logs/`.

## Lab 4 - Automatizacion de annex
1. Exporta el JSON del dashboard de Grafana y copialo en `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json`.
2. Ejecuta:
   ```bash
   cargo xtask sns-annex      --suffix <suffix>      --cycle <cycle>      --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --output docs/source/sns/reports/<suffix>/<cycle>.md      --regulatory-entry docs/source/sns/regulatory/<memo>.md      --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```
3. Pega la ruta del annex + salida SHA-256: `________________________________`.

## Notas de feedback
- Que fue poco claro?
- Que labs se pasaron de tiempo?
- Bugs de tooling observados?

Devuelve los cuadernos completos al facilitador; deben quedar en
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`.
