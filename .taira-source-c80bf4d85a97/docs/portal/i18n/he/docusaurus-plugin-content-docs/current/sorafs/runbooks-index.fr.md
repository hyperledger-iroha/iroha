---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/runbooks-index.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: runbooks-index
כותרת: Index des runbooks opérateurs
sidebar_label: Index des runbooks
תיאור: Point d'entrée canonique pour les runbooks opérateurs SoraFS migrés.
---

> Reflète le registre des responsables qui se trouve dans `docs/source/sorafs/runbooks/`.
> Chaque nouveau guide d'exploitation SoraFS doit être lié ici dès qu'il est publié dans
> la build du portail.

Utilisez cette page pour vérifier quels runbooks ont terminé la migration de l'arborescence
de docs héritée vers le portail. Chaque entrée indique la responsabilité, le chemin source
canonique et la copie portail afin que les relecteurs puissent acceder directement au guide
תליון souhaité l'aperçu bêta.

## Hôte d'aperçu bêta

La DocOps מעורפל a désormais promu l'hôte d'aperçu bêta approuvé par les relecteurs à
`https://docs.iroha.tech/`. Lorsque vous dirigez des opérateurs ou des relecteurs vers un
runbook migré, référencez ce nom d'hôte afin qu'ils consultent l'instantané du portail
חסות פר בדיקה. Les procédures de publication/rollback se trouvent dans
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Runbook | אחראי(ים) | העתק פורטל | מקור |
|--------|----------------|----------------|--------|
| שער Lancement & DNS | Networking TL, Ops Automation, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Playbook d'exploitation SoraFS | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Réconciliation de capacité | האוצר / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Ops du registre de pins | Tooling WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Checklist d'exploitation des nœuds | צוות אחסון, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Runbook litiges et révocations | מועצת ממשל | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Playbook de manifeste de staging | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Observabilité de l’ancre Taikai | פלטפורמת מדיה WG / DA Program / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## רשימת אימות

- [x] La build du portail renvoie vers cet index (entrée de la barre latérale).
- [x] Chaque runbook migré list le chemin source canonique pour garder les relecteurs
  alignés lors des revues de documentation.
- [x] Le pipeline d'aperçu DocOps bloque les merges lorsqu'un runbook listé manque dans la
  sortie du portail.

Les migrations futures (עמ' ex. nouveaux exercices de chaos או annexes de governance)
doivent ajouter une ligne au tableau ci-dessus et mettre à jour la checklist DocOps intégrée dans
`docs/examples/docs_preview_request_template.md`.