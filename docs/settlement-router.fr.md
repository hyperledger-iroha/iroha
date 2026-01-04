---
lang: fr
direction: ltr
source: docs/settlement-router.md
status: complete
translator: manual
source_hash: a00e929ef6088863db93ea122b7314ee921b9b37bea2c6e2c536082c29038f97
source_last_modified: "2025-11-12T08:35:29.309807+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traduction française de docs/settlement-router.md (Deterministic Settlement Router) -->

# Routeur de Règlement Déterministe (NX-3)

**Statut :** Planifié (NX-3)  
**Responsables :** Economics WG / Core Ledger WG / Treasury / SRE  
**Périmètre :** Implémente la politique unifiée de conversion vers XOR et de buffer décrite
dans l’item de roadmap « Unified lane settlement & XOR conversion (NX-3) ».

Ce document formalise l’algorithme, la télémétrie et les contrats opérationnels du routeur
de règlement unifié. Il complète le résumé des frais dans `docs/source/nexus_fee_model.md`
en expliquant comment le routeur dérive les débits en XOR, interagit avec le DEX AMM/CLMM
canonique (ou le RFQ gouverné de repli) et expose des reçus déterministes pour chaque
lane.

## Objectifs

   flux AMX cross‑DS—utilisent le même pipeline de règlement, de sorte que les
   rapprochements nocturnes n’ont besoin que d’un seul modèle.
2. **Conversion déterministe :** Les montants dus en XOR sont dérivés de paramètres
   on‑chain (TWAP, épsilon de volatilité, tier de liquidité) et arrondis à l’unité
   micro‑XOR pour que chaque nœud produise le même reçu.
3. **Buffers de sécurité :** Chaque dataspace/lane maintient un buffer XOR P95 sur 72 h.
   Le routeur débite d’abord les buffers, applique des portes de type throttle/XOR‑only
   lorsque les niveaux baissent et exporte des métriques pour que les opérateurs puissent
   recharger de manière proactive.
4. **Liquidité gouvernée :** Les conversions privilégient le DEX AMM/CLMM de lane public
   canonique ; les RFQ/swap‑lines gouvernés prennent le relais quand la profondeur
   diminue ou que des coupe‑circuits (circuit breakers) se déclenchent.

## Architecture

| Composant | Emplacement | Responsabilité |
|----------|-------------|----------------|
| `crates/settlement_router/` | Crate du routeur (nouveau) | Calcul du shadow price, haircuts, comptabilité de buffer, construction de planning, suivi d’exposition P&L. |
| `crates/oracle_adapter/` | Middleware d’oracle | Maintient la fenêtre TWAP (60 s par défaut) + volatilité glissante, émet un prix de circuit si les flux deviennent obsolètes. |
| `crates/iroha_core/src/fees` et `LaneBlockCommitment` | Core ledger | Persiste les reçus de lane, la variance en XOR et les métadonnées de swap dans les preuves de bloc pour les auditeurs. |
| DEX AMM/CLMM canonique et RFQ gouverné | Équipe DEX/AMM | Exécute des swaps déterministes ; fournit profondeur + tiers de haircut (tier1=profond → 0 bp, tier2=moyen → 25 bp, tier3=faible → 75 bp). |
| Lignes de swap du Trésor / APIs sponsor | Financial Services WG | Permettent à des sponsors ou MM autorisés (≤25 % de Bmin) de recharger les buffers sans attendre les swaps de marché. |
| Télémétrie + dashboards | `iroha_telemetry`, `dashboards/…` | Émet `iroha_settlement_buffer_xor`, `iroha_settlement_pnl_xor`, `iroha_settlement_haircut_bp`, etc. ; les dashboards déclenchent des alertes sur les seuils de buffer. |

## Pipeline de conversion

1. **Agrégation des reçus**
   - Chaque transaction/branche AMX émet un `LaneSettlementReceipt` avec le montant local
     en micro, le XOR dû, le tier de haircut, la variance réalisée (`xor_variance_micro`)
     et les métadonnées de l’appelant (déjà défini dans `docs/source/nexus_fee_model.md`).
   - Le routeur regroupe les reçus par `(lane, dataspace)` pendant l’exécution du bloc.

2. **Shadow price (`shadow_price.rs`)**
   - Récupère le TWAP 60 s (`twap_local_per_xor`) et un échantillon de volatilité sur 30 s.
   - Calcule épsilon : base 25 bp + (bucket de volatilité × 25 bp, plafonné à 75 bp).
   - Persiste la `volatility_class` résolue (`stable`, `elevated`, `dislocated`) dans
     `LaneSwapMetadata` afin que les auditeurs et dashboards puissent faire le lien entre
     la marge supplémentaire et le régime d’oracle capturé dans
     `pipeline.gas.units_per_gas`.
   - Calcule `xor_due = ceil(local_micro / twap × (1 + epsilon))`.
   - Stocke le triplet `(twap, epsilon, tier)` dans les métadonnées de swap pour
     l’audit.

3. **Haircut + variance (`haircut.rs`)**
   - Applique les haircuts selon le profil de liquidité (tier1=0 bp, tier2=25 bp,
     tier3=75 bp).
   - Enregistre `total_xor_after_haircut` et `total_xor_variance` (différence entre le
     dû et le montant après haircut) pour que le Trésor puisse voir combien de marge de
     sécurité chaque tier consomme.

4. **Débit de buffer (`buffer.rs`)**
   - Chaque buffer de lane/dataspace définit Bmin = dépense XOR P95 sur 72 h.
   - États : **Normal** (≥75 %), **Alert** (<75 %), **Throttle** (<25 %),
     **XOR‑only** (<10 %), **Halt** (<2 % ou oracle obsolète).
   - Le routeur débite les buffers par reçu ; lorsque les seuils sont franchis, il émet
     une télémétrie déterministe et applique des throttles (par ex. division par deux du
     taux d’inclusion en dessous de 25 %).
   - Les métadonnées de lane doivent déclarer la réserve du Trésor via :
     - `metadata.settlement.buffer_account` – ID de compte détenant la réserve (ex. :
       `buffer::cbdc_treasury`).
     - `metadata.settlement.buffer_asset` – Asset débité pour le headroom (généralement
       `xor#sora`).
     - `metadata.settlement.buffer_capacity_micro` – Capacité exprimée en micro‑XOR
       (chaîne décimale).
   - La télémétrie expose `iroha_settlement_buffer_xor` (headroom restant),
     `iroha_settlement_buffer_capacity_xor` et `iroha_settlement_buffer_state` (état
     discret).

5. **Planification des swaps (`schedule.rs`)**
   - Regroupe les reçus par asset local et tier de liquidité afin de minimiser le nombre
     de swaps.
   - Respecte les limites de taille/quantité pour chaque ordre AMM/CLMM.
   - Produit un `LaneSwapSchedule` déterministe décrivant :
     - l’asset local agrégé ;
     - le montant cible en XOR ;
     - le DEX/tier sélectionné ;
     - le haircut appliqué ;
     - le lane/dataspace impacté.

6. **Règlement et preuves de bloc**
   - Le scheduler envoie les ordres à la couche DEX. Le résultat de chaque swap est codé
     dans `LaneSwapMetadata` et annexé à `LaneBlockCommitment`.
   - Les auditeurs peuvent reconstruire la conversion XOR‑local à partir de :
     - TWAP et volatilité ;
     - tier de liquidité ;
     - haircut appliqué ;
     - variance totale en XOR.

## Télémétrie et alertes

Métriques minimales à exposer :

- `iroha_settlement_buffer_xor{dataspace,lane}` – headroom restant en XOR.
- `iroha_settlement_buffer_capacity_xor{dataspace,lane}` – capacité Bmin configurée.
- `iroha_settlement_buffer_state{dataspace,lane}` – état discret (`normal`, `alert`,
  `throttle`, `xor_only`, `halt`).
- `iroha_settlement_pnl_xor{dataspace,lane}` – P&L XOR cumulé depuis le lancement.
- `iroha_settlement_haircut_bp{dataspace,lane,tier}` – haircut effectif appliqué.

Dashboards et alertes :

- Déclencher des alertes lorsque l’état passe à `alert` (<75 %) ou pire.
- Construire des tableaux de bord montrant le headroom par lane/dataspace et la
  consommation quotidienne.
- Mettre en avant les lanes dont la variance cumulée (`total_xor_variance`) dépasse la
  plage attendue.

## Stratégie de déploiement

Il est recommandé d’activer le routeur par étapes :

1. **Shadow mode :** activer les calculs du routeur + la télémétrie, mais garder
2. **Activation des débits :** passer `settlement_debit=true` pour les lanes à faible
   risque (profil C) et vérifier les rapprochements.
3. **Conversions actives :** activer les tranches TWAMM + swaps AMM quand la télémétrie
   montre des buffers stables ; maintenir le RFQ de repli prêt.
4. **Intégration AMX :** s’assurer que les lanes AMX propagent les reçus et exposent
   `SETTLEMENT_ROUTER_UNAVAILABLE` aux SDK pour autoriser les retries.
   conditionner les merges futurs au respect des seuils de télémétrie de règlement.

Suivre ce document garantit que la tâche NX‑3 du roadmap fournit un comportement
déterministe, des artefacts auditables et des recommandations claires pour les
développeurs comme pour les opérateurs.
