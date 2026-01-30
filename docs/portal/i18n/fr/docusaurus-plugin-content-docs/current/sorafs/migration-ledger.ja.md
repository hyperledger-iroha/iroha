---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sorafs/migration-ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 508186c41784402e89441e8884e4f15cc6363be33de9acc3194e7d285ba9f361
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
id: migration-ledger
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


> Adapte de [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# Registre de migration SoraFS

Ce registre reprend le journal des migrations capture dans le RFC d'architecture
SoraFS. Les entrees sont groupees par jalon et indiquent la fenetre effective,
les equipes impactees et les actions requises. Les mises a jour du plan de
migration DOIVENT modifier cette page et le RFC
(`docs/source/sorafs_architecture_rfc.md`) pour garder les consommateurs en aval
alignes.

| Jalon | Fenetre effective | Resume du changement | Equipes impactees | Actions | Statut |
|-------|-------------------|---------------------|------------------|---------|--------|
| M1 | Semaines 7–12 | Le CI impose des fixtures deterministes; les preuves d'alias sont disponibles en staging; le tooling expose des flags d'attente explicites. | Docs, Storage, Governance | S'assurer que les fixtures restent signees, enregistrer les alias dans le registry de staging, mettre a jour les checklists de release avec l'exigence `--car-digest/--root-cid`. | ⏳ En attente |

Les minutes du plan de controle de gouvernance qui referencent ces jalons vivent sous
`docs/source/sorafs/`. Les equipes doivent ajouter des puces datees sous chaque ligne
lorsque des evenements notables surviennent (ex: nouveaux enregistrements d'alias,
retrospectives d'incidents du registry) afin de fournir une trace auditable.

## Mises a jour recentes

- 2025-11-01 — Diffusion de `migration_roadmap.md` au conseil de gouvernance et aux listes
  operateurs pour revision; en attente de validation lors de la prochaine session du
  conseil (ref: suivi `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 — L'ISI d'enregistrement du Pin Registry applique desormais la validation
  partagee chunker/politique via les helpers `sorafs_manifest`, gardant les chemins
  on-chain alignes avec les checks Torii.
- 2026-02-13 — Ajout des phases de rollout de provider advert (R0–R3) au registre et
  publication des dashboards et de la guidance operateur associes
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).
