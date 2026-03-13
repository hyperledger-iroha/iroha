---
lang: fr
direction: ltr
source: docs/source/nexus_cross_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6e6f144bf3aef313ba55b539c9e92c827bd626973fe38b557f0b668cc909f589
source_last_modified: "2025-12-13T05:07:11.929584+00:00"
translation_last_reviewed: 2026-01-01
---

# Engagements cross-lane de Nexus et pipeline de preuves

> **Statut:** livrable NX-4 - pipeline d'engagements cross-lane et preuves (cible Q4 2025).  
> **Responsables:** Nexus Core WG / Cryptography WG / Networking TL.  
> **Elements de roadmap relies:** NX-1 (geometrie des lanes), NX-3 (settlement router), NX-4 (ce document), NX-8 (scheduler global), NX-11 (conformite SDK).

Cette note decrit comment les donnees d'execution par lane deviennent un engagement global verifiable. Elle relie le settlement router existant (`crates/settlement_router`), le lane block builder (`crates/iroha_core/src/block.rs`), les surfaces telemetrie/status, et les hooks LaneRelay/DA prevus qui restent a livrer pour le roadmap **NX-4**.

## Objectifs

- Produire un `LaneBlockCommitment` deterministe par lane block capturant settlement, liquidite et donnees de variance sans divulguer l'etat prive.
- Relayer ces engagements (et leurs attestations DA) vers l'anneau NPoS global afin que le merge ledger ordonne, valide et persiste les mises a jour cross-lane.
- Exposer les memes payloads via Torii et telemetrie pour que les operateurs, SDKs et auditeurs puissent rejouer le pipeline sans outillage sur mesure.
- Definir les invariants et les bundles de preuve requis pour valider NX-4: preuves de lane, attestations DA, integration du merge ledger et couverture de regression.

## Composants et surfaces

| Composant | Responsabilite | References d'implementation |
|-----------|----------------|-----------------------------|
| Executeur de lane et settlement router | Coter les conversions XOR, accumuler les receipts par transaction, appliquer la politique de buffer | `crates/iroha_core/src/settlement/mod.rs`, `crates/settlement_router` |
| Lane block builder | Vider les `SettlementAccumulator`s, emettre des `LaneBlockCommitment`s avec le lane block | `crates/iroha_core/src/block.rs:3340-3415` |
| LaneRelay broadcaster | Emballer les QCs de lane + preuves DA, les diffuser via `iroha_p2p`, et alimenter le merge ring | `crates/iroha_core/src/nexus/lane_relay.rs`, `crates/iroha_core/src/sumeragi/main_loop.rs` |
| Global merge ledger | Verifier les QCs de lane, reduire les merge hints, persister les engagements du world-state | `docs/source/merge_ledger.md`, `crates/iroha_core/src/sumeragi/status.rs`, `crates/iroha_core/src/state.rs` |
| Torii status et dashboards | Exposer `lane_commitments`, `lane_settlement_commitments`, `lane_relay_envelopes`, gauges du scheduler et tableaux Grafana | `crates/iroha_torii/src/routing.rs:16660-16880`, `dashboards/grafana/nexus_lanes.json` |
| Stockage de preuves | Archiver `LaneBlockCommitment`s, artefacts RBC et snapshots Alertmanager pour audits | `docs/settlement-router.md`, `artifacts/nexus/*` (bundle futur) |

## Structures de donnees et layout de payload

Les payloads canoniques vivent dans `crates/iroha_data_model/src/block/consensus.rs`.

### `LaneSettlementReceipt`

- `source_id` - hash de transaction ou id fourni par l'appelant.
- `local_amount_micro` - debit du token de gas du dataspace.
- `xor_due_micro` / `xor_after_haircut_micro` / `xor_variance_micro` - entrees deterministes du livre XOR et la marge de securite par receipt (`due - after haircut`).
- `timestamp_ms` - timestamp UTC en millisecondes capture pendant le settlement.

Les receipts heritent des regles de cotation deterministes de `SettlementEngine` et sont agreges dans chaque `LaneBlockCommitment`.

### `LaneSwapMetadata`

Metadonnees optionnelles qui enregistrent les parametres utilises lors des cotations:

- `epsilon_bps`, `twap_window_seconds`, `volatility_class`.
- bucket `liquidity_profile` (Tier1-Tier3).
- string `twap_local_per_xor` pour que les auditeurs recomputent les conversions exactement.

### `LaneBlockCommitment`

Resume par lane stocke avec chaque bloc:

- En-tete: `block_height`, `lane_id`, `dataspace_id`, `tx_count`.
- Totaux: `total_local_micro`, `total_xor_due_micro`, `total_xor_after_haircut_micro`, `total_xor_variance_micro`.
- `swap_metadata` optionnel.
- Vecteur `receipts` ordonne.

Ces structs derivent deja `NoritoSerialize`/`NoritoDeserialize`, elles peuvent donc etre diffusees on-chain, via Torii, ou via fixtures sans derive de schema.

### `LaneRelayEnvelope`

`LaneRelayEnvelope` (voir `crates/iroha_data_model/src/nexus/relay.rs`) emballe le `BlockHeader`
de la lane, un `commit QC (`Qc`)` optionnel, un hash optionnel de `DaCommitmentBundle`, le
`LaneBlockCommitment` complet et le compte de bytes RBC par lane. L'enveloppe stocke un
`settlement_hash` derive par Norito (via `compute_settlement_hash`) pour que les recepteurs
valident le payload de settlement avant de le relayer au merge ledger. Les appelants doivent
rejeter les enveloppes quand `verify` echoue (mismatch de sujet QC, mismatch de hash DA ou mismatch
settlement hash), quand `verify_with_quorum` echoue (erreurs de longueur de bitmap de
signataires/quorum), ou quand la signature QC agregat ne peut pas etre verifiee contre le roster
du comite par dataspace. La preimage du QC couvre le hash du lane block plus `parent_state_root` et
`post_state_root`, afin que l'appartenance et la correction du state-root soient verifiees ensemble.

### Selection du comite de lane

Les QCs de lane relay sont valides contre un comite par dataspace. La taille du comite est `3f+1`,
ou `f` est configure dans le catalogue du dataspace (`fault_tolerance`). Le pool de validateurs
est celui du dataspace: manifests de gouvernance de lane pour les lanes admin-managed et enregistrements
staking de lane publique pour les lanes stake-elected. L'appartenance au comite est echantillonnee
par epoque de facon deterministe en utilisant la graine d'epoque VRF liee a `dataspace_id` et
`lane_id` (stable pour l'epoque). Si le pool est plus petit que `3f+1`, la finalite du lane relay
se met en pause jusqu'a restauration du quorum. Les operateurs peuvent etendre le pool en utilisant
l'instruction multisig admin `SetLaneRelayEmergencyValidators` (necessite `CanManagePeers` et
`nexus.lane_relay_emergency.enabled = true`, desactive par defaut). Quand il est actif, l'autorite
doit etre un compte multisig respectant les minimums configures
(`nexus.lane_relay_emergency.multisig_threshold`/`multisig_members`, par defaut 3-sur-5). Les
surcharges sont stockees par dataspace, appliquees seulement quand le pool est sous quorum, et
nettoyees en soumettant une liste vide de validateurs. Quand `expires_at_height` est defini, la
validation ignore la surcharge une fois que le `block_height` de l'enveloppe lane relay depasse la
hauteur d'expiration. Le compteur de telemetrie
`lane_relay_emergency_override_total{lane,dataspace,outcome}` enregistre si la surcharge a ete
appliquee (`applied`) ou manquante/expiree/insuffisante/desactivee pendant la validation.

## Cycle de vie de l'engagement

1. **Coter et preparer les receipts.**  
   La facade de settlement (`SettlementEngine`, `SettlementAccumulator`) enregistre un
   `PendingSettlement` par transaction. Chaque enregistrement stocke les entrees TWAP, profil de
   liquidite, timestamps et montants XOR afin de devenir ensuite un `LaneSettlementReceipt`.

2. **Sceller les receipts dans le bloc.**  
   Pendant `BlockBuilder::finalize`, chaque paire `(lane_id, dataspace_id)` vide son accumulateur.
   Le builder instancie un `LaneBlockCommitment`, copie la liste des receipts, additionne les
   totaux, et stocke des metadonnees de swap optionnelles (via `SwapEvidence`). Le vecteur resultat
   est pousse dans le slot de statut Sumeragi (`crates/iroha_core/src/sumeragi/status.rs`) afin que
   Torii et la telemetrie l'exposent immediatement.

3. **Packaging relay et attestations DA.**  
   `LaneRelayBroadcaster` consomme maintenant les `LaneRelayEnvelope`s emis pendant le scellage du
   bloc et les diffuse comme des frames `NetworkMessage::LaneRelay` a haute priorite. Les enveloppes
   sont verifiees, dedupliquees par `(lane_id,dataspace_id,height,settlement_hash)`, et persistees
   dans le snapshot de statut Sumeragi (`/v2/sumeragi/status`) pour les operateurs et auditeurs. Le
   broadcaster continuera d'evoluer pour attacher les artefacts DA (preuves de chunk RBC, headers
   Norito, manifests SoraFS/Object) et alimenter le merge ring sans blocage head-of-line.

4. **Ordonnancement global et merge ledger.**  
   L'anneau NPoS valide chaque relay envelope: verifier `lane_qc` contre le comite par dataspace,
   recalculer les totaux de settlement, verifier les preuves DA, puis alimenter le tip de la lane
   dans le merge ledger decrit dans `docs/source/merge_ledger.md`. Quand l'entree de merge est
   scellee, le hash du world-state (`global_state_root`) engage maintenant chaque `LaneBlockCommitment`.

5. **Persistance et exposition.**  
   Kura ecrit le lane block, l'entree de merge et le `LaneBlockCommitment` de facon atomique pour
   que le replay reconstruise la meme reduction. `/v2/sumeragi/status` expose:
   - `lane_commitments` (metadonnees d'execution).
   - `lane_settlement_commitments` (le payload decrit ici).
   - `lane_relay_envelopes` (headers de relay, QCs, digests DA, settlement hash et comptes de bytes RBC).
  Dashboards (`dashboards/grafana/nexus_lanes.json`) lisent les memes surfaces telemetrie/status
  pour afficher le throughput de lane, alertes de disponibilite DA, volume RBC, deltas de settlement
  et evidence de relay.

## Regles de verification et preuves

Le merge ring DOIT appliquer ce qui suit avant d'accepter un engagement de lane:

1. **Validite du QC de lane.** Verifier la signature BLS agregat sur la preimage du vote
   d'execution (hash de bloc, `parent_state_root`, `post_state_root`, hauteur/vue/epoque,
   `chain_id` et tag de mode) contre le roster du comite par dataspace; garantir que la longueur
   du bitmap de signataires correspond au comite, que les signataires mappent vers des indices
   valides, et que la hauteur du header correspond a `LaneBlockCommitment.block_height`.
2. **Integrite des receipts.** Recalculer les agregats `total_*` depuis le vecteur de receipts;
   rejeter l'engagement si les sommes divergent ou si les receipts contiennent des `source_id`s
   dupliques.
3. **Sanite des metadonnees de swap.** Confirmer que `swap_metadata` (si present) correspond a la
   configuration de settlement et a la politique de buffer de la lane.
4. **Attestation DA.** Valider que les preuves RBC/SoraFS fournies par le relay hashent vers le
   digest integre et que l'ensemble des chunks couvre tout le payload du bloc (`rbc_bytes_total`
   en telemetrie doit le refleter).
5. **Reduction de merge.** Une fois que les preuves par lane passent, inclure le tip de la lane
   dans l'entree du merge ledger et recalculer la reduction Poseidon2 (`reduce_merge_hint_roots`).
   Toute divergence annule l'entree de merge.
6. **Telemetrie et piste d'audit.** Incrementer les compteurs d'audit par lane
   (`nexus_audit_outcome_total{lane_id,...}`) et persister l'enveloppe pour que le bundle de preuve
   contienne a la fois la preuve et la trace d'observabilite.

## Disponibilite des donnees et observabilite

- **Metriques:**  
  `nexus_scheduler_lane_teu_*`, `nexus_scheduler_dataspace_*`, `sumeragi_rbc_da_reschedule_total`,
  `da_reschedule_total`, `sumeragi_da_gate_block_total{reason="missing_local_data"}`,
  `lane_relay_invalid_total{error}`, `lane_relay_emergency_override_total{outcome}`, et
  `nexus_audit_outcome_total` existent deja dans `crates/iroha_telemetry/src/metrics.rs`. Les
  operateurs doivent alerter sur les pics de missing-availability (les compteurs de reschedule sont
  adversariaux.
- **Surfaces Torii:**  
  `/v2/sumeragi/status` inclut `lane_commitments`, `lane_settlement_commitments` et des snapshots
  de dataspace. `/v2/nexus/lane-config` (prevu) publiera la geometrie de `LaneConfig` pour que les
  clients puissent mapper `lane_id` <-> labels de dataspace.
- **Dashboards:**  
  `dashboards/grafana/nexus_lanes.json` trace le backlog de lane, signaux de disponibilite DA et
  les totaux de settlement exposes ci-dessus. Les definitions d'alertes doivent paginer quand:
  - `nexus_scheduler_dataspace_age_slots` depasse la politique.
  - `sumeragi_da_gate_block_total{reason="missing_local_data"}` augmente de facon persistante.
  - `total_xor_variance_micro` devie des normes historiques.
- **Bundles de preuves:**  
  Chaque release doit attacher des exports `LaneBlockCommitment`, snapshots Grafana/Alertmanager
  et manifests de DA relay sous `artifacts/nexus/cross-lane/<date>/`. Le bundle devient l'ensemble
  de preuve canonique lors de la soumission des rapports de readiness NX-4.

## Checklist d'implementation (NX-4)

1. **Service LaneRelay**
   - Schema defini dans `LaneRelayEnvelope`; broadcaster implemente dans
     `crates/iroha_core/src/nexus/lane_relay.rs` et cable au scellage de blocs
     (`crates/iroha_core/src/sumeragi/main_loop.rs`), emettant `NetworkMessage::LaneRelay` avec
     deduplication par noeud et persistance de statut.
   - Persister les artefacts relay pour audits (`artifacts/nexus/relay/...`).
2. **Hooks d'attestation DA**
   - Integrer les preuves de chunk RBC / SoraFS avec les relay envelopes et stocker des metriques
     resume dans `SumeragiStatus`.
   - Exposer le statut DA via Torii et Grafana pour les operateurs.
3. **Validation du merge ledger**
   - Etendre le validateur d'entrees de merge pour exiger des relay envelopes, pas des headers de
     lane bruts.
   - Ajouter des tests de replay (`integration_tests/tests/nexus/*.rs`) qui alimentent des
     engagements synthetiques dans le merge ledger et affirment une reduction deterministe.
4. **Mises a jour SDK et tooling**
   - Documenter le layout Norito de `LaneBlockCommitment` pour les consommateurs SDK
     (`docs/portal/docs/nexus/lane-model.md` lie deja ici; l'etendre avec des snippets d'API).
   - Les fixtures deterministes vivent sous `fixtures/nexus/lane_commitments/*.{json,to}`; lancer
     `cargo xtask nexus-fixtures` pour regenerer (ou `--verify` pour valider) les echantillons
     `default_public_lane_commitment` et `cbdc_private_lane_commitment` quand des changements de
     schema arrivent.
5. **Observabilite et runbooks**
   - Cablage du pack Alertmanager pour les nouvelles metriques et documentation du workflow de
     preuve dans `docs/source/runbooks/nexus_cross_lane_incident.md` (suivi).

Completer la checklist ci-dessus, avec cette specification, satisfait la partie documentation de
**NX-4** et debloque le travail d'implementation restant.
