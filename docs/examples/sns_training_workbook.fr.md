---
lang: fr
direction: ltr
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-11-15T09:17:29.843566+00:00"
translation_last_reviewed: 2026-01-01
---

# Modele de cahier d'entrainement SNS

Utilisez ce cahier comme support canonique pour chaque cohorte de formation. Remplacez les placeholders (`<...>`) avant de le distribuer aux participants.

## Details de session
- Suffixe: `<.sora | .nexus | .dao>`
- Cycle: `<YYYY-MM>`
- Langue: `<ar/es/fr/ja/pt/ru/ur>`
- Facilitateur: `<name>`

## Lab 1 - Export KPI
1. Ouvrez le dashboard KPI du portal (`docs/portal/docs/sns/kpi-dashboard.md`).
2. Filtrez par suffixe `<suffix>` et plage de temps `<window>`.
3. Exportez des snapshots PDF + CSV.
4. Notez le SHA-256 du JSON/PDF exporte ici: `______________________`.

## Lab 2 - Drill de manifest
1. Recuperez le manifest d'exemple depuis `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json`.
2. Validez avec `cargo run --bin sns_manifest_check -- --input <file>`.
3. Generez le squelette de resolver avec `scripts/sns_zonefile_skeleton.py`.
4. Collez le resume du diff:
   ```
   <git diff output>
   ```

## Lab 3 - Simulation de litige
1. Utilisez le CLI guardian pour demarrer un freeze (case id `<case-id>`).
2. Notez le hash du litige: `______________________`.
3. Televersez le log de preuves vers `artifacts/sns/training/<suffix>/<cycle>/logs/`.

## Lab 4 - Automatisation des annexes
1. Exportez le JSON du dashboard Grafana et copiez-le dans `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json`.
2. Executez:
   ```bash
   cargo xtask sns-annex      --suffix <suffix>      --cycle <cycle>      --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --output docs/source/sns/reports/<suffix>/<cycle>.md      --regulatory-entry docs/source/sns/regulatory/<memo>.md      --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```
3. Collez le chemin de l'annexe + sortie SHA-256: `________________________________`.

## Notes de feedback
- Qu'etait peu clair?
- Quels labs ont depasse le temps?
- Bugs de tooling observes?

Renvoyez les cahiers completes au facilitateur; ils doivent etre places sous
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`.
