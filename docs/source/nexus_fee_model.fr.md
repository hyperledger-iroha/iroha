---
lang: fr
direction: ltr
source: docs/source/nexus_fee_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 532c57a0dae54224af0d30640edf8a3cbc8ac9a1df7d73b563bd16c3a635aec1
source_last_modified: "2026-01-08T19:45:50.411145+00:00"
translation_last_reviewed: 2026-01-08
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
- Chaque transaction doit porter le champ type et lie a la signature
  `fee_payment` (`FeePaymentIntent`). Il choisit comme payeur l'autorite ou
  un programme sponsor exact avec sa revision immuable, et contient les maxima
  signes par composant ainsi qu'une limite de gas positive si necessaire. Les
  anciennes cles de metadonnees `fee_sponsor`, `gas_limit` et
  `gas_asset_id` sont rejetees.
- Demandez un devis avant de signer : construisez le payload non signe exact,
  faites authentifier `POST /v1/fees/quote` par son autorite, inspectez
  l'intent recommande, remplacez uniquement `payload.fee_payment`, puis
  signez et soumettez ce meme payload. Le devis est une observation, pas une
  reservation ; l'admission reverifie l'etat courant.
- Le settlement direct accepte l'autorite ou un programme sponsor exact. Le
  settlement par receipts (`lane_relay_burn`) est reserve a un sponsor exact :
  les frais Nexus payes par l'autorite sont rejetes avec
  `relay_capacity_unavailable`, car son solde n'est pas un verrou de source
  de receipt authentifie.
- Chaque transaction qui paye du gas enregistre un `LaneSettlementReceipt`. Chaque recu stocke
  l'identifiant de source fourni par l'appelant, le micro-montant local, le XOR a payer
  immediatement, le XOR attendu apres le haircut, la marge de securite realisee
  (`xor_variance`), et l'horodatage du bloc en millisecondes.
- L'execution du bloc agrege les recus par lane/dataspace et les publie via `lane_settlement_commitments`
  dans `/v1/sumeragi/status`. Les totaux exposent `total_local_amount`, `total_xor_due`, et
  `total_xor_after_haircut` additionnes sur le bloc pour les exports nocturnes de reconciliation.
- Un nouveau compteur `total_xor_variance` suit la marge de securite consommee (difference entre
  le XOR du et l'attendu post-haircut), et `swap_metadata` documente les parametres deterministes de
  conversion (TWAP, epsilon, liquidity profile, et volatility_class) afin que les auditeurs puissent
  verifier les entrees du quote independamment de la configuration runtime.

Les consommateurs peuvent observer `lane_settlement_commitments` aux cotes des snapshots de
commitments lane et dataspace existants afin de verifier que les buffers de frais, les tiers de
haircut et l'execution du swap correspondent au modele de frais Nexus configure.
