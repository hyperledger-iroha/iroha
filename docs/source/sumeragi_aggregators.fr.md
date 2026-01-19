---
lang: fr
direction: ltr
source: docs/source/sumeragi_aggregators.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee79dba673794a3dd4f888d3daf39163a827443bb22d413ab4d7f2e252762293
source_last_modified: "2025-12-26T13:17:08.872635+00:00"
translation_last_reviewed: 2026-01-01
---

# Routage des agregateurs Sumeragi

## Apercu

Cette note capture la strategie deterministe de routage des collectors ("aggregators") utilisee par Sumeragi apres la mise a jour d equite de la Phase 3. Chaque validateur calcule le meme ordre de collectors pour une hauteur et une view de bloc donnees. Le design elimine la dependance a une alea ad hoc et maintient le fan-out normal des votes borne par la liste des collectors; quand les collectors sont indisponibles ou que le quorum stagne, les rebroadcasts de replanification reutilisent les cibles des collectors avec un fallback vers la topologie de commit.

## Selection deterministe

- Le nouveau module `sumeragi::collectors` expose `deterministic_collectors(topology, mode, k, seed, height, view)` qui renvoie un `Vec<PeerId>` reproductible pour le couple `(height, view)`.
- Le mode permissioned utilise une selection basee sur PRF avec une seed issue de l etat PRF/VRF de l epoch. Le helper derive un ordre deterministe par `(height, view)` depuis la topologie canonique et exclut le leader. Quand la seed PRF est indisponible, il retombe sur le segment de queue contigu pour conserver le determinisme.
- Le mode NPoS continue d utiliser le PRF par epoch, mais le helper centralise maintenant le calcul afin que chaque appelant recoive le meme ordre. La seed est derivee de l alea d epoch fournie par `EpochManager`.
- `CollectorPlan` suit la consommation des cibles ordonnees et enregistre si le fallback gossip a ete declenche. Les mises a jour de telemetrie (`collect_aggregator_ms`, `sumeragi_redundant_sends_*`, `sumeragi_gossip_fallback_total`) indiquent la frequence des fallbacks et la duree du fan-out redondant.

## Objectifs d equite

1. **Reproductibilite:** La meme topologie de validateurs, le meme mode de consensus, la seed PRF et le tuple `(height, view)` doivent conduire aux memes collectors primaires/secondaires sur chaque pair. Le helper masque les particularites de topologie (proxy tail, validateurs Set B) pour rendre l ordre portable entre composants et tests.
2. **Rotation:** La selection PRF fait tourner le collector primaire entre les hauteurs et vues dans les deux modes, evitant qu un seul validateur Set B possede en permanence les taches d aggregation. Le fallback au segment contigu n est utilise que lorsque la seed PRF manque.
3. **Observabilite:** La telemetrie continue de signaler les affectations par collector et le chemin de fallback emet un avertissement quand le gossip est engage afin que les operateurs detectent des collectors defaillants.

## Reessais et backoff gossip

- Les validateurs conservent un `CollectorPlan` dans l etat de proposition; le plan enregistre combien de collectors ont ete contactes et si la limite de fan-out redondant a ete atteinte.
- Les plans de collectors sont indexes par `(height, view)` et sont reinitialises lorsque le sujet change afin que des reessais de view-change perimes ne reutilisent pas d anciennes cibles.
- Le redundant send (`r`) est applique de facon deterministe en progressant dans le plan. Quand aucun collector n est disponible pour le tuple `(height, view)`, les votes se rabattent sur la topologie de commit complete (hors soi) pour eviter le deadlock.
- Quand le quorum stagne, le chemin de replanification rebroadcast les votes en cache via le plan de collectors, avec fallback vers la topologie de commit lorsque les collectors sont vides, locaux uniquement, ou sous quorum. Cela fournit un fallback "gossip" borne sans payer le cout d un broadcast complet sur le chemin rapide en regime etabli.
- Chaque rejet de proposition du au gate de locked QC incremente `block_created_dropped_by_lock_total`; les chemins de validation d en-tete en echec incrementent `block_created_hint_mismatch_total` et `block_created_proposal_mismatch_total`, aidant les operateurs a correler des fallbacks repetes avec des problemes de correction du leader. Le snapshot `/v1/sumeragi/status` exporte aussi les hashes les plus recents de Highest/Locked QC pour que les dashboards relient les pics de drop a des hashes de bloc precis.

## Resume d implementation

- Le nouveau module public `sumeragi::collectors` heberge `CollectorPlan` et `deterministic_collectors` afin que les tests au niveau crate et les tests d integration puissent verifier les proprietes d equite sans instancier l acteur de consensus complet.
- `CollectorPlan` vit dans l etat de proposition Sumeragi et est reinitialise lorsque le pipeline de proposition se termine.
- `Sumeragi` construit les plans de collectors via `init_collector_plan` et cible les collectors lors de l emission des votes availability/precommit. Les votes availability et precommit reviennent a la topologie de commit lorsque les collectors sont vides, locaux uniquement, ou sous quorum, et les rebroadcasts retombent dans les memes conditions.
- Les tests unitaires et d integration valident le determinisme PRF, la selection de fallback et les transitions d etat de backoff.

## Validation de revue

- Reviewed-by: Consensus WG
- Reviewed-by: Platform Reliability WG
