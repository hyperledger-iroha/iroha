---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Registre de migration SoraFS
description : Journal des changements canoniques qui conviennent à chaque jalon de migration, les responsables et les suivis requis.
---

> Adapter de [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# Registre de migration SoraFS

Ce registre reprend le journal des migrations capturé dans le RFC d'architecture
SoraFS. Les entrées sont groupées par jalon et indiquent la fenêtre effective,
les équipes impactées et les actions requises. Les mises à jour du plan de
migration DOIVENT modifier cette page et le RFC
(`docs/source/sorafs_architecture_rfc.md`) pour garder les consommateurs en aval
s'aligne.

| Jalón | Fenêtre efficace | CV du changement | Equipes impactées | Actions | Statuts |
|-------|---------|-----------|------------------|---------|--------|
| M1 | Semaines 7 à 12 | Le CI impose des luminaires déterministes ; les preuves d'alias sont disponibles en staging; le outillage expose des drapeaux d'attente explicites. | Documents, stockage, gouvernance | S'assurer que les luminaires restent signés, enregistrer les alias dans le registre de staging, mettre à jour les checklists de release avec l'exigence `--car-digest/--root-cid`. | ⏳ En attente |Les minutes du plan de contrôle de gouvernance qui référencent ces jalons vivent sous
`docs/source/sorafs/`. Les équipes doivent ajouter des puces datées sous chaque ligne
lorsque des événements notables surviennent (ex: nouveaux enregistrements d'alias,
rétrospectives d'incidents du registre) afin de fournir une trace auditable.

## Mises à jour récentes

- 2025-11-01 — Diffusion de `migration_roadmap.md` au conseil de gouvernance et aux listes
  opérateurs pour révision; en attente de validation lors de la prochaine session du
  conseil (réf : suivi `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 — L'ISI d'enregistrement du Pin Registry applique désormais la validation
  partagee chunker/politique via les helpers `sorafs_manifest`, gardant les chemins
  on-chain s'aligne avec les checks Torii.
- 2026-02-13 — Ajout des phases de déploiement de l'annonce du fournisseur (R0–R3) au registre et
  publication des tableaux de bord et de la guidance opérateur associés
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).