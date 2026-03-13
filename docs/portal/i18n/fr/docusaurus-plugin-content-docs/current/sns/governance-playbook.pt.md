---
lang: fr
direction: ltr
source: docs/portal/docs/sns/governance-playbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Fonte canonica
Cette page est en ligne `docs/source/sns/governance_playbook.md` et elle sert maintenant à
copia canonica do portail. L'archive est permanente pour les PR de traduction.
:::

# Playbook de gouvernance du service de noms Sora (SN-6)

**Statut :** Redigido 2026-03-24 - référence vivante pour la prochaine SN-1/SN-6  
**Les liens font la feuille de route :** SN-6 "Conformité et résolution des litiges", SN-7 "Résolveur et synchronisation de passerelle", politique d'endereco ADDR-1/ADDR-5  
**Pré-requis :** Esquema do registro em [`registry-schema.md`](./registry-schema.md), contrato da API do registrar em [`registrar-api.md`](./registrar-api.md), guia UX de enderecos em [`address-display-guidelines.md`](./address-display-guidelines.md), et voici la structure du contenu dans [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Ce playbook décrit les corps de gouvernance du Sora Name Service (SNS)
adotam cartas, aprovam registros, escalam disputas e provam que os estados de
résolveur et passerelle permanente en synchronisation. Ele cumpre o requisito do roadmap de
que a CLI `sns governance ...`, manifestes Norito et artefatos de auditoria
partager une référence unique à l'opérateur avant la N1 (lancement
public).

## 1. École et public

Le document est destiné à :- Les membres du Conselho de Governanca qui votent des chartes, des politiques de suffixe et
  résultats du litige.
- Membres du conseil des tuteurs qui émettent des gels d'émergence et
  réviser les revers.
- Stewards de suffiso que operam filas do registrar, aprovam leiloes e gerenciam
  division des recettes.
- Opérateurs de résolution/passerelle responsables de la propagation SoraDNS, actualisés
  GAR et garde-corps de télémétrie.
- Equipes de conformité, tesouraria et support qui doivent démontrer que toda
  cacao de gouvernance deixou artefatos Norito auditaveis.

Ele cobre ases bêta fechada (N0), lancement public (N1) et expansion (N2)
listadas em `roadmap.md`, vinculando cada fluxo de trabalho as evidencias
nécessités, tableaux de bord et chemins d'escalade.

## 2. Papeis et carte de contact| Papier | Responsabilités principales | Artefatos et télémétrie principaux | Escalaçao |
|-------|------------------------------|-----------------------------------|---------------|
| Conseil de Gouvernance | Rédiger et ratifier les chartes, politiques de suffixe, vérifications des différends et rotations de l'intendant. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, votes du conseil armazenados via `sns governance charter submit`. | Président du Conseil + rastreador de l'agenda de gouvernance. |
| Conseil des tuteurs | Émite les congélations molles/dures, canons d'émergence et révisions de 72 h. | Billets gardien émis par `sns governance freeze`, manifestes de dérogation enregistrés dans `artifacts/sns/guardian/*`. | Gardien Rotacao de garde (<=15 min ACK). |
| Intendants de suffixe | Operam filas do registrar, leiloes, niveis de preco e comunicacao com clientses ; reconhecem conformidades. | Politiques de steward em `SuffixPolicyV1`, folhas de referencia de preco, remerciements de steward armazenados junto a memos régulateurios. | Diriger le programme steward + PagerDuty par suffixe. |
| Opérations de registraire et cobranca | Les points de terminaison d'exploitation `/v2/sns/*`, rapprochent les paiements, émettent des télémétries et des instantanés permanents de CLI. | API du registraire ([`registrar-api.md`](./registrar-api.md)), mesures `sns_registrar_status_total`, preuves de paiement archivées dans `artifacts/sns/payments/*`. | Duty manager do registrar e liaison da tesouraria. || Opérateurs de résolution et de passerelle | Mantem SoraDNS, GAR et l'état de la passerelle associé aux événements du registraire ; transmettre des mesures de transparence. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | Résolveur SRE sur appel + ponte ops do gateway. |
| Trésorerie et finances | Application de division de réception 70/30, exclusions de référence, registres fiscaux/tesouraria et atestacoes SLA. | Manifestes de cumul de réception, exportations Stripe/tesouraria, annexes KPI trimestrais em `docs/source/sns/regulatory/`. | Contrôleur financier + officiel de conformité. |
| Liaison de conformité et de réglementation | Accompanha obrigacoes globais (EU DSA, etc.), actualise les engagements KPI et enregistre les divulgations. | Mémos régulateurs sur `docs/source/sns/regulatory/`, ponts de référence, entrées `ops/drill-log.md` pour ensaios tabletop. | Guide du programme de conformité. |
| Support / SRE d'astreinte | Lida com incidents (colisos, drift de cobranca, quedas de solver), coordonne les messages des clients et donne les runbooks. | Modèles d'incident, `ops/drill-log.md`, preuves de laboratoire, transcriptions Slack/war-room archivées dans `incident/`. | Rotacao de garde SNS + gestao SRE. |

## 3. Artefatos canonicos et fontes de dados| Artefato | Localisation | Proposé |
|--------------|------------|--------------|
| Carta + addenda KPI | `docs/source/sns/governance_addenda/` | Cartes assinadas com controle de versao, clauses KPI et décisions de gouvernance référencées par les votes de CLI. |
| Esquema do registro | [`registry-schema.md`](./registry-schema.md) | Structures canoniques Norito (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Contrat du registraire | [`registrar-api.md`](./registrar-api.md) | Charges utiles REST/gRPC, métriques `sns_registrar_status_total` et attentes de hook de gouvernance. |
| Guide UX des enderecos | [`address-display-guidelines.md`](./address-display-guidelines.md) | Rendu canonique I105 (préféré) et compressé (seconde meilleure option) reflétant les portefeuilles/explorateurs. |
| Documents SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Dérivation déterministe des hôtes, flux de transparence et contrôle des alertes. |
| Mémos réglementaires | `docs/source/sns/regulatory/` | Notas de entrada por jurisdicao (ex. EU DSA), remerciements de steward, anexos de template. |
| Journal de forage | `ops/drill-log.md` | Registro de ensaios de caos e IR requeridos antes de sayas de phase. |
| Équipement d'artéfacts | `artifacts/sns/` | Tests de paiement, gardien de tickets, résolveur de différences, exportations KPI et CLI assinada produit par `sns governance ...`. |Tous les aspects de la gouvernance devraient se référer au moins à un art du tableau
acima pour que les auditeurs reconstruisent le rastro de décision dans 24 heures.

## 4. Playbooks de cycle de vie

### 4.1 Mocoes de carta e steward

| Étapa | Responsavel | CLI / Preuves | Notes |
|-------|-------------|-------|-------|
| Redigir addendum et deltas KPI | Relator do conselho + lider steward | Modèle Markdown activé dans `docs/source/sns/governance_addenda/YY/` | Incluez les ID des KPI de convention, les crochets de télémétrie et les conditions d'activité. |
| Envoyer la proposition | Président du Conseil | `sns governance charter submit --input SN-CH-YYYY-NN.md` (produit `CharterMotionV1`) | Une CLI émet le manifeste Norito salvo em `artifacts/sns/governance/<id>/charter_motion.json`. |
| Voto et remerciement tuteur | Conselho + tuteurs | `sns governance ballot cast --proposal <id>` et `sns governance guardian-ack --proposal <id>` | Anexar atas hasheadas e provas de quorum. |
| Intendant d'Aceitacao | Programme de steward | `sns governance steward-ack --proposal <id> --signature <file>` | Obrigatoire avant de changer de politique de suffixe ; enveloppe du registraire em `artifacts/sns/governance/<id>/steward_ack.json`. |
| Ativação | Ops faire registraire | Actualiser `SuffixPolicyV1`, actualiser les caches du registraire, publier la note sur `status.md`. | Horodatage d'activation enregistré dans `sns_governance_activation_total`. |
| Journal des auditoires | Conformité | Ajouter l'entrée au `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` et aucune bûche de forage n'est ouverte sur la table. | Inclure des références aux tableaux de bord de télémétrie et aux différences politiques. |

### 4.2 Agréments d'enregistrement, leilao et preco1. **Preflight :** Le registraire consulte `SuffixPolicyV1` pour confirmer le niveau de
   preco, termos disponiveis e janelas de graca/redencao. Mantenha folhas de
   preco synchronisés avec une table de niveaux 3/4/5/6-9/10+ (niveau base +
   coefficients de suffixe) documenté sans feuille de route.
2. ** Offre scellée de Leiloes : ** Prime pour les pools, exécutez le cycle d'engagement de 72 h /
   Révélation 24 h via `sns governance auction commit` / `... reveal`. Publique un
   liste des commits (hachages simples) dans `artifacts/sns/auctions/<name>/commit.json`
   pour que les auditeurs vérifient de manière aléatoire.
3. **Vérification du paiement :** Les registraires valident `PaymentProofV1` contre un
   Division de tesouraria (70 % tesouraria / 30 % steward avec exclusion de référence <=10 %).
   Armazene ou JSON Norito dans `artifacts/sns/payments/<tx>.json` et vincule-o na
   réponse du registraire (`RevenueAccrualEventV1`).
4. **Hook de gouvernance :** Anexe `GovernanceHookV1` pour noms premium/guarded com
   référence et identifiants de proposition du conseil et des assinaturas de steward. Crochets
   ausentes resultam em `sns_err_governance_missing`.
5. **Activation + synchronisation du résolveur :** Assurez-vous que Torii émette l'événement d'enregistrement,
   Action ou suivi de la transparence du résolveur pour confirmer que le nouvel état est
   GAR/zone se propage (voir 4.5).
6. **Divulgation au client :** Actualisez le grand livre affiché auprès du client (portefeuille/explorateur)
   via les appareils os partagés em [`address-display-guidelines.md`](./address-display-guidelines.md),garantindo que renderizacoes I105 et comprimidas correspondam a orientacoes de copy/QR.

### 4.3 Rénovations, cobranca et réconciliation de la tesouraria- **Flux de rénovation :** Les registraires appliquent une janvier de grâce de 30 jours + janvier
  de redencao de 60 jours spécifié dans `SuffixPolicyV1`. Apos 60 jours, un
  séquence de reprise hollandaise (7 jours, taxons 10x décimaux 15%/jour) et
  actionné automatiquement via `sns governance reopen`.
- **Divisao de receita:** Cada renovacao ou transferencia cria um
  `RevenueAccrualEventV1`. Les exportations de tesouraria (CSV/Parquet) doivent être réconciliées
  ces événements quotidiens; l'annexe est publiée dans `artifacts/sns/treasury/<date>.json`.
- **Carve-outs de reference:** Pourcentages de référence opcionais sao rastreados por
  ajouter le suffixe `referral_share` à la politique de steward. Les bureaux d'enregistrement émettent un
  Diviser les manifestes de référence finals et armazenam à partir de la preuve du paiement.
- **Cadencia de relatorios:** Financas publica anexos KPI mensais (registros,
  rénovations, ARPU, utilisation de litiges/caution) dans `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
  Les tableaux de bord développent des tableaux exportés pour les numéros de
  Grafana batam com comme preuves du grand livre.
- **Revisao KPI mensal:** O checkpoint da primeira terca-feira junta o lider de
  Financas, intendant de l'usine et PM du programme. Abra o [Tableau de bord SNS KPI] (./kpi-dashboard.md)
  (intégrer le portail de `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`),
  exporter sous forme de tableaux de débit + réception du registraire, registre deltas no
  anexo et anexe os artefatos ao memo. Agir sur un incident pour une révisionquebras de SLA (janelas de freeze >72 h, picos de erro do registrar, drift de ARPU).

### 4.4 Gelations, disputes et apelles

| Phase | Responsavel | Acao et preuves | ANS |
|------|-------------|--------|-----|
| Commande de gel doux | Intendant / support | Ouvrez le ticket `SNS-DF-<id>` avec les preuves de paiement, la référence au lien de litige et le(s) sélection(s) confirmé(s). | <=4 heures par entrée. |
| Gardien des billets | Conselho tuteur | `sns governance freeze --selector <I105> --reason <text> --until <ts>` produit `GuardianFreezeTicketV1`. Armazene ou JSON font le ticket em `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h d'exécution. |
| Ratification du conseil | Conseil de gouvernance | Approuver ou refuser les congélations, documenter la décision avec le lien vers le gardien du ticket et le résumé du lien de litige. | Proxima sessao do conselho ou voto assincrono. |
| Tableau d'arbitrage | Conformidade + steward | Envoyer un message de 7 jurés (conforme à la feuille de route) avec cedulas hasheadas via `sns governance dispute ballot`. Anexar recibos de voto anonimizados ao pacote de incidente. | Veredito <=7 dias apos deposito do bond. |
| Apelacao | Tuteur + conseiller | Apelacos dobram o bond e repetem o processo de juradas ; Manifeste du registraire Norito `DisputeAppealV1` et ticket de référence principal. | <=10 jours. |
| Décongeler et réparer | Registraire + opérations de résolveur | Exécuter `sns governance unfreeze --selector <I105> --ticket <id>`, actualiser le statut du registraire et propager les différences GAR/résolveur. | Immédiatement après le veredito. |Canones d'émergence (congelamentos acionados por tuteur <=72 h) seguem o mesmo
flux, mais exige une révision rétroactive du conseil et une note de transparence sur
`docs/source/sns/regulatory/`.

### 4.5 Propagande du résolveur et de la passerelle

1. **Hook d'événement :** Chaque événement enregistré est émis pour le flux d'événements de
   résolveur (`tools/soradns-resolver` SSE). Les opérations du résolveur s'inscrivent
   registram diffs via o tailer de transparencia
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Modèle de mise à jour GAR :** Les passerelles développent des modèles de mise à jour GAR
   références à `canonical_gateway_suffix()` et réinsertion dans la liste
   `host_pattern`. Armazene diffère en `artifacts/sns/gar/<date>.patch`.
3. **Publication de zonefile :** Utilisez le squelette du fichier de zone décrit dans `roadmap.md`.
   (nom, ttl, cid, preuve) et envie pour Torii/SoraFS. Archiver le JSON Norito dans
   `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Chèque de transparence :** Exécuter `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   para garantir que os alertas sigam verdes. Anexe a saya de texto do
   Prometheus au rapport semanal de transparence.
5. **Auditoria de gateway :** Registre des en-têtes `Sora-*` (politique de
   cache, CSP, digest GAR) et annexe comme journal de gouvernance pour les opérateurs
   possam prouve que la passerelle sert le nouveau nom avec les garde-corps espérés.

## 5. Télémétrie et relations| Sinal | Fonte | Description / Açao |
|-------|-------|--------|
| `sns_registrar_status_total{result,suffix}` | Les gestionnaires font le registraire Torii | Contador de successo/erro para registros, renovacoes, gelamentos, transferencias; alerte lorsque `result="error"` augmente par suffixe. |
| `torii_request_duration_seconds{route="/v2/sns/*"}` | Métriques Torii | SLO de latence pour les gestionnaires d'API ; tableaux de bord alimenta basés sur `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` et `soradns_bundle_cid_drift_total` | Tailleur de transparence du résolveur | Détecta s'avère obsolète ou dérive de GAR ; garde-corps définis dans `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | CLI de gouvernance | Contador incrémentado quando um charter/addendum ativa; utilisé pour concilier les décisions du conseil avec les ajouts publiés. |
| Jauge `guardian_freeze_active` | Gardien CLI | Accompagne les serviettes de gel doux/dur selon le choix ; page SRE se o valor ficar `1` alem do SLA déclaré. |
| Tableaux de bord des anexos KPI | Finances / Docs | Rollups mensais publicados junto a memos Regulatorios ; Le portail est intégré via [SNS KPI Dashboard] (./kpi-dashboard.md) pour que les stewards et les régulateurs accèdent à mon visa Grafana. |

## 6. Conditions requises pour les preuves et les auditoires| Açao | Preuve à l'archive | Armament |
|------|------------|--------------------|
| Mudanca de carta / politique | Manifeste Norito assassiné, transcription CLI, différence de KPI, accusé de réception de l'intendant. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Registro / rénovation | Charge utile `RegisterNameRequestV1`, `RevenueAccrualEventV1`, preuve de paiement. | `artifacts/sns/payments/<tx>.json`, journaux de l'API du registraire. |
| Leïlao | Les manifestes s'engagent/révèlent, sentiment de hasard, plan de calcul du vendeur. | `artifacts/sns/auctions/<name>/`. |
| Congeler / décongeler | Gardien du ticket, hachage du vote du conseil, URL du journal des incidents, modèle de communication avec le client. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Propagande du résolveur | Diff zonefile/GAR, trecho JSONL do tailer, instantané Prometheus. | `artifacts/sns/resolver/<date>/` + rapports de transparence. |
| Régulateur d'admission | Mémo d'admission, suivi des délais, accusé de réception du steward, CV des mudancas KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Checklist de porte de phase| Phase | Critères de dite | Bundle de preuves |
|------|----------|--------------------|
| N0 - Beta fechada | Esquema de registro SN-1/SN-2, CLI de registrar manual, Drill Guardian complet. | Motion de carte + ACK steward, journaux d'exécution à sec du registraire, rapport de transparence du résolveur, entré dans `ops/drill-log.md`. |
| N1 - Lancement public | Leiloes + niveaux de preco fixo ativos para `.sora`/`.nexus`, libre-service du registraire, synchronisation automatique du résolveur, tableaux de bord de Cobranca. | Diff de folha de preco, resultados CI do registrar, anexo de pagamento/KPI, saya do tailer de transparencia, notas de ensaio de incidente. |
| N2 - Expansão | `.dao`, API de revendeur, portail de litige, cartes de pointage de steward, tableaux de bord d'analyse. | Captures du portail, mesures SLA de litige, exportations de cartes de pointage de steward, carte de gouvernance actualisée avec la politique du revendeur. |

Comme le dit la phase exige des exercices sur table registrados (fluxo feliz de registro,
gel, panne du résolveur) avec artefatos anexados em `ops/drill-log.md`.

## 8. Réponse aux incidents et escalades| Gatilho | Sévérité | Dono immédiatement | Acoes obrigatorios |
|---------|-----------|---------------|-------------------|
| Drift de solver/GAR ou provas obsoletas | 1 septembre | Résolveur SRE + tuteur conselho | Page ou sur appel du résolveur, capturez ce que dit le tailer, décidez de congeler les noms afetados, état public à chaque 30 min. |
| Queda de registrar, falha de cobranca, ou erreurs API généralisées | 1 septembre | Le responsable de service fait le registraire | Pare novos leiloes, mude para CLI manual, notifique stewards/tesouraria, anexe logs do Torii ao doc de incidente. |
| Litige de nom unique, non-concordance de paiement ou escalade de client | 2 septembre | Steward + dirigeant de support | Faites preuve de paiement, déterminez si le gel est doux et nécessaire, répondez à la sollicitante à l'intérieur du SLA, enregistrez le résultat sans suivre le litige. |
| Dossier de salle de conformité | 2 septembre | Liaison de conformité | Redigir plano de remediacao, archiva memo em `docs/source/sns/regulatory/`, agenda session of conselho de accompagnement. |
| Drill ou ensaio | 3 septembre | PM faire le programme | Exécutez le scénario roteirizado de `ops/drill-log.md`, archivez les artefatos, marquez les lacunes comme les tarefas de la feuille de route. |

Tous les incidents doivent être criés `incident/YYYY-MM-DD-sns-<slug>.md` avec des tableaux
de propriété, journaux de commandes et références comme preuves produites depuis longtemps
ce livre de jeu.

## 9. Références- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
-`ops/drill-log.md`
- `roadmap.md` (seco SNS, DG, ADDR)

Mantenha este playbook actualisé toujours le texte des cartes, en tant que superficie
de CLI ou les contrats de télémétrie mudarem ; comme entrées dans la feuille de route que
référence `docs/source/sns/governance_playbook.md` devem semper correspondant a
Dernière révision.