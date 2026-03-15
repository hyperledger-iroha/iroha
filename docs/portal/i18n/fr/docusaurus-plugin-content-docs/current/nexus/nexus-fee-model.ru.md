---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : modèle de frais-nexus
titre : Modèles de société Nexus
description: Zerkalo `docs/source/nexus_fee_model.md`, документирующее квитанции расчетов по lane и поверхности согласования.
---

:::note Канонический источник
Cette page correspond à `docs/source/nexus_fee_model.md`. Держите обе копии синхронизированными, пока мигрируют переводы на японский, иврит, испанский, португальский, французский, русский, arabe et ourdou.
:::

# Modèles de société Nexus

Les routeurs de votre routeur doivent déterminer les paramètres de votre entreprise sur la voie principale, les opérateurs peuvent s'occuper de la consommation d'essence. Modèle de société Nexus.- Il s'agit d'un routeur architectural complet, d'un routeur politique, d'un système de télémétrie matriciel et d'un déploiement ultérieur. `docs/settlement-router.md`. Ceci est dû aux paramètres que nous proposons, en lien avec la feuille de route post-NX-3 et avec SRE pour surveiller le routeur dans продакшене.
- La configuration du gaz actif (`pipeline.gas.units_per_gas`) active la configuration souhaitée `twap_local_per_xor`, `liquidity_profile` (`tier1`, `tier2` ou `tier3`) et `volatility_class` (`stable`, `elevated`, `dislocated`). Ce signal a été ajouté au routeur de règlement, ce qui permet à XOR de prendre en charge le canal TWAP et votre coupe de cheveux pour la voie.
- Lorsque la transmission est effectuée, le gaz est mis en marche `LaneSettlementReceipt`. Lorsque la réception est effectuée, vous avez préalablement besoin d'un identifiant, d'un micro-résumé local, XOR avec un système d'identification inconnu, et XOR est délivré après coupe de cheveux, une variété de faits (`xor_variance_micro`) et un bloc de plusieurs millions de personnes.
- Utiliser le bloc pour agréger les reçus sur la voie/l'espace de données et publier sur `lane_settlement_commitments` dans `/v2/sumeragi/status`. Il s'agit d'un `total_local_micro`, d'un `total_xor_due_micro` et d'un `total_xor_after_haircut_micro`, résumés dans le bloc pour les petits utilisateurs.- Le nouveau schéma `total_xor_variance_micro` s'applique, le cas échéant étant donné que le XOR et l'opération sont effectués après la coupe de cheveux), le `swap_metadata` documente les paramètres de conversion (TWAP, epsilon, profil de liquidité et volatilité_class), que les auditeurs peuvent vérifier. Les paramètres de configuration ne correspondent pas à la configuration.

Les utilisateurs peuvent utiliser `lane_settlement_commitments` pour les engagements liés à la voie et à l'espace de données, ce qui concerne les magasins de la société, Votre coupe de cheveux et votre utilisation d'échange sont adaptées aux modèles modernes de la société Nexus.