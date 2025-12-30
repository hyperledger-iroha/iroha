<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0c5ad15044887087c487d93762739fa9241f384a634d178aa76d1dcf8cb1cdb0
source_last_modified: "2025-11-09T14:34:44.965608+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: Registre de migration SoraFS
description: Journal des changements canonique qui suit chaque jalon de migration, les responsables et les suivis requis.
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
| M0 | Semaines 1–6 | Fixtures de chunker publiees; les pipelines emettent des bundles CAR + manifest aux cotes des artefacts legacy; entrees du registre creees. | Docs, DevRel, SDKs | Adopter `sorafs_manifest_stub` avec des flags d'attente, consigner des entrees dans ce registre, maintenir le CDN legacy. | ✅ Actif |
| M1 | Semaines 7–12 | Le CI impose des fixtures deterministes; les preuves d'alias sont disponibles en staging; le tooling expose des flags d'attente explicites. | Docs, Storage, Governance | S'assurer que les fixtures restent signees, enregistrer les alias dans le registry de staging, mettre a jour les checklists de release avec l'exigence `--car-digest/--root-cid`. | ⏳ En attente |
| M2 | Semaines 13–20 | Le pinning base sur le registry devient le chemin principal; les artefacts legacy passent en lecture seule; les gateways priorisent les preuves du registry. | Storage, Ops, Governance | Router le pinning via le registry, geler les hosts legacy, publier des avis de migration pour les operateurs. | ⏳ En attente |
| M3 | Semaine 21+ | Acces uniquement par alias impose; observabilite alerte sur la parite du registry; CDN legacy retire. | Ops, Networking, SDKs | Supprimer le DNS legacy, faire tourner les URL mises en cache, surveiller les dashboards de parite, mettre a jour les defaults SDK. | ⏳ En attente |
| R0–R3 | 2025-03-31 → 2025-07-01 | Phases d'application des provider adverts: R0 observer, R1 avertir, R2 appliquer handles/capacites canoniques, R3 purger les payloads legacy. | Observability, Ops, SDKs, DevRel | Importer `grafana_sorafs_admission.json`, suivre la checklist operateur dans `provider_advert_rollout.md`, preparer les renouvellements d'adverts 30+ jours avant la barriere R2. | ⏳ En attente |

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
