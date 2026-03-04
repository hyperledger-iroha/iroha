---
lang: pt
direction: ltr
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-11-15T09:17:29.843566+00:00"
translation_last_reviewed: 2026-01-01
---

# Modelo de caderno de treinamento SNS

Use este caderno como o handout canonico para cada turma de treinamento. Substitua os placeholders (`<...>`) antes de distribuir aos participantes.

## Detalhes da sessao
- Sufixo: `<.sora | .nexus | .dao>`
- Ciclo: `<YYYY-MM>`
- Idioma: `<ar/es/fr/ja/pt/ru/ur>`
- Facilitador: `<name>`

## Lab 1 - Exportacao de KPI
1. Abra o dashboard KPI do portal (`docs/portal/docs/sns/kpi-dashboard.md`).
2. Filtre por sufixo `<suffix>` e intervalo de tempo `<window>`.
3. Exporte snapshots PDF + CSV.
4. Registre o SHA-256 do JSON/PDF exportado aqui: `______________________`.

## Lab 2 - Drill de manifest
1. Baixe o manifest de exemplo em `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json`.
2. Valide com `cargo run --bin sns_manifest_check -- --input <file>`.
3. Gere o esqueleto do resolver com `scripts/sns_zonefile_skeleton.py`.
4. Cole o resumo do diff:
   ```
   <git diff output>
   ```

## Lab 3 - Simulacao de disputa
1. Use o CLI guardian para iniciar um freeze (case id `<case-id>`).
2. Registre o hash da disputa: `______________________`.
3. Envie o log de evidencia para `artifacts/sns/training/<suffix>/<cycle>/logs/`.

## Lab 4 - Automacao de annex
1. Exporte o JSON do dashboard Grafana e copie-o para `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json`.
2. Execute:
   ```bash
   cargo xtask sns-annex      --suffix <suffix>      --cycle <cycle>      --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --output docs/source/sns/reports/<suffix>/<cycle>.md      --regulatory-entry docs/source/sns/regulatory/<memo>.md      --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```
3. Cole o caminho do annex + saida SHA-256: `________________________________`.

## Notas de feedback
- O que ficou pouco claro?
- Quais labs passaram do tempo?
- Bugs de tooling observados?

Devolva os cadernos completos ao facilitador; eles devem ficar em
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`.
