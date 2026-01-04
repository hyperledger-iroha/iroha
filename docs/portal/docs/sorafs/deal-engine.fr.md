<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a85de93fe3864e11479242bbe76b6e1b96f854b548dc0ef78c4a6a4504d31bba
source_last_modified: "2025-11-15T07:12:49.005624+00:00"
translation_last_reviewed: 2025-12-29
---

---
id: deal-engine
title: Moteur d'accords SoraFS
sidebar_label: Moteur d'accords
description: Vue d'ensemble du moteur d'accords SF-8, de l'intégration Torii et des surfaces de télémétrie.
---

:::note Source canonique
:::

# Moteur d'accords SoraFS

La piste de roadmap SF-8 introduit le moteur d'accords SoraFS, fournissant
une comptabilité déterministe pour les accords de stockage et de récupération entre
clients et fournisseurs. Les accords sont décrits via les payloads Norito
définis dans `crates/sorafs_manifest/src/deal.rs`, couvrant les termes de l'accord,
le verrouillage de bonds, les micropaiements probabilistes et les enregistrements de règlement.

Le worker SoraFS embarqué (`sorafs_node::NodeHandle`) instancie désormais un
`DealEngine` pour chaque processus de nœud. Le moteur :

- valide et enregistre les accords via `DealTermsV1` ;
- cumule des charges libellées en XOR lorsque l'usage de réplication est rapporté ;
- évalue les fenêtres de micropaiement probabiliste via un échantillonnage déterministe
  basé sur BLAKE3 ; et
- produit des snapshots de ledger et des payloads de règlement adaptés à la publication
  de gouvernance.

Les tests unitaires couvrent la validation, la sélection des micropaiements et les flux de
règlement afin que les opérateurs puissent exercer les APIs en confiance. Les règlements
émettent désormais des payloads de gouvernance `DealSettlementV1`, s'intégrant directement
au pipeline de publication SF-12, et mettent à jour la série OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) pour les dashboards Torii et
l'application des SLO. Les travaux suivants ciblent l'automatisation du slashing initiée par
les auditeurs et la coordination des sémantiques d'annulation avec la politique de gouvernance.

La télémétrie d'usage alimente aussi l'ensemble de métriques `sorafs.node.micropayment_*` :
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, et les compteurs de tickets
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Ces totaux exposent le flux de loterie
probabiliste pour que les opérateurs puissent corréler les gains de micropaiements et
le carry-over de crédit avec les résultats de règlement.

## Intégration Torii

Torii expose des endpoints dédiés pour que les fournisseurs signalent l'usage et pilotent
le cycle de vie des accords sans wiring spécifique :

- `POST /v1/sorafs/deal/usage` accepte la télémétrie `DealUsageReport` et renvoie
  des résultats de comptabilité déterministes (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` finalise la fenêtre courante, en streamant le
  `DealSettlementRecord` résultant avec un `DealSettlementV1` encodé en base64
  prêt pour publication dans le DAG de gouvernance.
- Le feed `/v1/events/sse` de Torii diffuse désormais des enregistrements
  `SorafsGatewayEvent::DealUsage` résumant chaque soumission d'usage (epoch, GiB-heures mesurés,
  compteurs de tickets, charges déterministes), des enregistrements
  `SorafsGatewayEvent::DealSettlement` qui incluent le snapshot canonique du ledger de règlement
  ainsi que le digest/taille/base64 BLAKE3 de l'artefact de gouvernance sur disque, et des alertes
  `SorafsGatewayEvent::ProofHealth` dès que les seuils PDP/PoTR sont dépassés (fournisseur, fenêtre,
  état de strike/cooldown, montant de pénalité). Les consommateurs peuvent filtrer par fournisseur
  pour réagir à de nouvelles télémétries, des règlements ou des alertes de santé des proofs sans polling.

Les deux endpoints participent au framework de quotas SoraFS via la nouvelle fenêtre
`torii.sorafs.quota.deal_telemetry`, permettant aux opérateurs d'ajuster le taux de soumission
autorisé par déploiement.
