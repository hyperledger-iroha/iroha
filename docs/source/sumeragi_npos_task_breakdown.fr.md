---
lang: fr
direction: ltr
source: docs/source/sumeragi_npos_task_breakdown.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1773b8fda6cda00e38b333096bfe5d6f6181c883ece5a62c11a190a09870d29
source_last_modified: "2025-12-12T12:49:53.638997+00:00"
translation_last_reviewed: 2026-01-01
---

## Decoupage des taches Sumeragi + NPoS

Cette note etend la feuille de route de la Phase A en petites taches d ingenierie afin de livrer le travail Sumeragi/NPoS restant de facon incrementale. Les annotations de statut suivent la convention: `DONE` termine, `IN PROGRESS` en cours, `NOT STARTED` non demarre, et `NEEDS TESTS` tests requis.

### A2 - Adoption des messages au niveau wire
- DONE: Exposer les types Norito `Proposal`/`Vote`/`Qc` dans `BlockMessage` et exercer les round-trips encode/decode (`crates/iroha_data_model/tests/consensus_roundtrip.rs`).
- DONE: Bloquer les frames precedents `BlockSigned/BlockCommitted`; le toggle de migration est reste a `false` avant le retrait.
- DONE: Retirer le knob de migration qui basculait les anciens messages de bloc; le mode Vote/QC est maintenant le seul chemin wire.
- DONE: Mettre a jour les routeurs Torii, les commandes CLI et les consommateurs de telemetrie pour preferer les snapshots JSON `/v1/sumeragi/*` aux anciens frames de bloc.
- DONE: La couverture d integration exerce les endpoints `/v1/sumeragi/*` uniquement via le pipeline Vote/QC (`integration_tests/tests/sumeragi_vote_qc_commit.rs`).
- DONE: Supprimer les anciens frames une fois la parite fonctionnelle et les tests d interop en place.

### Plan de suppression des frames
1. DONE: Des tests soak multi-noeuds ont tourne 72 h sur les harnesses telemetrie et CI; les snapshots Torii ont montre un debit stable du proposer et la formation de QC sans regressions.
2. DONE: La couverture des tests d integration tourne maintenant uniquement sur le chemin Vote/QC (`sumeragi_vote_qc_commit.rs`), garantissant que des pairs mixtes atteignent le consensus sans les anciens frames.
3. DONE: La documentation operateur et l aide CLI ne mentionnent plus l ancien chemin wire; la guidance de troubleshooting pointe maintenant vers la telemetrie Vote/QC.
4. DONE: Les variantes de messages, compteurs de telemetrie et caches de commit en attente ont ete supprimes; la matrice de surface reflete maintenant la surface Vote/QC uniquement.

### A3 - Application moteur et pacemaker
- DONE: Invariants Lock/HighestQC appliques dans `handle_message` (voir `block_created_header_sanity`).
- DONE: Le suivi de disponibilite des donnees valide le hash du payload RBC lors de l enregistrement de la livraison (`Actor::ensure_block_matches_rbc_payload`), afin que les sessions desaccordees ne soient pas traitees comme livrees.
- DONE: Integrer l exigence PrecommitQC (`require_precommit_qc`) dans les configs par defaut et ajouter des tests negatifs (defaut maintenant `true`; les tests couvrent les chemins avec gate et opt-out).
- DONE: Remplacer les heuristiques de redundant-send a l echelle de la view par des controleurs pacemaker appuyes par l EMA (`aggregator_retry_deadline` derive maintenant de l EMA live et pilote les delais de redundant send).
- DONE: Bloquer l assemblage des propositions sur la backpressure de file (`BackpressureGate` arrete maintenant le pacemaker quand la file est saturee et enregistre des deferrals pour status/telemetry).
- DONE: Les votes de availability sont emis apres validation de la proposition quand DA est requis (sans attendre le `DELIVER` local RBC), et la preuve d availability est suivie via `AvailabilityQC` comme preuve de securite pendant que le commit avance sans attendre. Cela evite les attentes circulaires entre transport de payload et vote.
- DONE: La couverture restart/liveness exerce maintenant la recuperation RBC au cold-start (`integration_tests/tests/sumeragi_da.rs::sumeragi_rbc_session_recovers_after_cold_restart`) et la reprise du pacemaker apres downtime (`integration_tests/tests/sumeragi_npos_liveness.rs::npos_pacemaker_resumes_after_downtime`).
- DONE: Ajouter des tests de regression deterministes de restart/view-change couvrant la convergence de lock (`integration_tests/tests/sumeragi_lock_convergence.rs`).

### A4 - Pipeline des collectors et aleatoire
- DONE: Les helpers de rotation deterministe des collectors vivent dans `collectors.rs`.
- DONE: GA-A4.1 - La selection de collectors supportee par PRF enregistre maintenant des seeds deterministes et height/view dans `/status` et la telemetrie; les hooks de refresh VRF propagent le contexte apres commits et reveals. Owners: `@sumeragi-core`. Tracker: `project_tracker/npos_sumeragi_phase_a.md` (ferme).
- DONE: GA-A4.2 - Exposer la telemetrie de participation aux reveals + commandes CLI d inspection et mettre a jour les manifests Norito. Owners: `@telemetry-ops`, `@torii-sdk`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:6`.
- DONE: GA-A4.3 - Codifier la recuperation late-reveal et les tests d epoch a participation nulle dans `integration_tests/tests/sumeragi_randomness.rs` (`npos_late_vrf_reveal_clears_penalty_and_preserves_seed`, `npos_zero_participation_epoch_reports_full_no_participation`), en exercant la telemetrie de nettoyage des penalites. Owners: `@sumeragi-core`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:7`.

### A5 - Reconfiguration conjointe et evidence
- DONE: Le scaffolding d evidence, la persistance WSV et les roundtrips Norito couvrent maintenant double-vote, invalid proposal, invalid QC et variantes de double exec avec deduplication deterministe et pruning d horizon (`sumeragi::evidence`).
- DONE: GA-A5.1 - Activation joint-consensus (ancien set commit, nouveau set actif au bloc suivant) enforcee avec une couverture d integration ciblee.
- DONE: GA-A5.2 - Docs de governance et flux CLI pour slashing/jailing mis a jour, avec des tests de synchronisation mdBook pour figer les defaults et le wording de l evidence horizon.
- DONE: GA-A5.3 - Tests d evidence en chemin negatif (duplicate signer, forged signature, stale epoch replay, mixed manifest payloads) plus fixtures fuzz integres et executes chaque nuit pour proteger la validation roundtrip Norito.

### A6 - Outillage, docs, validation
- DONE: Telemetrie/reporting RBC en place; le rapport DA genere des metriques reelles (y compris les compteurs d eviction).
- DONE: GA-A6.1 - Le test happy-path NPoS a 4 peers avec VRF tourne maintenant en CI avec seuils pacemaker/RBC enforcees via `integration_tests/tests/sumeragi_npos_happy_path.rs`. Owners: `@qa-consensus`, `@telemetry-ops`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:11`.
- DONE: GA-A6.2 - Capturer la baseline de performance NPoS (blocs de 1 s, k=3) et publier dans `status.md`/docs operateur avec seeds de harness reproductibles + matrice materielle. Owners: `@performance-lab`, `@telemetry-ops`. Report: `docs/source/generated/sumeragi_baseline_report.md`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:12`. La run live a ete enregistree sur Apple M2 Ultra (24 cores, 192 GB RAM, macOS 15.0) avec la commande documentee dans `scripts/run_sumeragi_baseline.py`.
- DONE: GA-A6.3 - Les guides de troubleshooting operateur pour l instrumentation RBC/pacemaker/backpressure ont atterri (`docs/source/telemetry.md:523`); la correlation de logs est maintenant geree par `scripts/sumeragi_backpressure_log_scraper.py`, pour que les operateurs puissent extraire les paires pacemaker deferral/missing-availability sans grep manuel. Owners: `@operator-docs`, `@telemetry-ops`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:13`.
- DONE: Ajout de scenarios de performance RBC store/chunk-loss (`npos_rbc_store_backpressure_records_metrics`, `npos_rbc_chunk_loss_fault_reports_backlog`), couverture de fan-out redondant (`npos_redundant_send_retries_update_metrics`) et harness de jitter borne (`npos_pacemaker_jitter_within_band`) afin que la suite A6 exerce les deferrals soft-limit du store, les drops deterministes de chunks, la telemetrie redundant-send et les bandes de jitter pacemaker sous stress. [integration_tests/tests/sumeragi_npos_performance.rs:633] [integration_tests/tests/sumeragi_npos_performance.rs:760] [integration_tests/tests/sumeragi_npos_performance.rs:800] [integration_tests/tests/sumeragi_npos_performance.rs:639]

### Etapes immediates
1. DONE: Le harness de jitter borne exerce les metriques de jitter pacemaker (`integration_tests/tests/sumeragi_npos_performance.rs::npos_pacemaker_jitter_within_band`).
2. DONE: Renforcer les assertions de deferral RBC dans `npos_queue_backpressure_triggers_metrics` en amorcant une pression deterministe du store RBC (`integration_tests/tests/sumeragi_npos_performance.rs::npos_queue_backpressure_triggers_metrics`).
3. DONE: Etendre le soak de `/v1/sumeragi/telemetry` pour couvrir des epochs longues et des collectors adverses, en comparant les snapshots aux compteurs Prometheus sur plusieurs heights. Couvert par `integration_tests/tests/sumeragi_telemetry.rs::npos_telemetry_soak_matches_metrics_under_adversarial_collectors`.

Suivre cette liste ici permet de garder `roadmap.md` concentre sur les jalons tout en donnant a l equipe une checklist vivante. Mettre a jour les entrees (et marquer l achevement) au fil des patchs.
