---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : modèle de frais-nexus
titre : Mises à jour du modèle de frais Nexus
description : Miroir de `docs/source/nexus_fee_model.md`, documentant les recus de règlement de voies et les surfaces de réconciliation.
---

:::note Source canonique
Cette page reflète `docs/source/nexus_fee_model.md`. Gardez les deux copies alignées pendant la migration des traductions japonaise, hébraïque, espagnole, portugaise, française, russe, arabe et ourdoue.
:::

# Mises à jour du modèle de frais Nexus

Le routeur de régulation unifie la capture maintenant des recus déterministes par voie afin que les opérateurs puissent concilier les débits de gaz avec le modèle de frais Nexus.- Pour l'architecture complète du routeur, la politique de buffer, la matrice de télémétrie et la séquence de déploiement, voir `docs/settlement-router.md`. Ce guide explique comment les paramètres documentés ici se rattachent au livrable du roadmap NX-3 et comment les SRE doivent surveiller le routeur en production.
- La configuration de l'actif de gas (`pipeline.gas.units_per_gas`) inclut un décimal `twap_local_per_xor`, un `liquidity_profile` (`tier1`, `tier2`, ou `tier3`), et une `volatility_class` (`stable`, `elevated`, `dislocated`). Ces indicateurs alimentent le routeur de règlement afin que la cotation XOR corresponde au TWAP canonique et au palier de coupe de cheveux pour la voie.
- Chaque transaction qui paie du gaz enregistre un `LaneSettlementReceipt`. Chaque fois que vous stockez l'identifiant source fourni par l'appelant, le micro-montant local, le XOR à régler immédiatement, le XOR attendu après la coupe de cheveux, la variance réalisée (`xor_variance_micro`), et l'horodatage du bloc en millisecondes.
- L'exécution de blocs agrége les recus par lane/dataspace et les publics via `lane_settlement_commitments` dans `/v1/sumeragi/status`. Les totaux exposent `total_local_micro`, `total_xor_due_micro`, et `total_xor_after_haircut_micro` additionnés sur le bloc pour les exportations nocturnes de réconciliation.- Un nouveau compteur `total_xor_variance_micro` suit la marge de sécurité consommée (différence entre le XOR du et l'attendu post-haircut), et `swap_metadata` documente les paramètres de conversion déterministe (TWAP, epsilon, liquid profile, et volatilité_class) afin que les auditeurs puissent vérifier les entrées de la cotation indépendamment de la configuration d'exécution.

Les consommateurs peuvent suivre `lane_settlement_commitments` aux cotes des snapshots existants de engagements de lane et de dataspace pour vérifier que les buffers de frais, les paliers de coupe de cheveux et l'exécution du swap correspondant au modèle de frais Nexus configurer.