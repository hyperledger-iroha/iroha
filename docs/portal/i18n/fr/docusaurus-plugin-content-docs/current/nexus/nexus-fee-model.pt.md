---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : modèle de frais-nexus
titre : Actualisations du modèle de taxas du Nexus
description : Espelho de `docs/source/nexus_fee_model.md`, documentando recibos de liquidacao de lanes e superficies de conciliacao.
---

:::note Fonte canonica
Cette page reflète `docs/source/nexus_fee_model.md`. Mantenha as duas copias alinhadas enquanto as traducoes japonesas, hebraicas, espanholas, portuguesas, francesas, russas, arabes et urdu migram.
:::

# Actualisations du modèle de taxons du Nexus

Le distributeur de liquidation unifié vient de capturer des recettes déterminées par la voie pour que les opérateurs possèdent des débits de gaz conciliaires avec le modèle de taxes Nexus.- Pour l'architecture complète du rotor, la politique de tampon, la matrice de télémétrie et le séquencement de déploiement, voir `docs/settlement-router.md`. Ce guide explique comment les paramètres documentés ici sont connectés à l'entregavel de la feuille de route NX-3 et comment les SRE développent un moniteur ou un rotor en production.
- La configuration de l'activité de gaz (`pipeline.gas.units_per_gas`) comprend un décimal `twap_local_per_xor`, un `liquidity_profile` (`tier1`, `tier2`, ou `tier3`), et un `volatility_class` (`stable`, `elevated`, `dislocated`). Il s'agit d'un transformateur qui alimente le distributeur de liquide pour que le paiement XOR résultant corresponde au canon TWAP et au niveau de coupe de cheveux de la voie.
- Cada transacao que paga gas registra um `LaneSettlementReceipt`. Chaque fois que vous recevez l'identifiant d'origine fourni par le chaman, la micro-valeur locale, le XOR est immédiatement envoyé, le XOR s'attend à la coupe de cheveux, une variation réalisée (`xor_variance_micro`) et l'horodatage du blocage des erreurs.
- L'exécution des blocs agrégés par la voie/espace de données et l'appareil public via `lane_settlement_commitments` dans `/v2/sumeragi/status`. Les totaux affichés `total_local_micro`, `total_xor_due_micro` et `total_xor_after_haircut_micro` ne sont pas bloqués pour les exportations nocturnes de conciliation.- Un nouveau contact `total_xor_variance_micro` rastreia quanto de margem de seguranca foi consumida (diferenca entre o XOR devido et o esperado pos-haircut), et `swap_metadata` documente les paramètres de conversation déterministique (TWAP, epsilon, profil de liquidité et volatilité_class) pour que les auditeurs possèdent vérifier les entrées du côté indépendamment de la configuration lors de l'exécution.

Les consommateurs peuvent accompagner `lane_settlement_commitments` avec les instantanés existants des engagements de voie et d'espace de données pour vérifier que les tampons de taxes, les niveaux de coupe de cheveux et l'exécution du swap correspondent au modèle de taxes configuré par Nexus.