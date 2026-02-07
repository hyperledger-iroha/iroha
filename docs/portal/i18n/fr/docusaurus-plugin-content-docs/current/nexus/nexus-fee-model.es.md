---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : modèle de frais-nexus
titre : Actualisations du modèle de tarifs de Nexus
description : En particulier de `docs/source/nexus_fee_model.md`, qui documente les recettes de liquidation des voies et les superficies de conciliation.
---

:::note Fuente canonica
Cette page reflète `docs/source/nexus_fee_model.md`. Manten ambas copias alineadas mientras las traducciones al japones, hebreo, espanol, portugues, frances, ruso, arabe y urdu migran.
:::

# Actualisations du modèle de tarifs de Nexus

Le routeur de liquidation unifié a maintenant capturé des recettes déterminées par la voie pour que les opérateurs puissent concilier les débits de gaz avec le modèle de tarifs de Nexus.- Pour l'architecture complète du routeur, la politique de tampon, la matrice de télémétrie et la sécurité de déploiement, ainsi que `docs/settlement-router.md`. Ce guide explique comment les paramètres documentés ici sont associés à la feuille de route NX-3 et comment les SRE doivent surveiller le routeur en production.
- La configuration de l'activité de gaz (`pipeline.gas.units_per_gas`) comprend un décimal `twap_local_per_xor`, un `liquidity_profile` (`tier1`, `tier2`, ou `tier3`) et un `volatility_class` (`stable`, `elevated`, `dislocated`). Ces bandes alimentent le routeur de liquidation pour que la cotisation XOR résultante coïncide avec le canon TWAP et le niveau de coupe de cheveux pour la voie.
- Chaque transaction de gaz enregistrée sous le numéro `LaneSettlementReceipt`. Chaque fois que vous recevez l'identifiant d'origine fourni par l'appelant, le micro-montage local, le XOR est demandé immédiatement, le XOR est attendu après la coupe de cheveux, la variante réalisée (`xor_variance_micro`) et la marque de temps de blocage en milisegundos.
- L'exécution des blocs s'ajoute aux voies/espaces de données et au public via `lane_settlement_commitments` et `/v1/sumeragi/status`. Les totaux exposés `total_local_micro`, `total_xor_due_micro` et `total_xor_after_haircut_micro` sont placés sur le blocage pour les exportations nocturnes de conciliation.- Un nouveau contador `total_xor_variance_micro` indique la marge de sécurité de consommation (différence entre la dette XOR et les attentes après la coupe de cheveux), et `swap_metadata` documente les paramètres de conversion déterministes (TWAP, epsilon, profil de liquidité et volatilité_class) pour que les auditeurs puissent vérifier la entrées de cotation indépendantes de la configuration au moment de l'exécution.

Les consommateurs peuvent observer `lane_settlement_commitments` conjointement avec les instantanés existants des engagements de voie et d'espace de données pour vérifier que les tampons de tarifs, les niveaux de coupe de cheveux et l'exécution du swap coïncident avec le modèle de tarifs de Nexus configuré.