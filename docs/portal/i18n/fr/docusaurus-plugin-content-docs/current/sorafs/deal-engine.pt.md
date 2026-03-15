---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : moteur de transaction
titre : Moteur d'accord avec le SoraFS
sidebar_label : Moteur d'accord
description : Visa général du moteur d'accord SF-8, intégré avec Torii et superficie de télémétrie.
---

:::note Fonte canonica
Cette page espelha `docs/source/sorafs/deal_engine.md`. Mantenha ambos os local alinhados quanto a documentacao alternativa permanent ativa.
:::

# Moteur d'accord pour SoraFS

La piste de la feuille de route SF-8 présente le moteur conforme au SoraFS, prévu
stabilité déterminée pour les accords d'armement et de récupération entre
clients et fournisseurs. Les descriptions sont conformes aux charges utiles Norito
défini dans `crates/sorafs_manifest/src/deal.rs`, cobrindo termos do acordo,
blocage des obligations, micropaiements probabilistes et registres de liquidation.

Le travailleur embauché par SoraFS (`sorafs_node::NodeHandle`) il y a quelques instants
`DealEngine` pour chaque processus de nœud. Ô moteur :

- valida e registra concordos usando `DealTermsV1` ;
- cumul de cobrancas libellés en XOR lors de l'utilisation de la réplication et du rapport ;
- avalia janelas de micropagamento probabilistico usando amostragem deterministica
  basé sur BLAKE3; e
- produire des instantanés du grand livre et des charges utiles de liquidation adéquates pour la publication
  de gouvernance.Testes unitarios cobrem validacao, selecão de micropagamentos e fluxos de liquidacao para
que les opérateurs peuvent utiliser les API avec confiance. Liquidacoes agora émetem
charges utiles de gouvernance `DealSettlementV1`, connectées directement au pipeline de
publication SF-12, et mise à jour de la série OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) pour les tableaux de bord de Torii et
application des SLO. Les éléments suivants se dirigent vers l'automatisation du lancement de la coupe par
les auditeurs et la coordination des sémantiques d'annulation avec la politique de gouvernance.

La télémétrie d'utilisation maintenant également l'alimentation ou le ensemble de mesures `sorafs.node.micropayment_*` :
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, et les contadores de tickets
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Esses totais expoem o fluxo de loterie
probabilité pour que les opérateurs puissent corréler les opérations de micro-paiement et
report de crédit avec résultats de liquidation.

## Intégration avec Torii

Torii expose les points de terminaison dédiés aux preuves de rapport d'utilisation et de vérification
cycle de vie en accord avec le câblage en toute sécurité :- `POST /v1/sorafs/deal/usage` chaîne de télémétrie `DealUsageReport` et retour
  resultados deterministas de contabilidade (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` finalise le message actuel, transmet le message
  `DealSettlementRecord` résultant d'un `DealSettlementV1` en base64
  Pronto para publicacao no DAG de gouvernance.
- Le flux `/v1/events/sse` vers Torii transmet les enregistrements `SorafsGatewayEvent::DealUsage`.
  reprendre chaque envoi d'utilisation (époque, GiB-heures, contadores de tickets,
  cobrancas deterministas), registres `SorafsGatewayEvent::DealSettlement`
  qui inclut l'instantané canonique du grand livre de liquidation mais aussi le digest/tamanho/base64
  BLAKE3 fait de l'art de gouverner en discothèque, et alertes `SorafsGatewayEvent::ProofHealth`
  semper que limiares PDP/PoTR sao excedidos (provedor, janela, estado de strike/cooldown,
  valeur de la pénalité). Les consommateurs peuvent filtrer le fournisseur pour réagir à la nouvelle
  telemetria, liquidacoes ou alertas de saude de proofs sem polling.

Les points de terminaison participent au framework de cotas de SoraFS via une nouvelle janvier
`torii.sorafs.quota.deal_telemetry`, permettant aux opérateurs d'ajuster les taxons d'envoi
autorisé à déployer.