---
lang: fr
direction: ltr
source: docs/portal/docs/sns/governance-playbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sns/governance_playbook.md` کی عکاسی کرتا ہے اور اب پورٹل
کی کینونیکل کاپی ہے۔ سورس فائل ترجمہ PRs کے لئے برقرار رہتی ہے۔
:::

# Sora Name Service est un service (SN-6)

**حالت:** 2026-03-24 مسودہ - SN-1/SN-6 تیاری کے لئے زندہ حوالہ  
**روڈمیپ لنکس:** SN-6 "Conformité et résolution des litiges", SN-7 "Résolveur et synchronisation de passerelle", ADDR-1/ADDR-5 ایڈریس پالیسی  
**پیشگی شرائط:** رجسٹری اسکیمہ [`registry-schema.md`](./registry-schema.md) میں، رجسٹرار API معاہدہ [`registrar-api.md`](./registrar-api.md) میں، ایڈریس UX رہنمائی [`address-display-guidelines.md`](./address-display-guidelines.md) میں، اور اکاؤنٹ اسٹرکچر قواعد [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md) میں۔

Il s'agit d'un service de noms de Sora (SNS) pour les utilisateurs de Sora Name Service (SNS).
ہیں، رجسٹریشن منظور کرتی ہیں، تنازعات کو escalader کرتی ہیں، اور ثابت کرتی ہیں کہ
résolveur et passerelle pour résoudre le problème یہ روڈمیپ کی اس ضرورت کو پورا
کرتی ہے کہ `sns governance ...` CLI, Norito manifeste et artefacts d'audit N1
(عوامی لانچ) سے پہلے ایک ہی آپریٹر ریفرنس شیئر کریں۔

## 1. دائرہ کار اور سامعین

یہ دستاویز ان کے لئے ہے:- Conseil de gouvernance کے اراکین جو چارٹرز، suffixe پالیسیوں اور تنازعہ نتائج پر ووٹ دیتے ہیں۔
- Le Conseil des Gardiens gèle les décisions et les inversions
- Suffixe stewards et registraire et ventes aux enchères et partage des revenus et répartition des revenus.
- Résolveur/passerelle pour SoraDNS et GAR pour les garde-corps de télémétrie et les garde-corps de télémétrie
- Conformité, soutien à la trésorerie, vérification des comptes et vérification des comptes Norito artefacts چھوڑے۔

یہ `roadmap.md` میں درج بند-بیٹا (N0) عوامی لانچ (N1) اور توسیع (N2) مراحل کو
کور کرتی ہے، ہر ورک فلو کو درکار شواہد، tableaux de bord et remontage
جوڑتی ہے۔

## 2. کردار اور رابطہ نقشہ| کردار | ذمہ داریاں | Objets façonnés et télémétrie | Escalade |
|------|----------|-------------------------------|---------------|
| Conseil de gouvernance | چارٹرز، suffixe پالیسیوں، تنازعہ فیصلوں اور rotations des stewards کی تدوین و توثیق۔ | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, bulletins de vote du conseil et `sns governance charter submit` pour le vote du conseil | Président du conseil + suivi du dossier de gouvernance۔ |
| Conseil des gardiens | gels doux/durs, canons, et avis de 72 h. | Les tickets Guardian et `sns governance freeze` sont des manifestes de remplacement et `artifacts/sns/guardian/*` sont disponibles pour tous. | Rotation de garde du tuteur (<=15 min ACK)۔ |
| Intendants de suffixe | files d'attente des bureaux d'enregistrement, enchères, niveaux de tarification et communications avec les clients. reconnaissances de conformité دیتے ہیں۔ | Politiques de gestion `SuffixPolicyV1` Feuilles de référence sur les prix, remerciements des responsables et notes réglementaires | Responsable du programme Steward + suffixe مخصوص PagerDuty۔ |
| Opérations de registraire et de facturation | `/v1/sns/*` les points de terminaison des paiements réconcilient les paiements et la télémétrie émettent des instantanés CLI pour les paiements. | API du registraire ([`registrar-api.md`](./registrar-api.md)), métriques `sns_registrar_status_total`, preuves de paiement et `artifacts/sns/payments/*` میں محفوظ ہیں۔ | Responsable de service du registraire et liaison avec la trésorerie || Opérateurs de résolution et de passerelle | SoraDNS, GAR et l'état de la passerelle et les événements du bureau d'enregistrement sont alignés sur les liens flux de mesures de transparence | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | Résolveur SRE d'astreinte + pont d'opérations de passerelle۔ |
| Trésorerie et Finances | Répartition des revenus à 70/30, exclusions de référence, déclarations fiscales/du Trésor et attestations SLA. | Manifestes de régularisation des revenus، Stripe/exportations de trésorerie، اور سہ ماہی Annexes KPI `docs/source/sns/regulatory/` میں۔ | Contrôleur financier + responsable de la conformité۔ |
| Conformité et liaison réglementaire | Les clauses restrictives (EU DSA) et les clauses restrictives des KPI ainsi que les divulgations et les informations relatives aux KPI | Mémos réglementaires `docs/source/sns/regulatory/` pour les platines de référence, pour les répétitions sur table et les entrées `ops/drill-log.md` | Responsable du programme de conformité۔ |
| Support / SRE d'astreinte | incidents (collisions, dérive de facturation, pannes du résolveur) et messagerie client ainsi que les runbooks et les runbooks. | Modèles d'incidents, `ops/drill-log.md`, preuves de laboratoire mises en scène, transcriptions Slack/war-room et `incident/` | Rotation d'astreinte SNS + gestion SRE۔ |

## 3. Objets façonnés et objets de collection| Artefact | مقام | مقصد |
|--------------|------|------|
| Charte + addenda KPI | `docs/source/sns/governance_addenda/` | Les engagements des KPI et les votes CLI sont également pris en compte. |
| Schéma du registre | [`registry-schema.md`](./registry-schema.md) | Carte Norito (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`) |
| Contrat de bureau d'enregistrement | [`registrar-api.md`](./registrar-api.md) | Charges utiles REST/gRPC, métriques `sns_registrar_status_total` et hook de gouvernance |
| Adresse du guide UX | [`address-display-guidelines.md`](./address-display-guidelines.md) | Rendus i105 (ترجیحی) / compressés (`sora`) (`sora`, deuxième meilleur) et portefeuilles/explorateurs pour les utilisateurs |
| Documents SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Dérivation déterministe de l'hôte, flux de travail de suivi de transparence et règles d'alerte |
| Notes réglementaires | `docs/source/sns/regulatory/` | Notes d'admission juridictionnelles (مثلا EU DSA) ، accusés de réception des responsables ، annexes du modèle ۔ |
| Journal de forage | `ops/drill-log.md` | Chaos et répétitions IR et sorties de phase |
| Stockage d'artefacts | `artifacts/sns/` | Preuves de paiement, tickets de gardien, différences de résolveur, exportations KPI, et `sns governance ...` et sortie CLI signée |

Il s'agit d'un artefact pour un artefact
چاہیے تاکہ auditeurs 24 گھنٹوں میں Decision Trail دوبارہ بنا سکیں۔

## 4. لائف سائیکل پلی بکس### 4.1 Charte et motions des délégués syndicaux

| مرحلہ | مالک | CLI / Preuves | نوٹس |
|-------|------|----------------|------|
| Projet d'addendum pour les deltas des KPI | Rapporteur du Conseil + responsable délégué | Modèle de démarque `docs/source/sns/governance_addenda/YY/` | ID d'engagement KPI, crochets de télémétrie et conditions d'activation |
| Proposition soumettre | Président du Conseil | `sns governance charter submit --input SN-CH-YYYY-NN.md` (`CharterMotionV1` Français) | CLI Norito manifeste `artifacts/sns/governance/<id>/charter_motion.json` میں محفوظ کرتی ہے۔ |
| Votez pour la reconnaissance du tuteur | Conseil + gardiens | `sns governance ballot cast --proposal <id>` et `sns governance guardian-ack --proposal <id>` | Minutes hachées et preuves de quorum |
| Acceptation du commissaire | Programme des délégués syndicaux | `sns governance steward-ack --proposal <id> --signature <file>` | Politiques de suffixe تبدیل کرنے سے پہلے لازم؛ `artifacts/sns/governance/<id>/steward_ack.json` enveloppe d'enveloppe en papier |
| Activation | Opérations du registraire | `SuffixPolicyV1` cache les caches du registraire `status.md` met en cache les caches du registraire | Horodatage d'activation `sns_governance_activation_total` میں لاگ ہوتا ہے۔ |
| Journal d'audit | Conformité | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` Journal de forage en bois massif sur table | Tableaux de bord de télémétrie et différences de politiques |

### 4.2 Inscription, enchères et approbations de prix1. **Vérification en amont :** Le registraire `SuffixPolicyV1` indique le niveau de tarification et les termes et conditions.
   fenêtres de grâce/rédemption قیمت کی شیٹس کو roadmap میں درج
   Niveau 3/4/5/6-9/10+ (niveau de base + coefficients de suffixe) avec synchronisation synchronisée
2. **Enchères à offres scellées :** Pools premium pour un engagement de 72 h / un cycle de révélation de 24 h
   `sns governance auction commit` / `... reveal` en anglais commettre فہرست
   (hachages) `artifacts/sns/auctions/<name>/commit.json` میں شائع کریں تاکہ
   les auditeurs vérifient le caractère aléatoire کر سکیں۔
3. **Vérification du paiement :** Greffiers `PaymentProofV1` et fractionnements de trésorerie.
   valider کرتے ہیں (70 % de trésorerie / 30 % d'intendant, exclusion de référence <= 10 %). NoritoJSON
   `artifacts/sns/payments/<tx>.json` Réponse du bureau d'enregistrement
   (`RevenueAccrualEventV1`) میں لنک کریں۔
4. **Crochet de gouvernance :** Premium/gardé ناموں کے لئے `GovernanceHookV1` لگائیں جس میں
   identifiants des propositions du conseil et signatures des délégués syndicaux Crochets manquants سے
   `sns_err_governance_missing` est en ligne
5. **Activation + synchronisation du résolveur :** Événement de registre Torii et transparence du résolveur
   tailer چلائیں تاکہ نیا GAR/zone state پھیلنے کی تصدیق ہو (4.5 دیکھیں)۔
6. **Divulgation client :** Grand livre destiné au client (portefeuille/explorateur) کو
   [`address-display-guidelines.md`](./address-display-guidelines.md) pour les appareils partagés
   Copie du rendu i105 compressé (`sora`) copie/QR
   orientation سے میل کھاتے ہیں۔### 4.3 Renouvellements, facturation et rapprochement de trésorerie

- **Flux de travail de renouvellement :** Bureau d'enregistrement `SuffixPolicyV1` pour 30 $ de grâce + 60 $
  fenêtres de remboursement 60 دن بعد Séquence de réouverture des Pays-Bas (7 دن، 10x frais جو
  15%/دن کم ہوتی ہے) خودکار طور پر `sns governance reopen` سے چلتی ہے۔
- **Répartition des revenus :** ہر renouvellement یا transfert `RevenueAccrualEventV1` بناتا ہے۔ Exportations du Trésor
  (CSV/Parquet) Les événements sont en cours de réconciliation et les événements sont réconciliés. preuves
  `artifacts/sns/treasury/<date>.json` میں منسلک کریں۔
- **Exclusions de référence :** اختیاری référence référence suffixe کے حساب سے `referral_share` کو
  politique de gestion responsable La division finale des bureaux d'enregistrement émettent کرتے ہیں
  اور manifestes de référence کو preuve de paiement کے ساتھ رکھتے ہیں۔
- **Cadence de reporting :** Finance et annexes KPI (enregistrements, renouvellements, ARPU, litige/caution)
  utilisation) `docs/source/sns/regulatory/<suffix>/YYYY-MM.md` میں پوسٹ کرتا ہے۔ Tableaux de bord
  Tableaux d'exportation pour les tables d'exportation Grafana Preuves du grand livre pour les clients
- ** Examen mensuel des KPI : ** Responsable des finances du point de contrôle, responsable du service et PM du programme
  [Tableau de bord SNS KPI] (./kpi-dashboard.md) کھولیں (portail intégré `sns-kpis` /
  `dashboards/grafana/sns_suffix_analytics.json`), débit du bureau d'enregistrement + tableaux de revenus
  exportation deltas annexe میں لاگ کریں، اور mémo d'artefacts اگر avis
  Violations du SLA (gel des fenêtres > 72 h, pics d'erreur du registraire, dérive de l'ARPU) et incident
  déclencheur کریں۔### 4.4 Gels, litiges et appels

| مرحلہ | مالک | Action et Preuve | ANS |
|-------|------|----------|-----|
| Demande de gel doux | Intendant / support | Ticket `SNS-DF-<id>` pour obtenir les preuves de paiement, la référence de la caution en litige, et le(s) sélecteur(s) concerné(s) | <=4 h de prise سے۔ |
| Billet de gardien | Conseil des gardiens | `sns governance freeze --selector <i105> --reason <text> --until <ts>` `GuardianFreezeTicketV1` Français Ticket JSON `artifacts/sns/guardian/<id>.json` pour le client | <=30 min ACK, <=2 h d'exécution۔ |
| Ratification du Conseil | Conseil de gouvernance | Geler approuver/rejeter le ticket du tuteur et le résumé des obligations de litige | اگلا séance du conseil یا vote asynchrone۔ |
| Commission d'arbitrage | Conformité + intendant | 7 panels de jurés (feuille de route pour les votes) avec bulletins de vote hachés `sns governance dispute ballot` pour les votes hachés Paquet d'incidents de reçus de vote anonymes میں لگائیں۔ | Verdict <=7 jours pour le dépôt de garantie |
| Appel | Gardien + conseil | Cautionnement d'appel et procédure du juré Norito Manifeste `DisputeAppealV1` Billet d'entrée principal et ticket principal | <=10 jours۔ |
| Dégel et correction | Opérations de registraire + résolveur | `sns governance unfreeze --selector <i105> --ticket <id>` Le statut du registraire est activé et les différences GAR/résolveur se propagent. | Verdict کے فوراً بعد۔ |

Canons d'urgence (gels déclenchés par un tuteur <= 72 h) pour le flux de travail
Pour l'examen rétroactif du conseil et pour la transparence `docs/source/sns/regulatory/`
note درکار ہے۔### 4.5 Résolveur et propagation de la passerelle

1. **Event hook :** par flux d'événements du résolveur d'événements de registre (`tools/soradns-resolver` SSE) par
   émettre ہوتا ہے۔ Les opérations de résolution s'abonnent à la bande-annonce de transparence
   (`scripts/telemetry/run_soradns_transparency_tail.sh`) Diffs de différence entre les deux
2. **Mise à jour du modèle GAR :** Passerelles et `canonical_gateway_suffix()`, reportez-vous aux modèles GAR
   اپڈیٹ کرنے ہوں گے اور `host_pattern` liste کو دوبارہ signe کرنا ہوگا۔ Différences
   `artifacts/sns/gar/<date>.patch` میں رکھیں۔
3. **Publication de Zonefile :** `roadmap.md` میں بیان کردہ squelette de fichier de zone (nom, ttl, cid, preuve)
   Appuyez sur le bouton Torii/SoraFS pour appuyer sur le bouton NoritoJSON
   `artifacts/sns/zonefiles/<name>/<version>.json` Archives des archives
4. **Contrôle de transparence :** `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   چلائیں تاکہ alertes vert رہیں۔ Sortie de texte Prometheus et rapport de transparence hebdomadaire
5. **Audit de passerelle :** Exemples d'en-tête `Sora-*` (politique de cache, CSP, résumé GAR)
   journal de gouvernance کے ساتھ joindre کریں تاکہ آپریٹرز ثابت کر سکیں کہ gateway نے نیا نام
   Les garde-corps مطلوبہ servent کیا۔

## 5. Rapports de télémétrie et de reporting| Signalisation | Source | Description/Action |
|--------|--------|----------------------|
| `sns_registrar_status_total{result,suffix}` | Gestionnaires de registraire Torii | رجسٹریشن، renouvellements, gels, transferts et compteur de réussite/erreur Le suffixe `result="error"` est indiqué dans l'alerte. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Métriques Torii | Gestionnaires d'API et SLO de latence `torii_norito_rpc_observability.json` pour les tableaux de bord et les flux d'informations |
| `soradns_bundle_proof_age_seconds` et `soradns_bundle_cid_drift_total` | Résolveur de transparence | پرانے preuves یا GAR drift کو détecter کرتا ہے؛ garde-corps `dashboards/alerts/soradns_transparency_rules.yml` میں définir ہیں۔ |
| `sns_governance_activation_total` | Gouvernance CLI | ہر activation de la charte/addendum پر بڑھنے والا compteur؛ les décisions du conseil et les addenda publiés کے ساتھ concilier کرنے میں استعمال۔ |
| Jauge `guardian_freeze_active` | CLI gardien | Sélecteur de piste de gel logiciel/dur pour Windows Le `1` SLA est en ligne avec la page SRE |
| Tableaux de bord annexes KPI | Finances / Documents | Des rollups et des mémos réglementaires sont également disponibles. portail انہیں [SNS KPI Dashboard] (./kpi-dashboard.md) کے ذریعے intégrer کرتا ہے تاکہ stewards اور régulateurs ایک ہی Grafana view تک پہنچیں۔ |

## 6. Preuves et exigences d'audit| Actions | Preuves à archiver | Stockage |
|--------|----------|---------|
| Changement de Charte/politique | Manifeste Norito signé, transcription CLI, KPI diff, accusé de réception de l'intendant | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Inscription / renouvellement | Charge utile `RegisterNameRequestV1`, `RevenueAccrualEventV1`, preuve de paiement۔ | `artifacts/sns/payments/<tx>.json`, journaux de l'API du registraire۔ |
| Vente aux enchères | Valider/révéler des manifestes, des graines aléatoires, une feuille de calcul de calcul du gagnant | `artifacts/sns/auctions/<name>/`. |
| Geler/dégeler | Ticket du gardien, hachage du vote du conseil, URL du journal des incidents, modèle de communication client | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Propagation du résolveur | Zonefile/GAR diff, extrait JSONL de queue, instantané Prometheus | `artifacts/sns/resolver/<date>/` + rapports de transparence۔ |
| Admission réglementaire | Mémo d'admission, suivi des délais, accusé de réception du responsable, résumé des modifications des KPI | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Liste de contrôle des portes de phase| Phases | Critères de sortie | Paquet de preuves |
|-------|---------------|-----------------|
| N0 - Bêta fermée | Schéma de registre SN-1/SN-2, registraire manuel CLI, foret gardien مکمل۔ | Motion de charte + steward ACK, journaux d'essais du registraire, rapport de transparence du résolveur, entrée `ops/drill-log.md`۔ |
| N1 - Lancement public | Enchères + niveaux à prix fixe en direct pour `.sora`/`.nexus`, registraire en libre-service, synchronisation automatique du résolveur, tableaux de bord de facturation۔ | Fiche de tarification diff, résultats CI du registraire, annexe de paiement/KPI, sortie du tailer de transparence, notes de répétition d'incident |
| N2 - Extension | `.dao`, API de revendeur, portail de litige, cartes de pointage de steward, tableaux de bord d'analyse | Captures d'écran du portail, contestation des métriques SLA, exportations des cartes de pointage des responsables, charte de gouvernance mise à jour faisant référence aux politiques des revendeurs. |

Sorties de phase avec exercices de table (chemin heureux d'enregistrement, gel, panne du résolveur)
artefacts `ops/drill-log.md` میں منسلک ہوں۔

## 8. Réponse aux incidents et escalade| Déclencheur | Gravité | Propriétaire immédiat | Actions obligatoires |
|---------|----------|-----------------|-------------------|
| Dérive du résolveur/GAR et des preuves périmées | 1 septembre | Résolveur SRE + carte gardienne | résolveur d'astreinte pour la page de capture de sortie du tailer et pour le gel des tâches pendant 30 min de mise à jour du statut |
| Panne du bureau d'enregistrement, échec de facturation, erreurs d'API généralisées | 1 septembre | Responsable de service du registraire | Vente aux enchères Manuel CLI pour les stewards/trésorerie Torii journaux et document d'incident en pièce jointe |
| Litige portant sur un seul nom, inadéquation des paiements, ou remontée du client | 2 septembre | Steward + responsable de support | les preuves de paiement sont un soft freeze et un système de suivi des litiges SLA est un demandeur et un système de suivi des litiges. |
| Conclusion de l'audit de conformité | 2 septembre | Agent de conformité | plan d'assainissement `docs/source/sns/regulatory/` note de service pour le suivi de la séance du conseil |
| Exercice یا répétition | 3 septembre | Programme PM | `ops/drill-log.md` Scénario scripté pour les archives d'artefacts et les lacunes et les tâches de la feuille de route |

Les incidents sont liés à `incident/YYYY-MM-DD-sns-<slug>.md` pour les tables de propriété
journaux de commandes, vous avez besoin de preuves et de références

## 9. حوالہ جات- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
-`ops/drill-log.md`
- `roadmap.md` (sections SNS, DG, ADDR)

Il y a le libellé de la charte, les surfaces CLI et les contrats de télémétrie.
اپڈیٹ رکھیں؛ entrées de la feuille de route par `docs/source/sns/governance_playbook.md` par
ریفرنس کرتی ہیں انہیں ہمیشہ تازہ ترین ورژن سے میل کھانا چاہیے۔