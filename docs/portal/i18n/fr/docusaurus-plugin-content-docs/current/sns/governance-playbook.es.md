---
lang: fr
direction: ltr
source: docs/portal/docs/sns/governance-playbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Fuente canonica
Cette page reflète `docs/source/sns/governance_playbook.md` et maintenant, monsieur comme moi
la copie canonique du portail. Le fichier est conservé pour les PR de traduction.
:::

# Playbook de gouvernance du Service de Nombres Sora (SN-6)

**État :** Borrador 2026-03-24 - référence vivante pour la préparation SN-1/SN-6  
**Enlaces del roadmap :** SN-6 "Compliance & Dispute Résolution", SN-7 "Resolver & Gateway Sync", politique de directives ADDR-1/ADDR-5  
**Prérequis :** Esquema de registro en [`registry-schema.md`](./registry-schema.md), contrat d'API de registrador en [`registrar-api.md`](./registrar-api.md), guide UX de directions en [`address-display-guidelines.md`](./address-display-guidelines.md), et réglementation de la structure des comptes en [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Ce playbook décrit les corps de gouvernement du service de noms Sora (SNS)
adoptan cartas, aprueban registros, escalan disputas, y prueban que los estados de
résolveur et passerelle permanente synchronisée. Remplir les exigences de la feuille de route de
que la CLI `sns governance ...`, les manifestes Norito et les artefacts de
les auditoires partagent une seule référence opérationnelle avant la N1 (lancement
public).

## 1. Alcance et audience

Le document est dirigé vers :- Miembros del Consejo de Gobernanza que votan cartas, politicas de soufijos y
  résultats des litiges.
- Miembros de la junta de Guardianes qui émettent des gelamientos d'urgence et
  révisions des réversions.
- Stewards de soufijos qui operan colas de registrador, aprueban subastas y
  gestionan repartos de ingresos.
- Opérateurs de résolveur/passerelle responsables de la propagation SoraDNS,
  actualizaciones GAR et garde-corps de télémétrie.
- Equipos de cumplimiento, tesoreria y soporte que deben demostrar que toda
  action de gobernanza dejo artefactos Norito auditables.

Cubre les phases de bêta-cerrada (N0), de lancement public (N1) et d'expansion (N2)
listadas en `roadmap.md`, vinculando cada flujo de trabajo con la evidencia,
 tableaux de bord et itinéraires d'escalade requis.

## 2. Rôles et carte des contacts| Rôle | Responsabilités principales | Artefacts et télémétrie principales | Escalade |
|------|-------------------------------|-----------------------------------------|------------|
| Conseil de Gouvernement | Rédaction et ratification de chartes, politiques de soufijos, verdicts de litige et rotations de stewards. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, votes du consejo almacenados via `sns governance charter submit`. | Présidence du Conseil + suivi de l'agenda du gouvernement. |
| Conseil des Gardiens | Émite des congélations douces/dures, canons d'émergence et révisions de 72 h. | Billets de gardien émis par `sns governance freeze`, manifestes de dérogation en `artifacts/sns/guardian/*`. | Guardia de garde (<= 15 min ACK). |
| Intendants de soufijo | Colas Operan del registrador, subastas, niveaux de prix et communication avec les clients ; reconocen cumlimientos. | Politiques de steward en `SuffixPolicyV1`, heures de référence de prix, accusent de steward conjointement avec des mémorandos réglementaires. | Diriger le programme des stewards + PagerDuty par sufijo. |
| Opérations d'enregistrement et de facturation | Les points de terminaison opérationnels `/v1/sns/*`, les pages de réconciliation, émettent des données de télémétrie et conservent des instantanés de CLI. | API du registrateur ([`registrar-api.md`](./registrar-api.md)), mesures `sns_registrar_status_total`, tests de page et `artifacts/sns/payments/*`. | Duty manager del registrador y enlace de tesoreria. || Opérateurs de résolution et passerelle | Gardez SoraDNS, GAR et l'état de la passerelle aligné avec les événements de l'enregistreur ; transmettre des mesures de transparence. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | SRE del solver on-call + puente ops de gateway. |
| Trésorerie et Finances | Application du rapport 70/30, exclusions de référence, déclarations fiscales/tesoreria et attestations SLA. | Manifiestos de acumulacion de ingresos, exportaciones Stripe/tesoreria, annexes KPI trimestrales en `docs/source/sns/regulatory/`. | Contrôleur des finances + oficial de cumplimiento. |
| Enlace de Cummplimiento y Regulación | Rastrea obligaciones globales (EU DSA, etc.), actualise les engagements KPI et présente les divulgations. | Mémos régulateurs en `docs/source/sns/regulatory/`, ponts de référence, entrées `ops/drill-log.md` pour ensayos tabletop. | Guide du programme de compilation. |
| Assistance / SRE d'astreinte | Gestion des incidents (colisions, dérives de facturation, lignes de résolution), coordination des messages avec les clients et le duo des runbooks. | Plantilles d'incident, `ops/drill-log.md`, preuves de laboratoire, transcriptions Slack/war-room et `incident/`. | Rotation d'astreinte SNS + gestion SRE. |

## 3. Artefacts canoniques et sources de données| Artefact | Emplacement | Proposé |
|----------|----------|---------|
| Carta + anexos KPI | `docs/source/sns/governance_addenda/` | Cartes fermes avec contrôle de version, engagements KPI et décisions de gouvernance référencées par les votes de CLI. |
| Esquema de registro | [`registry-schema.md`](./registry-schema.md) | Structures canoniques Norito (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Contrat du registrateur | [`registrar-api.md`](./registrar-api.md) | Charges utiles REST/gRPC, métriques `sns_registrar_status_total` et attentes du hook de gouvernance. |
| Guide d'orientation UX | [`address-display-guidelines.md`](./address-display-guidelines.md) | Rendus canoniques IH58 (préférés) et compressés (seconde meilleure option) réfléchis par les portefeuilles/explorateurs. |
| Documents SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Dérivation déterministe des hôtes, flux de transparence et réglementation des alertes. |
| Mémos réglementaires | `docs/source/sns/regulatory/` | Notas de ingreso jurisdiccional (p. ej., EU DSA), acuses de steward, anexos plantilla. |
| Journal de forage | `ops/drill-log.md` | Registre des analyses de causes et des IR requis avant le début de la phase. |
| Liste des objets | `artifacts/sns/` | Tests de paiement, tickets de gardien, différences de résolution, exportations KPI et sortie confirmée de `sns governance ...`. |Toutes les actions de gouvernement doivent se référer à moins d'un artefact dans la
 table antérieure pour que les auditeurs reconstruisent le rastro de décision en 24
 heures.

## 4. Playbooks de cycle de vie

### 4.1 Mociones de carta y steward

| Paso | Dueno | CLI / Preuves | Notes |
|------|-------|----------------|-------|
| Rédiger l'addendum et les KPI deltas | Rapporteur du conseiller + directeur de l'intendant | Plantilla Markdown en `docs/source/sns/governance_addenda/YY/` | Incluez les ID des KPI de convention, les crochets de télémétrie et les conditions d'activation. |
| Présenter la proposition | Présidence du Conseil | `sns governance charter submit --input SN-CH-YYYY-NN.md` (produire `CharterMotionV1`) | La CLI émet le manifeste Norito et `artifacts/sns/governance/<id>/charter_motion.json`. |
| Vote et reconnaissance des tuteurs | Consejo + tuteurs | `sns governance ballot cast --proposal <id>` et `sns governance guardian-ack --proposal <id>` | Minutes supplémentaires avec hachage et vérification du quorum. |
| Acceptation du steward | Programme des stewards | `sns governance steward-ack --proposal <id> --signature <file>` | Exigé avant de changer la politique des soufijos ; garder sur le `artifacts/sns/governance/<id>/steward_ack.json`. |
| Activation | Opérations du registrateur | Actualiser `SuffixPolicyV1`, rafraîchir les caches du registraire, publier la note sur `status.md`. | Horodatage d'activation sur `sns_governance_activation_total`. |
| Journal d'audit | Cumul | Agréger l'entrée au `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` et au journal de forage sur la table. | Inclure des références aux tableaux de bord de télémétrie et aux différences politiques. |

### 4.2 Registres, sous-titres et autorisations de prix1. **Preflight :** L'enregistreur consulte `SuffixPolicyV1` pour confirmer le niveau de
   prix, terminos disponibles et ventanas de gracia/redencion. Garder les heures de
   prix synchronisés avec la table de niveaux 3/4/5/6-9/10+ (niveau de base +
   coefficients de soufijo) documentés dans la feuille de route.
2. **Subastas scellé-offre :** Para pools premium, ejecutar el ciclo 72 h commit /
   Révélation 24 h via `sns governance auction commit` / `... reveal`. Publier la
   liste des commits (hachages solo) en `artifacts/sns/auctions/<name>/commit.json`
   pour que les auditeurs vérifient l'aléatoire.
3. **Vérification de paiement :** Les registrateurs valident `PaymentProofV1` contre le
   reparto de tesoreria (70 % tesoreria / 30 % steward con carve-out de referido <=10 %).
   Garder le JSON Norito et `artifacts/sns/payments/<tx>.json` et le connecter au
   réponse du registrateur (`RevenueAccrualEventV1`).
4. **Hook de gobernanza :** Adjuntar `GovernanceHookV1` para nombres premium/guarded
   avec références et identifiants de propuesta del consejo et firmas de steward. Crochets
   résultats faltants en `sns_err_governance_missing`.
5. **Activation + synchronisation du résolveur :** Une fois que Torii émet l'événement de
   s'inscrire, supprimer le détail de la transparence du résolveur pour confirmer
   que le nouveau statut GAR/zone est propagé (ver 4.5).
6. **Divulgation au client :** Actualiser le grand livre orienté client
   (portefeuille/explorateur) via los luminaires compartimentés fr[`address-display-guidelines.md`](./address-display-guidelines.md), assurer
   que les rendus IH58 et les emballages coïncident avec le guide de copie/QR.

### 4.3 Rénovations, facturation et réconciliation des valeurs- **Flux de rénovation :** Les registrateurs appliquent les fenêtres de grâce de
  30 jours + redencion de 60 jours spécifiés en `SuffixPolicyV1`. Après 60 ans
  jours, la sécurité de la reprise hollandaise (7 jours, tarif 10x dégressif 15%/jour)
  s'active automatiquement via `sns governance reopen`.
- **Reparto de ingresos:** Chaque rénovation ou transfert crée un
  `RevenueAccrualEventV1`. Les exportations de matériaux (CSV/Parquet) doivent
  réconcilier avec ces événements journaliers ; adjuntar pruebas fr
  `artifacts/sns/treasury/<date>.json`.
- **Carve-outs de références :** Les pourcentages de référence optionnelles sont rastrés par
  il a agrégé `referral_share` à la politique de l'intendant. Les registrateurs
  émettent le split final et gardent des manifestes de référence conjointement à la vérification du paiement.
- **Cadencia de reportes:** Finanzas publica anexos KPI mensuales (registros,
  rénovations, ARPU, utilisation de litiges/bond) fr
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Les tableaux de bord deben tomar
  de las mismas tablas exportadas para que los numeros de Grafana coïncident avec
  la preuve du grand livre.
- **Révision KPI mensual :** El checkpoint del primer martes empareja al lider de
  finanzas, steward de turno et PM del programa. Ouvrir le [Tableau de bord SNS KPI](./kpi-dashboard.md)
  (intégrer le portail de `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`),
  exporter les tableaux de débit et de revenus du registrateur, registraire deltas enel anexo y adjuntar los artefactos al memo. Supprimer un incident lors de la révision
  encuentra brechas SLA (ventanas de freeze >72 h, picos de error del registrador,
  dérive de l'ARPU).

### 4.4 Congelamientos, disputes y apelaciones| Phase | Dueno | Action et preuve | ANS |
|-------|-------|---------|-----|
| Demande de freeze soft | Steward / soporte | Présenter le ticket `SNS-DF-<id>` avec vérifications du paiement, référence de cautionnement de litige et sélecteur(s) affecté(s). | <=4 h après l'entrée. |
| Billet de gardien | Conseil des gardiens | `sns governance freeze --selector <IH58> --reason <text> --until <ts>` produit `GuardianFreezeTicketV1`. Garder le JSON du ticket en `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h d'éjection. |
| Ratification du Conseil | Conseil d'administration | Après avoir demandé des congélations, documenter la décision enlazada sur le ticket de tuteur et digérer le lien de litige. | Proxima session del consejo o voto asincrono. |
| Panel d'arbitrage | Cummplimiento + steward | Convoquer un panel de 7 jurés (selon la feuille de route) avec des boletas hasheadas via `sns governance dispute ballot`. Ajouter des reçus de vote anonymisés au paquet d'incident. | Veredicto <=7 jours après le dépôt de la caution. |
| Apélacion | Gardiens + conseiller | Las apelaciones duplican el bond y repiten el proceso de jurados ; Manifestation du registraire Norito `DisputeAppealV1` et ticket de référence principal. | <=10 jours. |
| Décongeler et réparer | Enregistreur + opérations de résolution | Exécutez `sns governance unfreeze --selector <IH58> --ticket <id>`, actualisez l'état de l'enregistreur et propagez les différences GAR/résolveur. | Immédiatement après le verdict. |Les canons d'émergence (congelamientos activés por tuteurs <=72 h) suivant
el mismo flujo mais requieren revision retroactiva del consejo et une note de
transparence en `docs/source/sns/regulatory/`.

### 4.5 Propagation du résolveur et de la passerelle

1. **Hook d'événement :** Chaque événement enregistré émet le flux d'événements de l'événement.
   résolveur (`tools/soradns-resolver` SSE). Ops de solver s'abonner
   enregistrer les différences via el tailer de transparencia
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Actualisation de la plante GAR :** Les passerelles doivent actualiser les plantes GAR
   référencées par `canonical_gateway_suffix()` et reconfirmer la liste
   `host_pattern`. Garder les différences en `artifacts/sns/gar/<date>.patch`.
3. **Publication de zonefile :** Utilisez la description du squelette de zonefile en
   `roadmap.md` (nom, ttl, cid, preuve) et utiliser Torii/SoraFS. Archivar el
   JSON Norito et `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Chèque de transparence :** Éjecter `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   pour garantir que les alertes vertes soient permanentes. Ajouter la sortie de texte
   de Prometheus au rapport de transparence semanal.
5. **Auditoria de gateway :** Le registraire indique les en-têtes `Sora-*` (politique de
   cache, CSP, digest de GAR) et compléments au journal de gouvernance pour que
   Les opérateurs peuvent vérifier que la passerelle sirvio le nouveau nom avec les
   garde-corps previstos.

## 5. Télémétrie et rapports| Sénat | Source | Description / Action |
|--------|--------|----------------------|
| `sns_registrar_status_total{result,suffix}` | Gestionnaires de registraire Torii | Contador de exito/error para registros, renovaciones, congelamientos, transferencias; alerta lorsque `result="error"` est soumis à un soufijo. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Mesures de Torii | SLO de latence pour l'API des gestionnaires ; tableaux de bord alimenta basados ​​en `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` et `soradns_bundle_cid_drift_total` | Tailleur de transparence du résolveur | Détecter les essais obsolètes ou la dérive du GAR ; garde-corps définis en `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | Gouvernance CLI | Contacter incrémenté lorsqu'une charte/un addendum est activé ; se usa para concilier les décisions du conseil avec les addenda publiés. |
| Jauge `guardian_freeze_active` | CLI gardien | Rastrea ventanas de freeze soft/hard par sélecteur ; Paginar a SRE si el valor queda en `1` plus alla del SLA déclaré. |
| Tableaux de bord annexes KPI | Finances / Docs | Rollups mensuels publiés avec des mémos réglementaires ; le portail est intégré via [SNS KPI Dashboard](./kpi-dashboard.md) pour que les stewards et les régulateurs accèdent à la même vue de Grafana. |

## 6. Exigences de preuve et d'auditoire| Accion | Preuve à l'archivage | Almacén |
|--------|------------|---------|
| Changement de charte / politique | Manifiesto Norito firmado, transcription CLI, diff KPI, accuse de steward. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Registre / rénovation | Charge utile `RegisterNameRequestV1`, `RevenueAccrualEventV1`, test de paiement. | `artifacts/sns/payments/<tx>.json`, journaux de l'API du registrateur. |
| Subasta | Manifiestos commit/reveal, semilla de aleatoriedad, tableur de calcul du organisateur. | `artifacts/sns/auctions/<name>/`. |
| Congeler / décongeler | Ticket de tuteur, hachage du vote du conseiller, URL du journal d'incident, plante de communication avec le client. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Propagation du résolveur | Zonefile/GAR diff, extrait JSONL du tailer, instantané Prometheus. | `artifacts/sns/resolver/<date>/` + rapports de transparence. |
| Ingreso régulateur | Mémo d'entrée, suivi des délais, accusé de steward, reprise des changements KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Checklist des portes de phase| Phase | Critères de sortie | Bundle de preuves |
|-------|----------|--------------------|
| N0 - Bêta Cerrada | Esquema de registro SN-1/SN-2, CLI de registrador manuel, foret de gardien terminé. | Motion de carte + ACK de steward, journaux d'exécution à sec de l'enregistreur, rapport de transparence du résolveur, entrée en `ops/drill-log.md`. |
| N1 - Lancement public | Subastas + niveaux de prix fixes actifs pour `.sora`/`.nexus`, libre-service d'enregistrement, synchronisation automatique du résolveur, tableaux de bord de facturation. | Diff de hoja de precios, resultados CI del registrador, anexo de pagos/KPI, salida de tailer de transparencia, notas de ensayo de incidente. |
| N2 - Extension | `.dao`, API de revendeur, portail de litiges, cartes de pointage de steward, tableaux de bord d'analyse. | Captures du portail, mesures SLA des litiges, exportations de cartes de pointage de steward, charte d'administration actualisée avec la politique du revendeur. |

Las salidas de fase requieren forets tabletop registrados (registro happy path,
gel, panne du résolveur) avec des artefacts adjoints en `ops/drill-log.md`.

## 8. Réponse aux incidents et escalades| Déclencheur | Sévérité | Dueno immédiat | Accions obligatoires |
|---------|----------|----------------|-----------------------|
| Dérive du résolveur/GAR ou des essais obsolètes | 1 septembre | Résolveur SRE + junta de tuteurs | Paginar on call del solver, capturar salida del tailer, decidir si gelar nombres afectados, publicar estado cada 30 min. |
| Caida de registrador, fallo de facturación o errores API generalizados | 1 septembre | Responsable de service du registrateur | Détenir de nouvelles subastas, modifier le manuel CLI, notifier les stewards/tesoreria, ajouter les journaux de Torii au document d'incident. |
| Litige de nom unique, non-concordance de paiement ou escalade de client | 2 septembre | Steward + guide de support | Recopier les tests de paiement, décider si vous avez failli geler doux, répondre à la sollicitante à l'intérieur du SLA, registraire résultant en tracker de litige. |
| Hallazgo de auditorium de cumplimiento | 2 septembre | Enlace de cumplimiento | Rédaction d'un plan de remédiation, mémo d'archivage en `docs/source/sns/regulatory/`, session de planification du conseil de suivi. |
| Drill ou ensayo | 3 septembre | PM du programme | Exécuter un scénario guidé à partir du `ops/drill-log.md`, des objets d'archives, marquer les lacunes comme les zones de la feuille de route. |

Tous les incidents doivent être créés `incident/YYYY-MM-DD-sns-<slug>.md` avec des tableaux
de propriété, journaux de commandes et références à la preuve produite en este
livre de jeu.

## 9. Références- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
-`ops/drill-log.md`
- `roadmap.md` (rubriques SNS, DG, ADDR)

Mantener ce playbook actualisé lorsque vous changez le texte des cartes, las
superficies de CLI ou des contrats de télémétrie ; les entrées de la feuille de route que
le référent `docs/source/sns/governance_playbook.md` doit coïncider siempre avec
la révision plus récente.