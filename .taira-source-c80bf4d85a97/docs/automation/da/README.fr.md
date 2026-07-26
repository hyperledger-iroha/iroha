---
lang: fr
direction: ltr
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9e5fce128259ae2b2c40782b3c96c38048fce6f3b4522319bd60b59db87a8252
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Automatisation du modèle de menaces Data-Availability (DA-1)

L'item DA-1 du roadmap et `status.md` exigent une boucle d'automatisation
 déterministe qui produit les résumés du modèle de menaces Norito PDP/PoTR
 publiés dans `docs/source/da/threat_model.md` et sa copie Docusaurus. Ce
 répertoire capture les artefacts référencés par :

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model` (qui exécute `scripts/docs/render_da_threat_model_tables.py`)
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## Flux

1. **Générez le rapport**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   Le résumé JSON enregistre le taux simulé d'échec de réplication, les seuils
   du chunker et toute violation de politique détectée par le harness PDP/PoTR
   dans `integration_tests/src/da/pdp_potr.rs`.
2. **Rendez les tables Markdown**
   ```bash
   make docs-da-threat-model
   ```
   Ceci exécute `scripts/docs/render_da_threat_model_tables.py` pour réécrire
   `docs/source/da/threat_model.md` et `docs/portal/docs/da/threat-model.md`.
3. **Archivez l'artefact** en copiant le rapport JSON (et le log CLI optionnel)
   dans `docs/automation/da/reports/<timestamp>-threat_model_report.json`. Quand
   une décision de gouvernance dépend d'une exécution précise, ajoutez le hash du
   commit et la graine du simulateur dans un `<timestamp>-metadata.md` associé.

## Attentes en matière de preuves

- Les fichiers JSON doivent rester <100 KiB pour vivre dans git. Les traces plus
  volumineuses doivent rester dans un stockage externe ; référencez leur hash
  signé dans la note de métadonnées si nécessaire.
- Chaque fichier archivé doit lister la graine, le chemin de configuration et la
  version du simulateur pour que les relances soient reproductibles à l'identique.
- Liez le fichier archivé depuis `status.md` ou l'entrée du roadmap chaque fois
  que les critères d'acceptation DA-1 avancent, afin que les relecteurs puissent
  vérifier la baseline sans relancer le harness.

## Réconciliation des engagements (omission du séquenceur)

Utilisez `cargo xtask da-commitment-reconcile` pour comparer les reçus
 d'ingestion DA aux enregistrements d'engagements DA et détecter les omissions
 ou manipulations du séquenceur :

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- Accepte des reçus en Norito ou JSON et des engagements depuis `SignedBlockWire`,
  `.norito` ou des bundles JSON.
- Échoue si un ticket manque dans le journal de blocs ou si les hashes divergent ;
  `--allow-unexpected` ignore les tickets présents uniquement dans les blocs
  lorsque vous limitez volontairement l'ensemble des reçus.
- Joignez le JSON émis aux paquets de gouvernance/Alertmanager pour les alertes
  d'omission ; par défaut il s'agit de `artifacts/da/commitment_reconciliation.json`.

## Audit des privilèges (revue d'accès trimestrielle)

Utilisez `cargo xtask da-privilege-audit` pour analyser les répertoires de
manifests/replay DA (et des chemins additionnels optionnels) afin de repérer les
entrées manquantes, non répertoires ou world-writable :

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- Lit les chemins d'ingestion DA depuis la config Torii et inspecte les
  permissions Unix lorsque disponibles.
- Signale les chemins manquants/non répertoires/world-writable et renvoie un
  code de sortie non nul en cas de problème.
- Signez et joignez le bundle JSON (`artifacts/da/privilege_audit.json` par
  défaut) aux paquets et dashboards de revue trimestrielle.
