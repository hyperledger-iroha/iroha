---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-fee-model
title: Mises a jour du modele de frais Nexus
description: Miroir de `docs/source/nexus_fee_model.md`, documentant les recus de reglement de lanes et les surfaces de reconciliation.
---

:::note Source canonique
Cette page reflete `docs/source/nexus_fee_model.md`. Gardez les deux copies alignees pendant la migration des traductions japonaise, hebraique, espagnole, portugaise, francaise, russe, arabe et ourdoue.
:::

# Mises a jour du modele de frais Nexus

Le routeur de reglement unifie capture maintenant des recus deterministes par lane afin que les operateurs puissent reconcilier les debits de gas avec le modele de frais Nexus.

- Pour l'architecture complete du routeur, la politique de buffer, la matrice de telemetrie et la sequence de deploiement, voir `docs/settlement-router.md`. Ce guide explique comment les parametres documentes ici se rattachent au livrable du roadmap NX-3 et comment les SRE doivent surveiller le routeur en production.
- La configuration de l'actif de gas (`pipeline.gas.units_per_gas`) inclut un decimal `twap_local_per_xor`, un `liquidity_profile` (`tier1`, `tier2`, ou `tier3`), et une `volatility_class` (`stable`, `elevated`, `dislocated`). Ces indicateurs alimentent le routeur de reglement afin que la cotation XOR corresponde au TWAP canonique et au palier de haircut pour la lane.
- Chaque transaction qui paie du gas enregistre un `LaneSettlementReceipt`. Chaque recu stocke l'identifiant source fourni par l'appelant, le micro-montant local, le XOR a regler immediatement, le XOR attendu apres le haircut, la variance realisee (`xor_variance_micro`), et l'horodatage du bloc en millisecondes.
- L'execution de blocs agrege les recus par lane/dataspace et les publie via `lane_settlement_commitments` dans `/v2/sumeragi/status`. Les totaux exposent `total_local_micro`, `total_xor_due_micro`, et `total_xor_after_haircut_micro` additionnes sur le bloc pour les exports nocturnes de reconciliation.
- Un nouveau compteur `total_xor_variance_micro` suit la marge de securite consommee (difference entre le XOR du et l'attendu post-haircut), et `swap_metadata` documente les parametres de conversion deterministe (TWAP, epsilon, liquidity profile, et volatility_class) afin que les auditeurs puissent verifier les entrees de la cotation independamment de la configuration d'execution.

Les consommateurs peuvent suivre `lane_settlement_commitments` aux cotes des snapshots existants de commitments de lane et de dataspace pour verifier que les buffers de frais, les paliers de haircut et l'execution du swap correspondent au modele de frais Nexus configure.
