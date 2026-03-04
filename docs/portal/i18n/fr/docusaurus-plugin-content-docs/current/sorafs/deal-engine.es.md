---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : moteur de transaction
titre : Moteur d'acuerdos de SoraFS
sidebar_label : Moteur d'accusés
description : Résumé du moteur d'acuerdos SF-8, intégration avec Torii et surface de télémétrie.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/deal_engine.md`. Gardez les emplacements alignés au milieu de la documentation héritée si active.
:::

# Moteur de capteurs de SoraFS

La ligne de route SF-8 présente le moteur de référence SoraFS, qui se porte
Contabilidad determinista para acuerdos de almacenamiento y recuperación entre
clients et fournisseurs. Les faits sont décrits avec les charges utiles Norito
défini en `crates/sorafs_manifest/src/deal.rs`, que cubren términos del acuerdo,
bloqueo de bonos, micropagos probabilísticos y registros de liquidación.

Le travailleur intégré à SoraFS (`sorafs_node::NodeHandle`) maintenant
`DealEngine` pour chaque processus de nœud. Le moteur :

- valida y registra acuerdos usando `DealTermsV1` ;
- cumul de cargaisons libellées en XOR lorsqu'elles sont signalées à l'utilisation de la réplication ;
- évaluer les fenêtres de micropago de façon probabiliste en utilisant un outil déterministe
  basado sur BLAKE3; oui
- produire des instantanés du grand livre et des charges utiles de liquidation adaptées à la publication
  de gobernanza.Las pruebas unitarias cubren validación, selección de micropagos y flujos de
liquidation pour que les opérateurs puissent exécuter les API avec confiance.
Les liquidations ont maintenant émis des charges utiles de gobernanza `DealSettlementV1`,
Connectez-vous directement au pipeline de publication SF-12 et actualisez la série.
OpenTélémétrie `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) pour les tableaux de bord de Torii et
application des SLO. Les étapes suivantes se concentrent sur l'automatisation du slashing
initié par les auditeurs et en coordonnant la sémántica d’annulation avec la politique de gouvernement.

La télémétrie d'utilisation alimente également le ensemble de mesures `sorafs.node.micropayment_*` :
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, et les contadores de tickets
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Ces sommes totales exposent le flux de loterie
probabilité pour que les opérateurs puissent corréler les victoires de micropagos et
le report de crédit avec les résultats de liquidation.

## Intégration avec Torii

Torii expose les points finaux dédiés pour que les fournisseurs signalent l'utilisation et la conduite du cycle
de la vie du travail sans câblage personnalisé :- `POST /v1/sorafs/deal/usage` accepter la télémétrie `DealUsageReport` et retourner
  resultados deterministas de contabilidad (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` finalise la fenêtre actuelle, transmet le
  `DealSettlementRecord` résultant avec un `DealSettlementV1` en base64
  liste de publication au DAG de gobernanza.
- Le flux `/v1/events/sse` de Torii transmet maintenant les enregistrements `SorafsGatewayEvent::DealUsage`
  que reprendre cada envío de uso (époque, GiB-hora medidos, contadores de tickets,
  cargos deterministas), registros `SorafsGatewayEvent::DealSettlement`
  qui inclut l'instantané canonique du grand livre de liquidation plus le digest/tamaño/base64
  BLAKE3 de l'artefact de gouvernance en discothèque et alertes `SorafsGatewayEvent::ProofHealth`
  Chaque fois que se dépassent les ombrelles PDP/PoTR (proveedor, ventana, estado de strike/cooldown,
  montant de pénalité). Les consommateurs peuvent filtrer le fournisseur pour réagir à un
  nouvelle télémétrie, liquidations ou alertes de santé des essais sans sondage.

Ambos endpoints participe au framework de cuotas de SoraFS à travers la nouvelle fenêtre
`torii.sorafs.quota.deal_telemetry`, permettant aux opérateurs d'ajuster la tasse d'envoi
permitida por despliegue.