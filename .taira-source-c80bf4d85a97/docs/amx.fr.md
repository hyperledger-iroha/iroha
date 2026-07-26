---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to the English source for current semantics.
lang: fr
direction: ltr
source: docs/amx.md
status: complete
translator: manual
source_hash: 563ec4d7d4d96fa04a7b210a962b9927046985eb01d1de0954575cf817f9f226
source_last_modified: "2025-11-09T19:43:51.262828+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traduction française de docs/amx.md (AMX Execution & Operations Guide) -->

# Guide d’Exécution et d’Exploitation AMX

**Statut :** Brouillon (NX‑17)  
**Public :** Équipe protocole cœur, ingénieurs AMX/consensus, SRE/Télémétrie,
équipes SDK et Torii  
**Contexte :** Complète l’élément de roadmap « Documentation (owner: Docs) —
mettre à jour `docs/amx.md` avec des diagrammes de timing, un catalogue
d’erreurs, les attentes opérateur et les recommandations développeur pour la
génération/l’utilisation des PVO ».

## Résumé

Les transactions atomiques entre espaces de données (AMX) permettent à un seul
envoi de toucher plusieurs data spaces (DS) tout en préservant la finalité
des slots de 1 s, des codes d’erreur déterministes et la confidentialité des
fragments dans les DS privés. Ce guide décrit le modèle temporel, le
traitement canonique des erreurs, les exigences de preuves pour les
opérateurs et les attentes développeur concernant les Proof Verification
Objects (PVO), de sorte que le livrable de roadmap soit auto‑contenu en
dehors du document de conception Nexus (`docs/source/nexus.md`).

Garanties clés :

- Chaque envoi AMX dispose de budgets déterministes pour prepare/commit ; les
  dépassements entraînent un abort avec des codes documentés plutôt que de
  bloquer les lanes.
- Les échantillons DA qui dépassent leur budget font passer la transaction au
  slot suivant tout en émettant une télémétrie explicite
  (`missing-availability warning`) au lieu de dégrader silencieusement le
  throughput.
- Les PVO décorrèlent les preuves lourdes de la fenêtre de 1 s en permettant
  aux clients/batchers de pré‑enregistrer des artefacts que l’hôte Nexus peut
  valider rapidement durant le slot.

## Modèle temporel des slots

### Chronologie

```text
t=0ms           70ms             300ms              600ms       840ms    1000ms
│─────────┬───────────────┬───────────────────┬──────────────┬──────────┬────────│
│         │               │                   │              │          │        │
│  Mempool│Proof build + DA│Consensus PREP/COM │ IVM/AMX exec │Settlement│ Guard  │
│  ingest │sample (≤300ms) │(≤300ms)           │(≤250ms)      │(≤40ms)   │(≤40ms) │
```

- Les budgets sont alignés sur le plan global du ledger : mempool 70 ms,
  commit DA ≤300 ms, consensus 300 ms, IVM/AMX 250 ms, settlement 40 ms, guard
  40 ms.
- Les transactions qui dépassent la fenêtre DA sont replanifiées de façon
  déterministe ; les autres dépassements se traduisent par des codes tels que
  `AMX_TIMEOUT` ou `SETTLEMENT_ROUTER_UNAVAILABLE`.
- La fenêtre guard absorbe l’export de télémétrie et les derniers contrôles
  d’audit afin que le slot se termine toujours en 1 s même si les exporters
  accusent un léger retard.

### Swim lane inter‑DS

```text
Client        DS A (public)        DS B (private)        Nexus Lane        Settlement
  │ submit tx │                     │                     │                 │
  │──────────▶│ prepare fragment    │                     │                 │
  │           │ proof + DA part     │ prepare fragment    │                 │
  │           │───────────────┬────▶│ proof + DA part     │                 │
  │           │               │     │─────────────┬──────▶│ Merge proofs    │
  │           │               │     │             │       │ verify PVO/DA   │
  │           │               │     │             │       │────────┬────────▶ apply
  │◀──────────│ result + code │◀────│ result + code │◀────│ outcome│          receipt
```

Chaque fragment DS doit terminer sa fenêtre de prepare de 30 ms avant que la
lane n’assemble le slot. Les preuves manquantes restent dans le mempool pour
le slot suivant plutôt que de bloquer les pairs.

### Checklist d’instrumentation

| Métrique / Trace | Source | SLO / Alerte | Notes |
|------------------|--------|--------------|-------|
| `iroha_slot_duration_ms` (histogramme) / `iroha_slot_duration_ms_latest` (gauge) | `iroha_telemetry` | p95 ≤ 1000 ms | Garde‑fou CI décrit dans `ans3.md`. |
| `iroha_da_quorum_ratio` | `iroha_telemetry` (hook de commit) | ≥0.95 par fenêtre de 30 min | Dérivée de la télémétrie de replanification DA ; chaque bloc met à jour le gauge. |
| `iroha_amx_prepare_ms` | Hôte IVM | p95 ≤ 30 ms par DS scope | Pilote les aborts `AMX_TIMEOUT`. |
| `iroha_amx_commit_ms` | Hôte IVM | p95 ≤ 40 ms par DS scope | Couvre merge de deltas + exécution de triggers. |
| `iroha_ivm_exec_ms` | Hôte IVM | alerte si >250 ms par lane | Reflète la fenêtre d’exécution des overlays IVM. |
| `iroha_amx_abort_total{stage}` | Exécuteur | alerte si >0.05 aborts/slot ou pics prolongés sur un seul stage | `stage` ∈ {`prepare`, `exec`, `commit`}. |
| `iroha_amx_lock_conflicts_total` | Planificateur AMX | alerte si >0.1 conflits/slot | Indique des ensembles R/W imprécis. |

## Catalogue d’erreurs AMX et attentes opérateur

Erreurs courantes :

- `AMX_TIMEOUT` – un fragment a dépassé son budget (prepare/exec/commit).
- `AMX_LOCK_CONFLICT` – conflit de verrou détecté par le planificateur.
- `SETTLEMENT_ROUTER_UNAVAILABLE` – le routeur de settlement n’a pas pu
  router l’opération dans le slot.
- `PVO_MISSING` – un PVO preregistré était attendu mais introuvable.

Les opérateurs doivent :

- Surveiller les métriques ci‑dessus et maintenir des dashboards AMX dédiés
  dans Grafana.
- Maintenir des runbooks pour réagir aux pics `AMX_TIMEOUT` ou
  `AMX_LOCK_CONFLICT` (par exemple revoir les hints d’accès ou ajuster les
  budgets).
- Archiver les preuves (logs, traces, PVOs) pour les incidents AMX afin de
  pouvoir les rejouer/analyser offline.

## Recommandations développeur : PVO

- Les Proof Verification Objects encapsulent des preuves lourdes (par
  exemple des preuves ZK pour des fragments privés) dans un artefact que
  l’hôte Nexus peut vérifier rapidement dans le slot.
- Les clients doivent preregistrer les PVO hors du chemin critique du slot et
  les référencer dans les transactions AMX via des identifiants stables.
- La conception d’un PVO doit garantir :
  - Un encapsulage Norito canonique (champs versionnés, en‑tête Norito,
    checksum).
  - Une séparation claire entre données publiques et privées pour préserver
    la confidentialité.

Voir `docs/source/nexus.md` pour des détails de design supplémentaires et
les formats exacts de PVO.

