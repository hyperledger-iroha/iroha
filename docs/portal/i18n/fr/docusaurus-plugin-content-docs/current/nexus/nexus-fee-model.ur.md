---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : modèle de frais-nexus
titre : Nexus فیس ماڈل اپ ڈیٹس
description : `docs/source/nexus_fee_model.md` pour les reçus de règlement de voie et les surfaces de rapprochement
---

:::note کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_fee_model.md` کی عکاسی کرتا ہے۔ جاپانی، عبرانی، ہسپانوی، پرتگالی، فرانسیسی، روسی، عربی اور اردو ترجمے migrate ہونے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Nexus فیس ماڈل اپ ڈیٹس

Routeur de règlement sur la voie pour les reçus déterministes et les débits de gaz Nexus pour les débits de gaz مطابق réconcilier کر سکیں۔- Routeur, architecture, politique de tampon, matrice de télémétrie et séquençage du déploiement pour `docs/settlement-router.md`. Voici les paramètres de la feuille de route NX-3 livrable pour les SRE et la production du routeur نگرانی کیسے کرنی چاہئے۔
- Configuration des actifs gaziers (`pipeline.gas.units_per_gas`) ou `twap_local_per_xor` décimal, `liquidity_profile` (`tier1`, `tier2`, ou `tier3`) et `volatility_class` (`stable`, `elevated`, `dislocated`) یہ flags règlement routeur کو flux ہوتے ہیں تاکہ حاصل ہونے والی XOR quote, canonique TWAP اور lane کے haircut tier سے میل کھائے۔
- Paiement du gaz et transaction `LaneSettlementReceipt` ریکارڈ کرتی ہے۔ L'appelant de réception est un identifiant de source, un micro-montant local, un micro-montant local, une coupe de cheveux ou un XOR attendu, une variance (`xor_variance_micro`) et un horodatage de bloc. (millisecondes) محفوظ کرتا ہے۔
- Réceptions d'exécution de blocs par voie/espace de données pour un agrégat global et un `/v1/sumeragi/status` et un `lane_settlement_commitments` pour un bloc ہے۔ totaux میں `total_local_micro`, `total_xor_due_micro`, اور `total_xor_after_haircut_micro` شامل ہوتے ہیں جو block پر جمع کر کے exports de réconciliation nocturne کے لئے فراہم ہوتے ہیں۔- Le compteur `total_xor_variance_micro` et la piste de suivi sont dotés d'une marge de sécurité supérieure (en raison du XOR et des attentes post-coupe de cheveux) Paramètres de conversion déterministes `swap_metadata` (TWAP, epsilon, profil de liquidité, classe de volatilité) et configuration d'exécution des auditeurs pour les entrées de cotation et les entrées de devis.

Consommateurs `lane_settlement_commitments` entre voies et instantanés d'engagement d'espace de données et tampons de frais, niveaux de coupe de cheveux, Pour l'exécution du swap configuré, modèle de frais Nexus