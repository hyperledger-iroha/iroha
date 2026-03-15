---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : moteur de transaction
Titre : Lecteur de disque SoraFS
sidebar_label : Modèle de lecteur
description: Обзор движка сделок SF-8, intégration Torii et technologie télémétrique.
---

:::note Канонический источник
:::

# Lecteur de disque SoraFS

Le plan SF-8 pour votre lecteur de disque SoraFS, approprié
Mesures de sécurité et de fonctionnement
Clients et fournisseurs. Соглашения описываются Charges utiles Norito,
определенными в `crates/sorafs_manifest/src/deal.rs`, покрывая условия сделки,
blocage des obligations, microplaquettes vérifiées et protection contre les rayures.

Le travailleur SoraFS (`sorafs_node::NodeHandle`) est disponible
Exemple `DealEngine` pour chaque processus que vous utilisez. Vidéo :

- valider et enregistrer les modèles à partir de `DealTermsV1` ;
- payer les frais de XOR lors de l'utilisation de réplications ;
- оценивает окна вероятностных микроплатежей с помощью детерминированного
  échantillonnage sur BLAKE3; je
- Former un grand livre d'instantanés et des charges utiles pour la gouvernance.Des tests de validation pour des microplaques et des ustensiles de cuisine de qualité supérieure
Les opérateurs peuvent vérifier l'API. Расчеты теперь выпускают gouvernance charges utiles
`DealSettlementV1`, pour ajouter des publications sur le pipeline SF-12 et mettre à jour la série
OpenTélémétrie `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) pour les passeports Torii et
Application du SLO. La section suivante se concentre sur l'automatisation du slashing, l'exploration
Les auditeurs et les législateurs se rapportent à la gouvernance politique.

Le système de télémétrie utilise la mesure `sorafs.node.micropayment_*` :
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, pour les billets de banque
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Эти totals раскрывают вероятностный
Les magasins d'usine, les opérateurs peuvent corréler les microplaques et
Les crédits sont élevés avec les résultats.

## Intégration Torii

Torii préserve vos points de terminaison, permettant aux fournisseurs d'exploiter l'utilisation et les fichiers.
Il s'agit d'un modèle sans câblage spécial :- `POST /v1/sorafs/deal/usage` utilise le télémètre `DealUsageReport` et le produit.
  детерминированные результаты учета (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` завершает текущий fenêtre, стримя
  Il s'agit de `DealSettlementRecord` avec le module base64 `DealSettlementV1`,
  готовым к публикации в gouvernance DAG.
- Lenta Torii `/v1/events/sse` теперь транслирует записи `SorafsGatewayEvent::DealUsage`,
  суммирующие каждую отправку utilisation (époque, измеренные GiB-heures, счетчики билетов,
  детерминированные charges), записи `SorafsGatewayEvent::DealSettlement`,
  plus de captures d'instantanés canoniques et de registre BLAKE3 digest/size/base64
  éléments de gouvernance sur le disque et alertes `SorafsGatewayEvent::ProofHealth` avant les prévisions
  порогов PDP/PoTR (провайдер, окно, состояние strike/cooldown, сумма штрафа). Les propriétaires peuvent
  Filtrez les informations fournies par le fournisseur pour obtenir de nouvelles alertes télémétriques ou de preuve de santé en dehors des sondages.

Les points de terminaison sont utilisés dans le cadre de quota SoraFS aujourd'hui
`torii.sorafs.quota.deal_telemetry`, permettant à l'opérateur de configurer le dossier
частоту отправки для каждого деплоя.