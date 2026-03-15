---
lang: fr
direction: ltr
source: docs/source/sumeragi_randomness_evidence_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9a7b2f030cb798b78947c0d7cb298ccbd8a94a006be2e804f1ed043dc15dabc
source_last_modified: "2025-11-15T08:00:21.780712+00:00"
translation_last_reviewed: 2026-01-01
---

# Runbook alea et evidence Sumeragi

Ce guide satisfait l item Milestone A6 du roadmap qui demandait des procedures
operateur actualisees pour l alea VRF et l evidence de slashing. Utilisez-le avec
{doc}`sumeragi` et {doc}`sumeragi_chaos_performance_runbook` lorsque vous preparez
un nouveau build de validateur ou capturez des artefacts de readiness pour governance.


Note: For the v1 release, VRF penalties jail offenders after the activation lag, and consensus slashing is delayed by `sumeragi.npos.reconfig.slashing_delay_blocks` (default 259200 blocks, ~3 days at 1s) so governance can cancel with `CancelConsensusEvidencePenalty` before it applies.

## Portee et prerequis

- `iroha_cli` configure pour le cluster cible (voir `docs/source/cli.md`).
- `curl`/`jq` pour scrapper le payload Torii `/status` lors de la preparation des entrees.
- Acces Prometheus (ou exports snapshot) pour les metriques `sumeragi_vrf_*`.
- Connaitre l epoch et le roster actuel afin de faire correspondre la sortie CLI
  avec le snapshot de staking ou le manifest de governance.

## 1. Confirmer la selection de mode et le contexte d epoch

1. Lancez `iroha --output-format text ops sumeragi params` pour prouver que le binaire a charge
   `sumeragi.consensus_mode="npos"` et enregistrer `k_aggregators`,
   `redundant_send_r`, la longueur d epoch, et les offsets VRF commit/reveal.
2. Inspectez la vue runtime:

   ```bash
   iroha --output-format text ops sumeragi status
   iroha --output-format text ops sumeragi collectors
   iroha --output-format text ops sumeragi rbc status
   ```

   La ligne `status` imprime le tuple leader/view, le backlog RBC, les retries DA,
   les offsets d epoch et les deferrals du pacemaker; `collectors` mappe les indices
   de collectors vers des IDs de peers pour montrer quels validateurs portent les
   taches d alea a la hauteur inspectee.
3. Capturez le numero d epoch a auditer:

   ```bash
   EPOCH=$(curl -s "$TORII/status" | jq '.sumeragi.epoch.height // 0')
   printf "auditing epoch %s\n" "$EPOCH"
   ```

   Conservez la valeur (decimal ou prefixe `0x`) pour les commandes VRF ci-dessous.

## 2. Snapshot des epochs VRF et penalites

Utilisez les sous-commandes CLI dediees pour extraire les enregistrements VRF
persistes de chaque validateur:

```bash
iroha --output-format text ops sumeragi vrf-epoch --epoch "$EPOCH"
iroha ops sumeragi vrf-epoch --epoch "$EPOCH" > artifacts/vrf_epoch_${EPOCH}.json

iroha --output-format text ops sumeragi vrf-penalties --epoch "$EPOCH"
iroha ops sumeragi vrf-penalties --epoch "$EPOCH" > artifacts/vrf_penalties_${EPOCH}.json
```

Les resumes indiquent si l epoch est finalise, combien de participants ont soumis
commits/reveals, la longueur du roster et la seed derivee. Le JSON capture la liste
des participants, le statut de penalite par signataire et la valeur `seed_hex`
utilisee par le pacemaker. Comparez le nombre de participants avec le roster de
staking et verifiez que les tableaux de penalites refletent les alertes declenchees
pendant les tests de chaos (late reveals dans `late_reveals`, validateurs forfeited
sous `no_participation`).

## 3. Surveiller la telemetrie VRF et les alertes

Prometheus expose les compteurs requis par le roadmap:

- `sumeragi_vrf_commits_emitted_total`
- `sumeragi_vrf_reveals_emitted_total`
- `sumeragi_vrf_reveals_late_total`
- `sumeragi_vrf_non_reveal_penalties_total`
- `sumeragi_vrf_non_reveal_by_signer{signer="peer_id"}`
- `sumeragi_vrf_no_participation_total`
- `sumeragi_vrf_no_participation_by_signer{signer="peer_id"}`
- `sumeragi_vrf_rejects_total_by_reason{reason="..."}`

Exemple PromQL pour le rapport hebdomadaire:

```promql
increase(sumeragi_vrf_non_reveal_by_signer[1w]) > 0
```

Pendant les drills de readiness, confirmez que:

- `sumeragi_vrf_commits_emitted_total` et `..._reveals_emitted_total` augmentent
  pour chaque bloc dans les fenetres commit/reveal.
- Les scenarios late-reveal declenchent `sumeragi_vrf_reveals_late_total` et
  effacent l entree correspondante dans le JSON `vrf_penalties`.
- `sumeragi_vrf_no_participation_total` ne monte que lorsque vous retenez
  volontairement les commits pendant les tests de chaos.

Le tableau de bord Grafana (`docs/source/grafana_sumeragi_overview.json`) inclut
un panneau pour chaque compteur; capturez des screenshots apres chaque run et
attachez-les au bundle d artefacts reference dans {doc}`sumeragi_chaos_performance_runbook`.

## 4. Ingestion d evidence et streaming

Les evidences de slashing doivent etre collectees sur chaque validateur et relaiees
vers Torii. Utilisez les helpers CLI pour demontrer la parite avec les endpoints HTTP
documentes dans {doc}`torii/sumeragi_evidence_app_api`:

```bash
# Count and list persisted evidence
iroha --output-format text ops sumeragi evidence count
iroha --output-format text ops sumeragi evidence list --limit 5

# Show JSON for audits
iroha ops sumeragi evidence list --limit 100 > artifacts/evidence_snapshot.json
```

Verifiez que le `total` reporte correspond au widget Grafana alimente par
`sumeragi_evidence_records_total`, et confirmez que les enregistrements plus anciens
que `sumeragi.npos.reconfig.evidence_horizon_blocks` sont rejetes (le CLI imprime
la raison). Lors des tests d alerting, soumettez un payload connu via:

```bash
iroha --output-format text ops sumeragi evidence submit --evidence-hex-file fixtures/evidence/double_prevote.hex
```

Surveillez `/v1/events/sse` avec un stream filtre pour prouver que les SDK voient
les memes donnees: reutilisez le one-liner Python de {doc}`torii/sumeragi_evidence_app_api`
pour construire le filtre et capturez les frames `data:` brutes. Les payloads SSE
 doivent echoer le kind d evidence et le signer vu dans la sortie CLI.

## 5. Packaging et reporting des evidences

Pour chaque rehearsal ou release candidate:

1. Stockez les fichiers JSON CLI (`vrf_epoch_*.json`, `vrf_penalties_*.json`,
   `evidence_snapshot.json`) sous le repertoire d artefacts du run (le meme
   root que les scripts chaos/performance).
2. Enregistrez les resultats Prometheus ou exports snapshot pour les compteurs
   listes ci-dessus.
3. Attachez la capture SSE et les acknowledgements d alertes au README d artefacts.
4. Mettez a jour `status.md` et
   `docs/source/project_tracker/npos_sumeragi_phase_a.md` avec les chemins
   d artefacts et le numero d epoch inspecte.

Suivre cette checklist garde les preuves d alea VRF et d evidence de slashing
 auditables pendant le rollout NPoS et fournit aux reviewers governance une trace
 deterministe vers les metriques capturees et les snapshots CLI.

## 6. Signaux de troubleshooting

- **Mode selection mismatch** - Si `iroha --output-format text ops sumeragi params` montre
  `consensus_mode="permissioned"` ou `k_aggregators` differe du manifest,
  supprimez les artefacts captures, corrigez `iroha_config`, redemarrez le validateur,
  et relancez le flux de validation decrit dans {doc}`sumeragi`.
- **Missing commits or reveals** - Une serie plate de `sumeragi_vrf_commits_emitted_total`
  ou `sumeragi_vrf_reveals_emitted_total` signifie que Torii ne relaie pas les frames VRF.
  Verifiez les logs du validateur pour des erreurs `handle_vrf_*`, puis re-soumettez le
  payload manuellement via les helpers POST documentes ci-dessus.
- **Unexpected penalties** - Quand `sumeragi_vrf_no_participation_total` grimpe,
  verifiez le fichier `vrf_penalties_<epoch>.json` pour confirmer l ID du signer et
  comparez-le au roster de staking. Des penalites qui ne correspondent pas aux drills
  de chaos indiquent un decalage d horloge du validateur ou une protection replay Torii;
  corrigez le peer fautif avant de relancer le test.
- **Evidence ingestion stalls** - Quand `sumeragi_evidence_records_total` stagne alors
  que les tests de chaos emettent des fautes, lancez `iroha ops sumeragi evidence count`
  sur plusieurs validateurs et confirmez que `/v1/sumeragi/evidence/count` correspond
  a la sortie CLI. Toute divergence signifie que les consommateurs SSE/webhook peuvent
  etre stale, donc re-soumettez un fixture connu et escaladez aux mainteneurs Torii si
  le compteur ne s incremente pas.
