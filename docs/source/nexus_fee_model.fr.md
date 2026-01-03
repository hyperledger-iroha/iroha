---
lang: fr
direction: ltr
source: docs/source/nexus_fee_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e02872dbcb6d92d8be4d40fc2864f28fc6564391640a6ea67768a1f837b57e0f
source_last_modified: "2025-11-15T20:09:59.438546+00:00"
translation_last_reviewed: 2026-01-01
---

# Mises a jour du modele de frais Nexus

Le routeur de settlement unifie capture maintenant des recus deterministes par lane afin que les
operateurs puissent reconciler les debits de gas avec le modele de frais Nexus.

- Pour l'architecture complete du routeur, la politique de buffer, la matrice de telemetrie et la
  sequence de rollout, voir `docs/settlement-router.md`. Ce guide explique comment les parametres
  documentes ici se rattachent au livrable du roadmap NX-3 et comment les SREs doivent surveiller le
  routeur en production.
- La configuration de l'asset gas (`pipeline.gas.units_per_gas`) inclut un decimal `twap_local_per_xor`,
  un `liquidity_profile` (`tier1`, `tier2`, ou `tier3`), et une `volatility_class` (`stable`,
  `elevated`, `dislocated`). Ces drapeaux alimentent le settlement router pour que le quote XOR
  resultant corresponde au TWAP canonique et au tier de haircut de la lane.
- Chaque transaction qui paye du gas enregistre un `LaneSettlementReceipt`. Chaque recu stocke
  l'identifiant de source fourni par l'appelant, le micro-montant local, le XOR a payer
  immediatement, le XOR attendu apres le haircut, la marge de securite realisee
  (`xor_variance_micro`), et l'horodatage du bloc en millisecondes.
- L'execution du bloc agrege les recus par lane/dataspace et les publie via `lane_settlement_commitments`
  dans `/v1/sumeragi/status`. Les totaux exposent `total_local_micro`, `total_xor_due_micro`, et
  `total_xor_after_haircut_micro` additionnes sur le bloc pour les exports nocturnes de reconciliation.
- Un nouveau compteur `total_xor_variance_micro` suit la marge de securite consommee (difference entre
  le XOR du et l'attendu post-haircut), et `swap_metadata` documente les parametres deterministes de
  conversion (TWAP, epsilon, liquidity profile, et volatility_class) afin que les auditeurs puissent
  verifier les entrees du quote independamment de la configuration runtime.

Les consommateurs peuvent observer `lane_settlement_commitments` aux cotes des snapshots de
commitments lane et dataspace existants afin de verifier que les buffers de frais, les tiers de
haircut et l'execution du swap correspondent au modele de frais Nexus configure.
