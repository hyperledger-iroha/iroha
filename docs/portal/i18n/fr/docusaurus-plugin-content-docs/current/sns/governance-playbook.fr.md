---
lang: fr
direction: ltr
source: docs/portal/docs/sns/governance-playbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Source canonique
Cette page reflète `docs/source/sns/governance_playbook.md` et sert maintenant de
copie canonique du portail. Le fichier source persiste pour les PR de traduction.
:::

# Playbook de gouvernance du Sora Name Service (SN-6)

**Statut :** Redige 2026-03-24 - référence vivante pour la préparation SN-1/SN-6  
**Liens du roadmap:** SN-6 "Compliance & Dispute Résolution", SN-7 "Resolver & Gateway Sync", politique d'adresses ADDR-1/ADDR-5  
**Prérequis :** Schema du registre dans [`registry-schema.md`](./registry-schema.md), contrat d'API du registrar dans [`registrar-api.md`](./registrar-api.md), guide UX d'adresses dans [`address-display-guidelines.md`](./address-display-guidelines.md), et règles de structure de comptes dans [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Ce playbook décrit comment les organes de gouvernance du Sora Name Service (SNS)
adoptent des chartes, approuvent des enregistrements, escaladent les litiges, et
prouvent que les états du résolveur et de la passerelle restent synchronisés. Il est satisfait
l'exigence du roadmap selon laquelle la CLI `sns governance ...`, les manifestes
Norito et les artefacts d'audit partagent une référence unique côté opérateur
avant N1 (lancement public).

## 1. Portee et public

Le document cible :- Membres du Conseil de gouvernance qui votent sur les chartes, les politiques de
  suffixe et les résultats du litige.
- Membres du conseil des gardiens qui emettent des gels d'urgence et examinant
  les réversions.
- Stewards de suffixe qui gérent les fichiers du registraire, approuvent les enchères
  et gérent les partages de revenus.
- Opérateurs résolveur/passerelle responsables de la propagation SoraDNS, des mises à
  jour GAR, et des garde-fous de télémétrie.
- Equipes de conformite, tresorerie et support qui doivent démontrer que chaque
  action de gouvernance a laisse des artefacts Norito auditables.

Il couvre les phases de bêta fermée (N0), de lancement public (N1) et d'expansion (N2)
énumérées dans `roadmap.md` en dépendant de chaque workflow aux preuves requises,
tableaux de bord et voies d'escalade.

## 2. Rôles et carte de contact| Rôle | Responsabilités principales | Artefacts et télémétrie principaux | Escalade |
|------|-------------------|--------------------------|---------|
| Conseil de gouvernance | Redige et ratifie les chartes, politiques de suffixe, verdicts de litige et rotations de stewards. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, bulletins du conseil stocks via `sns governance charter submit`. | Président du conseil + suivi du dossier de gouvernance. |
| Conseil des tuteurs | Emet des gels soft/hard, canons d'urgence, et revues 72 h. | Billets gardien émis par `sns governance freeze`, manifeste d'override consignes sous `artifacts/sns/guardian/*`. | Gardien de rotation de garde (<=15 min ACK). |
| Stewards de suffixe | Gerent les fichiers du registraire, les enchères, les niveaux de prix et la communication client; accordent les conformités. | Politiques de steward dans `SuffixPolicyV1`, fiches de référence de prix, remerciements de steward stocks a cote des memos reglementaires. | Lead du programme steward + PagerDuty par suffixe. |
| Ops registraire et facturation | Exploitez les points de terminaison `/v1/sns/*`, réconciliant les paiements, emettent la télémétrie et maintiennent des instantanés CLI. | Registraire API ([`registrar-api.md`](./registrar-api.md)), métriques `sns_registrar_status_total`, preuves de paiement archivées sous `artifacts/sns/payments/*`. | Duty manager du registraire et liaison trésorerie. || Opérateurs résolveur et passerelle | Maintenez SoraDNS, GAR et l'état du gateway aligné avec les événements du registrar; diffuse les métriques de transparence. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | Résolveur SRE sur appel + passerelle d'opérations de pont. |
| Trésorerie et finance | Appliquent la répartition 70/30, les carve-outs de référence, les dépôts fiscaux/trésorerie et les attestations SLA. | Manifestes d'accumulation de revenus, exportations Stripe/tresorerie, annexes KPI trimestriels sous `docs/source/sns/regulatory/`. | Controleur finance + responsable conforme. |
| Liaison conforme et réglementaire | Suit les obligations globales (EU DSA, etc.), met à jour les covenants KPI et dépose des divulgations. | Mémos réglementaires dans `docs/source/sns/regulatory/`, decks de référence, entrées `ops/drill-log.md` pour les répétitions sur table. | Lead du programme de conformité. |
| Support / Astreinte SRE | Gère les incidents (collisions, dérive de facturation, pannes de résolveur), coordonne la communication client et possède les runbooks. | Modèles d'incident, `ops/drill-log.md`, preuves de labo mises en scène, transcriptions Slack/war-room archivees sous `incident/`. | Rotation d'astreinte SNS + gestion SRE. |

## 3. Artefacts canoniques et sources de données| Artefact | Emplacement | Objectif |
|--------------|-------------|---------|
| Charte + addenda KPI | `docs/source/sns/governance_addenda/` | Chartes signées avec contrôle de version, covenants KPI et décisions de gouvernance référencées par les votes CLI. |
| Schéma du registre | [`registry-schema.md`](./registry-schema.md) | Structures canoniques Norito (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Contrat du registraire | [`registrar-api.md`](./registrar-api.md) | Charges utiles REST/gRPC, métriques `sns_registrar_status_total` et attentes des hooks de gouvernance. |
| Guide UX d'adresses | [`address-display-guidelines.md`](./address-display-guidelines.md) | Rendus canoniques I105 (préférer) et compresses (deuxième choix) reproduites par les wallets/explorers. |
| Documents SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Dérivation déterministe des hôtes, workflow du tailer de transparence et règles d'alerte. |
| Mémos réglementaires | `docs/source/sns/regulatory/` | Notes d'accueil par juridiction (ex. EU DSA), intendant des remerciements, annexes de modèle. |
| Journal de forage | `ops/drill-log.md` | Journal des répétitions chaos et IR requis avant sorties de phase. |
| Stockage d'objets | `artifacts/sns/` | Preuves de paiement, gardien de tickets, résolveur de diffs, exports KPI et sortie CLI signée produite par `sns governance ...`. |Toutes les actions de gouvernance doivent référencer au moins un artefact du
tableau ci-dessus afin que les auditeurs puissent reconstituer la trace de
décision en 24 heures.

## 4. Playbooks de cycle de vie

### 4.1 Motions de charte et steward| Étape | Propriétaire | CLI/Preuve | Remarques |
|-------|-------------|--------------|-------|
| Rediger l'addendum et les deltas KPI | Rapporteur du conseil + délégué principal | Modèle Markdown stock sous `docs/source/sns/governance_addenda/YY/` | Inclure les ID de covenant KPI, les hooks de télémétrie et les conditions d'activation. |
| Soumettre la proposition | Président du conseil | `sns governance charter submit --input SN-CH-YYYY-NN.md` (produit `CharterMotionV1`) | La CLI émet un manifeste Norito stocke sous `artifacts/sns/governance/<id>/charter_motion.json`. |
| Gardien du vote et de la reconnaissance | Conseil + tuteurs | `sns governance ballot cast --proposal <id>` et `sns governance guardian-ack --proposal <id>` | Joignez les minutes hashs et les preuves de quorum. |
| Responsable de l'acceptation | Responsable du programme | `sns governance steward-ack --proposal <id> --signature <file>` | Requis avant changement des politiques de suffixe; enregistrer l'enveloppe sous `artifacts/sns/governance/<id>/steward_ack.json`. |
| Activation | Opérations du registraire | Mettre à jour `SuffixPolicyV1`, rafraichir les caches du registrar, publier une note dans `status.md`. | Journal d'activation de l'horodatage dans `sns_governance_activation_total`. |
| Journal d'audit | Conforme | Ajouter une entrée a `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` et au journal de forage si table effectuer. | Inclure des références aux tableaux de bord de télémétrie et aux différences de politique. |

### 4.2 Approbations d'enregistrement, d'enchère et de prix1. **Preflight :** Le registraire interroge `SuffixPolicyV1` pour confirmer le niveau
   de prix, les termes disponibles et les fenêtres de grâce/redemption. Garder les
   fiches de prix synchronisées avec le tableau de niveaux 3/4/5/6-9/10+ (niveau
   de base + coefficients de suffixe) documenté dans la feuille de route.
2. **Enchères en offre scellée :** Pour les pools premium, exécuter le cycle 72 h commit /
   Révélation 24 h via `sns governance auction commit` / `... reveal`. Publier la liste
   des commits (hashes uniquement) sous `artifacts/sns/auctions/<name>/commit.json`
   afin que les auditeurs puissent vérifier l'aléatoire.
3. **Vérification de paiement:** Les registraires valident `PaymentProofV1` par rapport
   aux répartitions de tresorerie (70% tresorerie / 30% steward avec carve-out de
   référence <=10 %). Stocker le JSON Norito sous `artifacts/sns/payments/<tx>.json`
   et le lier dans la réponse du registraire (`RevenueAccrualEventV1`).
4. **Hook de gouvernance:** Attacher `GovernanceHookV1` pour les noms premium/guarded
   en référencant les identifiants de proposition du conseil et les signatures de steward.
   Les crochets manquants se declenchent `sns_err_governance_missing`.
5. **Activation + sync solver:** Une fois que Torii emet l'événement de registre,
   declencher le tailer de transparence du solver pour confirmer que le nouvel
   l'état GAR/zone s'est propagé (voir 4.5).
6. **Client de divulgation :** Mettre à jour le client orienté grand livre (portefeuille/explorateur)via les luminaires partages dans [`address-display-guidelines.md`](./address-display-guidelines.md),
   en s'assurant que les rendus I105 et compresses correspondant aux guides copy/QR.

### 4.3 Renouvellements, facturation et réconciliation trésorerie- **Workflow de renouvellement:** Les registraires appliquent la fenêtre de grâce de
  30 jours + la fenêtre de rachat de 60 jours spécifiées dans `SuffixPolicyV1`.
  Après 60 jours, la séquence de reprise hollandaise (7 jours, frais 10x
  décroissant de 15%/jour) se declenche automatiquement via `sns governance reopen`.
- **Répartition des revenus:** Chaque renouvellement ou transfert créé un
  `RevenueAccrualEventV1`. Les exportations de trésorerie (CSV/Parquet) doivent
  concilier ces événements quotidiens; joindre les preuves à
  `artifacts/sns/treasury/<date>.json`.
- **Carve-outs de référence:** Les pourcentages de référence optionnels sont suivis
  par suffixe en ajoutant `referral_share` à la politique steward. Les registraires
  emettent le split final et stockent les manifestes de référence a cote de la
  preuve de paiement.
- **Cadence de reporting:** La finance publique des annexes KPI mensuelles
  (enregistrements, renouvellements, ARPU, utilisation des litiges/bonds) sous
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Les tableaux de bord doivent
  s'appuyer sur les memes tables exportées afin que les chiffres Grafana
  correspondant aux preuves du grand livre.
- **Revue KPI mensuelle:** Le checkpoint du premier mardi associe le lead finance,
  le steward de service et le PM programme. Ouvrir le [Tableau de bord SNS KPI](./kpi-dashboard.md)
  (intégrer le portail de `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`),exportateur les tables de throughput + revenus du registrar, consignateur les deltas
  dans l'annexe, et joindre les artefacts au mémo. Declencher un incident si la
  revue trouve des violations SLA (fenetres de freeze >72 h, photos d'erreurs du
  registraire, dériver l'ARPU).

### 4.4 Gels, litiges et appels| Phases | Propriétaire | Action et preuve | ANS |
|-------|--------------|--------|-----|
| Demande de gel doux | Intendant / support | Déposer un ticket `SNS-DF-<id>` avec preuves de paiement, référence de caution de litige et sélecteur(s) affecté(s). | <=4 h après l'entrée. |
| Gardien des billets | Conseil gardien | `sns governance freeze --selector <I105> --reason <text> --until <ts>` produit `GuardianFreezeTicketV1`. Stocker le JSON du ticket sous `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h d'exécution. |
| Ratification du conseil | Conseil de gouvernance | Approuver ou rejeter les gels, documenter la décision avec lien vers le ticket tuteur et le digest du lien de litige. | Prochaine séance du conseil ou vote asynchrone. |
| Panel d'arbitrage | Conforme + intendant | Convoquer un panel de 7 jurés (selon la feuille de route) avec des bulletins hash via `sns governance dispute ballot`. Joignez les recus de vote anonymisés au paquet d'incident. | Verdict <=7 jours après dépôt du cautionnement. |
| Appel | Gardien + conseil | Les appels doublent le lien et repetent le processus des jurés ; enregistrer le manifeste Norito `DisputeAppealV1` et référencer le ticket primaire. | <=10jours. |
| Degel et remédiation | Registraire + résolveur d'opérations | Executer `sns governance unfreeze --selector <I105> --ticket <id>`, mettre à jour le statut du registrar, et propager les diffs GAR/resolver. | Immédiatement après le verdict. |Les canons d'urgence (gels declenches par gardien <=72 h) suivez le meme flux
mais exigeant une revue rétroactive du conseil et une note de transparence sous
`docs/source/sns/regulatory/`.

### 4.5 Résolveur et passerelle de propagation

1. **Hook d'événement:** Chaque événement de registre emet vers le flux d'événements
   résolveur (`tools/soradns-resolver` SSE). Les ops solver s'abonnent et
   enregistrer les différences via le tailer de transparence
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Mise à jour du template GAR:** Les passerelles doivent mettre à jour les templates
   Références GAR par `canonical_gateway_suffix()` et re-signer la liste
   `host_pattern`. Stocker les différences dans `artifacts/sns/gar/<date>.patch`.
3. **Publication de zonefile:** Utiliser le squelette de zonefile décrit dans
   `roadmap.md` (name, ttl, cid, proof) et le pousser vers Torii/SoraFS. Archiveur
   le JSON Norito sous `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Vérification de transparence:** Exécuteur `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   pour s'assurer que les alertes restent vertes. Joindre la sortie texte
   Prometheus au rapport de transparence hebdomadaire.
5. **Audit gateway:** Enregistrer des enchantillons d'en-têtes `Sora-*` (policy cache,
   CSP, digest GAR) et les joindre au journal de gouvernance afin que les
   les opérateurs pourraient prouver que le gateway a servi le nouveau nom avec les
   garde-fous prévus.

## 5. Télémétrie et reporting| Signalisation | Source | Description/Action |
|--------|--------|----------------------|
| `sns_registrar_status_total{result,suffix}` | Gestionnaires registraire Torii | Compteur succès/erreur pour enregistrements, renouvellements, gels, transferts; alerte lorsque `result="error"` augmente par suffixe. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Métriques Torii | SLO de latence pour les gestionnaires API ; alimente des tableaux de bord issue de `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` et `soradns_bundle_cid_drift_total` | Résolveur de transparence | Détecte des preuves périmées ou des dérives GAR; garde-fous défini dans `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | Gouvernance des CLI | Compteur incrémente à chaque activation de charte/addendum ; utiliser pour réconcilier les décisions du conseil vs addenda publies. |
| Jauge `guardian_freeze_active` | Gardien CLI | Convient aux fenêtres de gel soft/hard par sélecteur; page SRE si la valeur reste `1` au-dela du SLA déclarer. |
| Tableaux de bord d'annexes KPI | Finances / Documents | Rollups mensuels publiés avec les mémos réglementaires; le portail les intègre via [SNS KPI Dashboard](./kpi-dashboard.md) pour que les stewards et régulateurs accèdent à la même vue Grafana. |

## 6. Exigences d'évidence et d'audit| Actions | Preuve d'un archiveur | Stockage |
|--------|-----------|----------|
| Changement de charte / politique | Signe du manifeste Norito, transcription CLI, KPI diff, intendant de reconnaissance. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Enregistrement/renouvellement | Payload `RegisterNameRequestV1`, `RevenueAccrualEventV1`, preuve de paiement. | `artifacts/sns/payments/<tx>.json`, logs API du registraire. |
| Enchère | Manifestes commit/reveal, graine d'aléatoire, tableur de calcul du gagnant. | `artifacts/sns/auctions/<name>/`. |
| Gel/dégel | Ticket Guardian, hash de vote du conseil, URL de log d'incident, modèle de communication client. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Résolveur de propagation | Diff zonefile/GAR, extrait JSONL du tailer, instantané Prometheus. | `artifacts/sns/resolver/<date>/` + rapports de transparence. |
| Admission réglementaire | Mémo d'accueil, tracker de délais, steward de reconnaissance, reprise des changements KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Checklist de porte de phase| Phases | Critères de sortie | Ensemble de preuves |
|-------|----------|------------------|
| N0 - Bêta fermée | Schéma de registre SN-1/SN-2, manuel du registraire CLI, gardien de forage complet. | Motion de charte + ACK steward, logs de dry-run du registrar, rapport de transparence résolveur, entrée dans `ops/drill-log.md`. |
| N1 - Lancement public | Enchères + niveaux de prix fixes actifs pour `.sora`/`.nexus`, libre-service du registraire, résolveur de synchronisation automatique, tableaux de bord de facturation. | Diff de feuille de prix, resultats CI du registrar, annexe paiement/KPI, sortie du tailer de transparence, notes de rehearsal incident. |
| N2 - Extension | `.dao`, revendeur APIs, portail de litige, gestionnaire de scorecards, tableaux de bord analytiques. | Captures d'écran du portail, métriques SLA de litige, exportations de scorecards steward, charte de gouvernance mise à jour avec politiques revendeur. |

Les sorties de phase exigeant des exercices sur table enregistrés (parcours registre
happy path, gel, panne solver) avec artefacts attachés dans `ops/drill-log.md`.

## 8. Réponse aux incidents et escalade| Déclencheur | Sévère | Propriétaire immédiat | Actions obligatoires |
|-------------|----------|-----------------------|----------------------|
| Derive solver/GAR ou preuves perimees | 1 septembre | Résolveur SRE + conseil gardien | Pager l'on-call solver, capturer la sortie tailer, décider si les noms affectés doivent être geles, afficher un statut toutes les 30 min. |
| Panne registrar, echec de facturation, ou erreurs API généralisées | 1 septembre | Duty manager du registraire | Arrêter les nouvelles enchères, basculer sur CLI manuel, notifier stewards/tresorerie, joindre les logs Torii au doc ​​d'incident. |
| Litige sur un seul nom, mismatch de paiement, ou escalade client | 2 septembre | Intendant + support principal | Collecter les preuves de paiement, déterminer si un gel soft est nécessaire, répondre au demandeur dans le SLA, consigner le résultat dans le tracker de litige. |
| Constat d'audit de conformité | 2 septembre | Liaison conforme | Rediger un plan de remédiation, déposer un mémo sous `docs/source/sns/regulatory/`, planifier une session de conseil de suivi. |
| Exercice ou répétition | 3 septembre | Programme PM | Exécuter le script de scénario depuis `ops/drill-log.md`, archiver les artefacts, étiquetter les lacunes comme les taches du roadmap. |Tous les incidents doivent créer `incident/YYYY-MM-DD-sns-<slug>.md` avec des
tables de propriété, des logs de commandes et des références aux preuves
produits tout au long de ce playbook.

## 9. Références

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
-`ops/drill-log.md`
- `roadmap.md` (rubriques SNS, DG, ADDR)

Garder ce playbook a jour chaque fois que le texte des chartes, les surfaces CLI
ou les contrats de télémétrie changent; les entrées du roadmap qui référencent
`docs/source/sns/governance_playbook.md` doivent toujours correspondre à la
dernière révision.