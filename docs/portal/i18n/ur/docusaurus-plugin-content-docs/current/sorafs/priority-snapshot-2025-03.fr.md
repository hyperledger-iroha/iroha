---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: priority-snapshot-2025-03
title: Instantané des priorités — mars 2025 (Bêta)
description: Miroir du snapshot de steering Nexus 2025-03 ; en attente d'ACKs avant le rollout public.
---

> Source canonique : `docs/source/sorafs/priority_snapshot_2025-03.md`
>
> Statut : **Bêta / en attente des ACKs du steering** (Networking, Storage, Docs leads).

## Aperçu

L'instantané de mars maintient les initiatives docs/content-network alignées
avec les axes de livraison SoraFS (SF-3, SF-6b, SF-9). Une fois que tous les
leads auront accusé réception dans le canal de steering Nexus, retirez la note
« Bêta » ci-dessus.

### Fils de focus

1. **Diffuser l'instantané des priorités** — collecter les acknowledgements et
   les consigner dans les minutes du council du 2025-03-05.
2. **Clôturer le kickoff Gateway/DNS** — répéter le nouveau kit de facilitation
   (Section 6 du runbook) avant le workshop 2025-03-03.
3. **Migration des runbooks opérateur** — le portail `Runbook Index` est live ;
   exposer l'URL de beta preview après le sign-off d'onboarding des reviewers.
4. **Fils de delivery SoraFS** — aligner le travail restant SF-3/6b/9 avec le
   plan/roadmap :
   - Worker d'ingestion PoR + endpoint de statut dans `sorafs-node`.
   - Polissage des bindings CLI/SDK sur les intégrations orchestrator Rust/JS/Swift.
   - Câblage runtime du coordinateur PoR et événements GovernanceLog.

Voir le fichier source pour la table complète, la checklist de distribution et
les entrées de log.
