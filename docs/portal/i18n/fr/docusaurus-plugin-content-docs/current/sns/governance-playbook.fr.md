---
lang: fr
direction: ltr
source: docs/portal/docs/sns/governance-playbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Source canonique
Cette page reflete `docs/source/sns/governance_playbook.md` et sert maintenant de
copie canonique du portail. Le fichier source persiste pour les PRs de traduction.
:::

# Playbook de gouvernance du Sora Name Service (SN-6)

**Statut:** Redige 2026-03-24 - reference vivante pour la preparation SN-1/SN-6  
**Liens du roadmap:** SN-6 "Compliance & Dispute Resolution", SN-7 "Resolver & Gateway Sync", politique d'adresses ADDR-1/ADDR-5  
**Prerequis:** Schema du registre dans [`registry-schema.md`](./registry-schema.md), contrat d'API du registrar dans [`registrar-api.md`](./registrar-api.md), guide UX d'adresses dans [`address-display-guidelines.md`](./address-display-guidelines.md), et regles de structure de comptes dans [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Ce playbook decrit comment les organes de gouvernance du Sora Name Service (SNS)
adoptent des chartes, approuvent des enregistrements, escaladent les litiges, et
prouvent que les etats du resolver et du gateway restent synchronises. Il satisfait
l'exigence du roadmap selon laquelle la CLI `sns governance ...`, les manifestes
Norito et les artefacts d'audit partagent une reference unique cote operateur
avant N1 (lancement public).

## 1. Portee et public

Le document cible:

- Membres du Conseil de gouvernance qui votent sur les chartes, les politiques de
  suffixe et les resultats de litige.
- Membres du conseil des guardians qui emettent des gels d'urgence et examinent
  les reversions.
- Stewards de suffixe qui gerent les files du registrar, approuvent les encheres
  et gerent les partages de revenus.
- Operateurs resolver/gateway responsables de la propagation SoraDNS, des mises a
  jour GAR, et des garde-fous de telemetrie.
- Equipes de conformite, tresorerie et support qui doivent demontrer que chaque
  action de gouvernance a laisse des artefacts Norito auditables.

Il couvre les phases de beta fermee (N0), lancement public (N1) et expansion (N2)
enumerees dans `roadmap.md` en reliant chaque workflow aux preuves requises,
dashboards et voies d'escalade.

## 2. Roles et carte de contact

| Role | Responsabilites principales | Artefacts et telemetrie principaux | Escalade |
|------|-----------------------------|------------------------------------|---------|
| Conseil de gouvernance | Redige et ratifie les chartes, politiques de suffixe, verdicts de litige et rotations de stewards. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, bulletins du conseil stockes via `sns governance charter submit`. | President du conseil + suivi du docket de gouvernance. |
| Conseil des guardians | Emet des gels soft/hard, canons d'urgence, et revues 72 h. | Tickets guardian emis par `sns governance freeze`, manifestes d'override consignes sous `artifacts/sns/guardian/*`. | Rotation guardian on-call (<=15 min ACK). |
| Stewards de suffixe | Gerent les files du registrar, les encheres, les niveaux de prix et la communication client; reconnaissent les conformites. | Politiques de steward dans `SuffixPolicyV1`, fiches de reference de prix, acknowledgements de steward stockes a cote des memos reglementaires. | Lead du programme steward + PagerDuty par suffixe. |
| Ops registrar et facturation | Operent les endpoints `/v1/sns/*`, reconciliant les paiements, emettent la telemetrie et maintiennent des snapshots CLI. | API registrar ([`registrar-api.md`](./registrar-api.md)), metriques `sns_registrar_status_total`, preuves de paiement archivees sous `artifacts/sns/payments/*`. | Duty manager du registrar et liaison tresorerie. |
| Operateurs resolver et gateway | Maintiennent SoraDNS, GAR et l'etat du gateway aligne avec les evenements du registrar; diffusent les metriques de transparence. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | SRE resolver on-call + bridge ops gateway. |
| Tresorerie et finance | Appliquent la repartition 70/30, les carve-outs de referral, les depots fiscaux/tresorerie et les attestations SLA. | Manifestes d'accumulation de revenus, exports Stripe/tresorerie, appendices KPI trimestriels sous `docs/source/sns/regulatory/`. | Controleur finance + responsable conformite. |
| Liaison conformite et reglementaire | Suit les obligations globales (EU DSA, etc.), met a jour les covenants KPI et depose des divulgations. | Memos reglementaires dans `docs/source/sns/regulatory/`, decks de reference, entrees `ops/drill-log.md` pour les rehearsals tabletop. | Lead du programme de conformite. |
| Support / SRE on-call | Gere les incidents (collisions, derive de facturation, pannes de resolver), coordonne la communication client, et possede les runbooks. | Modeles d'incident, `ops/drill-log.md`, preuves de labo mises en scene, transcriptions Slack/war-room archivees sous `incident/`. | Rotation on-call SNS + management SRE. |

## 3. Artefacts canoniques et sources de donnees

| Artefact | Emplacement | Objectif |
|----------|-------------|---------|
| Charte + addenda KPI | `docs/source/sns/governance_addenda/` | Chartes signees avec controle de version, covenants KPI et decisions de gouvernance referencees par les votes CLI. |
| Schema du registre | [`registry-schema.md`](./registry-schema.md) | Structures Norito canoniques (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Contrat du registrar | [`registrar-api.md`](./registrar-api.md) | Payloads REST/gRPC, metriques `sns_registrar_status_total` et attentes des hooks de gouvernance. |
| Guide UX d'adresses | [`address-display-guidelines.md`](./address-display-guidelines.md) | Rendus canoniques IH58 (prefere) et compresses (second choix) reproduits par les wallets/explorers. |
| Docs SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Derivation deterministe des hosts, workflow du tailer de transparence et regles d'alerte. |
| Memos reglementaires | `docs/source/sns/regulatory/` | Notes d'accueil par juridiction (ex. EU DSA), acknowledgements steward, annexes de modele. |
| Journal de drill | `ops/drill-log.md` | Journal des rehearsals chaos et IR requis avant sorties de phase. |
| Stockage d'artefacts | `artifacts/sns/` | Preuves de paiement, tickets guardian, diffs resolver, exports KPI et sortie CLI signee produite par `sns governance ...`. |

Toutes les actions de gouvernance doivent referencer au moins un artefact du
tableau ci-dessus afin que les auditeurs puissent reconstruire la trace de
decision en 24 heures.

## 4. Playbooks de cycle de vie

### 4.1 Motions de charte et steward

| Etape | Proprietaire | CLI / Preuve | Notes |
|-------|-------------|--------------|-------|
| Rediger l'addendum et les deltas KPI | Rapporteur du conseil + lead steward | Template Markdown stocke sous `docs/source/sns/governance_addenda/YY/` | Inclure les IDs de covenant KPI, hooks de telemetrie et conditions d'activation. |
| Soumettre la proposition | President du conseil | `sns governance charter submit --input SN-CH-YYYY-NN.md` (produit `CharterMotionV1`) | La CLI emet un manifeste Norito stocke sous `artifacts/sns/governance/<id>/charter_motion.json`. |
| Vote et acknowledgement guardian | Conseil + guardians | `sns governance ballot cast --proposal <id>` et `sns governance guardian-ack --proposal <id>` | Joindre les minutes hashs et les preuves de quorum. |
| Acceptation steward | Programme steward | `sns governance steward-ack --proposal <id> --signature <file>` | Requis avant changement des politiques de suffixe; enregistrer l'enveloppe sous `artifacts/sns/governance/<id>/steward_ack.json`. |
| Activation | Ops du registrar | Mettre a jour `SuffixPolicyV1`, rafraichir les caches du registrar, publier une note dans `status.md`. | Timestamp d'activation logge dans `sns_governance_activation_total`. |
| Journal d'audit | Conformite | Ajouter une entree a `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` et au journal de drill si tabletop effectue. | Inclure des references aux dashboards de telemetrie et aux diffs de politique. |

### 4.2 Approbations d'enregistrement, d'enchere et de prix

1. **Preflight:** Le registrar interroge `SuffixPolicyV1` pour confirmer le niveau
   de prix, les termes disponibles et les fenetres de grace/redemption. Garder les
   fiches de prix synchronisees avec le tableau de niveaux 3/4/5/6-9/10+ (niveau
   de base + coefficients de suffixe) documente dans le roadmap.
2. **Encheres sealed-bid:** Pour les pools premium, executer le cycle 72 h commit /
   24 h reveal via `sns governance auction commit` / `... reveal`. Publier la liste
   des commits (hashes uniquement) sous `artifacts/sns/auctions/<name>/commit.json`
   afin que les auditeurs puissent verifier l'aleatoire.
3. **Verification de paiement:** Les registrars valident `PaymentProofV1` par rapport
   aux repartitions de tresorerie (70% tresorerie / 30% steward avec carve-out de
   referral <=10%). Stocker le JSON Norito sous `artifacts/sns/payments/<tx>.json`
   et le lier dans la reponse du registrar (`RevenueAccrualEventV1`).
4. **Hook de gouvernance:** Attacher `GovernanceHookV1` pour les noms premium/guarded
   en referencant les ids de proposition du conseil et les signatures de steward.
   Les hooks manquants declenchent `sns_err_governance_missing`.
5. **Activation + sync resolver:** Une fois que Torii emet l'evenement de registre,
   declencher le tailer de transparence du resolver pour confirmer que le nouvel
   etat GAR/zone s'est propage (voir 4.5).
6. **Divulgation client:** Mettre a jour le ledger oriente client (wallet/explorer)
   via les fixtures partages dans [`address-display-guidelines.md`](./address-display-guidelines.md),
   en s'assurant que les rendus IH58 et compresses correspondent aux guides copy/QR.

### 4.3 Renouvellements, facturation et reconciliation tresorerie

- **Workflow de renouvellement:** Les registrars appliquent la fenetre de grace de
  30 jours + la fenetre de redemption de 60 jours specifiees dans `SuffixPolicyV1`.
  Apres 60 jours, la sequence de reouverture hollandaise (7 jours, frais 10x
  decroissant de 15%/jour) se declenche automatiquement via `sns governance reopen`.
- **Repartition des revenus:** Chaque renouvellement ou transfert cree un
  `RevenueAccrualEventV1`. Les exports de tresorerie (CSV/Parquet) doivent
  reconciler ces evenements quotidiennement; joindre les preuves a
  `artifacts/sns/treasury/<date>.json`.
- **Carve-outs de referral:** Les pourcentages de referral optionnels sont suivis
  par suffixe en ajoutant `referral_share` a la politique steward. Les registrars
  emettent le split final et stockent les manifestes de referral a cote de la
  preuve de paiement.
- **Cadence de reporting:** La finance publie des annexes KPI mensuelles
  (enregistrements, renouvellements, ARPU, utilisation des litiges/bonds) sous
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Les dashboards doivent
  s'appuyer sur les memes tables exportees afin que les chiffres Grafana
  correspondent aux preuves du ledger.
- **Revue KPI mensuelle:** Le checkpoint du premier mardi associe le lead finance,
  le steward de service et le PM programme. Ouvrir le [SNS KPI dashboard](./kpi-dashboard.md)
  (embed portail de `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`),
  exporter les tables de throughput + revenus du registrar, consigner les deltas
  dans l'annexe, et joindre les artefacts au memo. Declencher un incident si la
  revue trouve des breaches SLA (fenetres de freeze >72 h, pics d'erreurs du
  registrar, derive ARPU).

### 4.4 Gels, litiges et appels

| Phase | Proprietaire | Action et preuve | SLA |
|-------|--------------|------------------|-----|
| Demande de gel soft | Steward / support | Deposer un ticket `SNS-DF-<id>` avec preuves de paiement, reference de bond de litige et selecteur(s) affecte(s). | <=4 h apres l'entree. |
| Ticket guardian | Conseil guardian | `sns governance freeze --selector <IH58> --reason <text> --until <ts>` produit `GuardianFreezeTicketV1`. Stocker le JSON du ticket sous `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h execution. |
| Ratification du conseil | Conseil de gouvernance | Approuver ou rejeter les gels, documenter la decision avec lien vers le ticket guardian et le digest du bond de litige. | Prochaine session du conseil ou vote asynchrone. |
| Panel d'arbitrage | Conformite + steward | Convoquer un panel de 7 jurors (selon roadmap) avec des bulletins hashes via `sns governance dispute ballot`. Joindre les recus de vote anonymises au paquet d'incident. | Verdict <=7 jours apres depot du bond. |
| Appel | Guardian + conseil | Les appels doublent le bond et repetent le processus des jurors; enregistrer le manifeste Norito `DisputeAppealV1` et referencer le ticket primaire. | <=10 jours. |
| Degel et remediation | Registrar + ops resolver | Executer `sns governance unfreeze --selector <IH58> --ticket <id>`, mettre a jour le statut du registrar, et propager les diffs GAR/resolver. | Immediatement apres le verdict. |

Les canons d'urgence (gels declenches par guardian <=72 h) suivent le meme flux
mais exigent une revue retroactive du conseil et une note de transparence sous
`docs/source/sns/regulatory/`.

### 4.5 Propagation resolver et gateway

1. **Hook d'evenement:** Chaque evenement de registre emet vers le flux d'evenements
   resolver (`tools/soradns-resolver` SSE). Les ops resolver s'abonnent et
   enregistrent les diffs via le tailer de transparence
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Mise a jour du template GAR:** Les gateways doivent mettre a jour les templates
   GAR references par `canonical_gateway_suffix()` et re-signer la liste
   `host_pattern`. Stocker les diffs dans `artifacts/sns/gar/<date>.patch`.
3. **Publication de zonefile:** Utiliser le squelette de zonefile decrit dans
   `roadmap.md` (name, ttl, cid, proof) et le pousser vers Torii/SoraFS. Archiver
   le JSON Norito sous `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Verification de transparence:** Executer `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   pour s'assurer que les alertes restent vertes. Joindre la sortie texte
   Prometheus au rapport de transparence hebdomadaire.
5. **Audit gateway:** Enregistrer des echantillons d'en-tetes `Sora-*` (policy cache,
   CSP, digest GAR) et les joindre au journal de gouvernance afin que les
   operateurs puissent prouver que le gateway a servi le nouveau nom avec les
   garde-fous prevus.

## 5. Telemetrie et reporting

| Signal | Source | Description / Action |
|--------|--------|----------------------|
| `sns_registrar_status_total{result,suffix}` | Gestionnaires registrar Torii | Compteur succes/erreur pour enregistrements, renouvellements, gels, transferts; alerte lorsque `result="error"` augmente par suffixe. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Metriques Torii | SLO de latence pour les handlers API; alimente des dashboards issus de `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` & `soradns_bundle_cid_drift_total` | Tailer de transparence resolver | Detecte des preuves perimees ou des derives GAR; garde-fous definis dans `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | CLI gouvernance | Compteur incremente a chaque activation de charte/addendum; utilise pour reconciler les decisions du conseil vs addenda publies. |
| `guardian_freeze_active` gauge | CLI guardian | Suit les fenetres de gel soft/hard par selecteur; page SRE si la valeur reste `1` au-dela du SLA declare. |
| Dashboards d'annexes KPI | Finance / Docs | Rollups mensuels publies avec les memos reglementaires; le portail les integre via [SNS KPI dashboard](./kpi-dashboard.md) pour que stewards et regulateurs accedent a la meme vue Grafana. |

## 6. Exigences d'evidence et d'audit

| Action | Evidence a archiver | Stockage |
|--------|---------------------|----------|
| Changement de charte / politique | Manifeste Norito signe, transcript CLI, diff KPI, acknowledgement steward. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Enregistrement / renouvellement | Payload `RegisterNameRequestV1`, `RevenueAccrualEventV1`, preuve de paiement. | `artifacts/sns/payments/<tx>.json`, logs API du registrar. |
| Enchere | Manifestes commit/reveal, graine d'aleatoire, tableur de calcul du gagnant. | `artifacts/sns/auctions/<name>/`. |
| Gel / degel | Ticket guardian, hash de vote du conseil, URL de log d'incident, modele de communication client. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Propagation resolver | Diff zonefile/GAR, extrait JSONL du tailer, snapshot Prometheus. | `artifacts/sns/resolver/<date>/` + rapports de transparence. |
| Intake reglementaire | Memo d'accueil, tracker de deadlines, acknowledgement steward, resume des changements KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Checklist de gate de phase

| Phase | Criteres de sortie | Bundle d'evidence |
|-------|--------------------|------------------|
| N0 - Beta fermee | Schema de registre SN-1/SN-2, CLI registrar manuel, drill guardian complete. | Motion de charte + ACK steward, logs de dry-run du registrar, rapport de transparence resolver, entree dans `ops/drill-log.md`. |
| N1 - Lancement public | Encheres + tiers de prix fixes actifs pour `.sora`/`.nexus`, registrar self-service, auto-sync resolver, dashboards de facturation. | Diff de feuille de prix, resultats CI du registrar, annexe paiement/KPI, sortie du tailer de transparence, notes de rehearsal incident. |
| N2 - Expansion | `.dao`, APIs reseller, portail de litige, scorecards steward, dashboards analytiques. | Captures ecran du portail, metriques SLA de litige, exports de scorecards steward, charte de gouvernance mise a jour avec politiques reseller. |

Les sorties de phase exigent des drills tabletop enregistres (parcours registre
happy path, gel, panne resolver) avec artefacts attaches dans `ops/drill-log.md`.

## 8. Reponse aux incidents et escalade

| Declencheur | Severite | Proprietaire immediat | Actions obligatoires |
|-------------|----------|-----------------------|----------------------|
| Derive resolver/GAR ou preuves perimees | Sev 1 | SRE resolver + conseil guardian | Pager l'on-call resolver, capturer la sortie tailer, decider si les noms affectes doivent etre geles, poster un statut toutes les 30 min. |
| Panne registrar, echec de facturation, ou erreurs API generalisees | Sev 1 | Duty manager du registrar | Arreter les nouvelles encheres, basculer sur CLI manuel, notifier stewards/tresorerie, joindre les logs Torii au doc d'incident. |
| Litige sur un seul nom, mismatch de paiement, ou escalation client | Sev 2 | Steward + lead support | Collecter les preuves de paiement, determiner si un gel soft est necessaire, repondre au demandeur dans le SLA, consigner le resultat dans le tracker de litige. |
| Constat d'audit de conformite | Sev 2 | Liaison conformite | Rediger un plan de remediation, deposer un memo sous `docs/source/sns/regulatory/`, planifier une session de conseil de suivi. |
| Drill ou rehearsal | Sev 3 | PM programme | Executer le scenario scripte depuis `ops/drill-log.md`, archiver les artefacts, etiqueter les gaps comme taches du roadmap. |

Tous les incidents doivent creer `incident/YYYY-MM-DD-sns-<slug>.md` avec des
tables de propriete, des logs de commandes et des references aux preuves
produites tout au long de ce playbook.

## 9. References

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (sections SNS, DG, ADDR)

Garder ce playbook a jour chaque fois que le texte des chartes, les surfaces CLI
ou les contrats de telemetrie changent; les entrees du roadmap qui referencent
`docs/source/sns/governance_playbook.md` doivent toujours correspondre a la
derniere revision.
