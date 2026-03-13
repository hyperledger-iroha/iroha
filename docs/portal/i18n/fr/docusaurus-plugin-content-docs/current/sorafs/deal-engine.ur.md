---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : moteur de transaction
titre : moteur de transaction SoraFS
sidebar_label : moteur de transactions
description : Moteur de transaction SF-8, intégration Torii et surfaces de télémétrie.
---

:::note مستند ماخذ
:::

# Moteur de transaction SoraFS

La feuille de route SF-8 suit le moteur de transaction SoraFS.
clients et fournisseurs de stockage et accords de récupération
comptabilité déterministe فراہم کرتا ہے۔ Accords sur les charges utiles Norito et sur les charges utiles
Il s'agit d'un produit `crates/sorafs_manifest/src/deal.rs` qui est en vente dans un magasin
conditions de transaction, verrouillage d'obligations, micropaiements probabilistes et enregistrements de règlement et paiements en espèces

Worker SoraFS intégré (`sorafs_node::NodeHandle`) pour le processus de nœud
Instance `DealEngine` en français یہ moteur :

- `DealTermsV1` pour les offres et pour valider et s'inscrire
- rapport d'utilisation de la réplication et les frais libellés en XOR s'accumulent.
- L'échantillonnage déterministe basé sur BLAKE3 et les fenêtres probabilistes de micropaiement évaluent les résultats. اور
- instantanés du grand livre et publication de la gouvernance et charges utiles de règlementValidation des tests unitaires, sélection des micropaiements, flux de règlement et couverture des paiements
opérateurs اعتماد کے ساتھ Exercice sur les API کر سکیں۔ Règlements اب `DealSettlementV1` gouvernance
les charges utiles émettent des liens vers le pipeline de publication SF-12 et vers le fil de connexion
OpenTelemetry et série `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) et tableaux de bord Torii et
Application du SLO et mise à jour du SLO Éléments de suivi automatisés par l'auditeur pour réduire les coupes
sémantique d'annulation et politique de gouvernance et coordination et coordination

Utilisation de la télémétrie pour l'ensemble de métriques `sorafs.node.micropayment_*` pour le flux de données :
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, guichets de vente de billets
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). یہ totaux flux de loterie probabiliste کو ظاہر کرتے ہیں تاکہ
les opérateurs de micropaiement gagnent et le report de crédit et les résultats du règlement sont en corrélation

## Intégration Torii

Les points de terminaison dédiés Torii exposent le rapport d'utilisation des fournisseurs de services.
Pour le câblage sur mesure et le cycle de vie des transactions, voici :- `POST /v2/sorafs/deal/usage` `DealUsageReport` télémétrie acceptée
  les résultats comptables déterministes (`UsageOutcome`) renvoient کرتا ہے۔
- `POST /v2/sorafs/deal/settle` finalisation de la fenêtre actuelle
  Il s'agit d'un flux de données `DealSettlementRecord` encodé en base64 `DealSettlementV1`.
  جو gouvernance DAG publication کے لیے تیار ہوتا ہے۔
- Torii vers `/v2/events/sse` flux vers `SorafsGatewayEvent::DealUsage` enregistrements diffusés par ici
  Il s'agit de la soumission d'utilisation et de la date d'achat (époque, GiB-heures mesurés, compteurs de billets,
  frais déterministes), enregistrements `SorafsGatewayEvent::DealSettlement` et instantané du grand livre de règlement canonique
  artefact de gouvernance sur disque par BLAKE3 digest/size/base64
  Alertes `SorafsGatewayEvent::ProofHealth` lorsque les seuils PDP/PoTR dépassent ہوں (fournisseur, fenêtre, état d'attaque/temps de recharge, montant de la pénalité)
  Fournisseur de consommateurs avec filtre et télémétrie pour les règlements et alertes de preuve de santé
  sondages pour réagir

Cadre de quota des points de terminaison SoraFS et fenêtre `torii.sorafs.quota.deal_telemetry` disponible
Les opérateurs et les opérateurs de déploiement et le réglage du taux de soumission autorisé sont également disponibles.