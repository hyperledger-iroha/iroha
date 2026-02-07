---
lang: he
direction: rtl
source: docs/portal/docs/sns/governance-playbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::הערה מקור קנוניק
Cette page reflete `docs/source/sns/governance_playbook.md` et sert Maintenant de
עותק canonique du portail. Le fichier source persiste pour les PRs de traduction.
:::

# Playbook de gouvernance du Sora Name Service (SN-6)

**סטטוט:** Redige 2026-03-24 - reference vivante pour la prepare SN-1/SN-6  
**שעבודים על מפת הדרכים:** SN-6 "תאימות ופתרון סכסוכים", SN-7 "Resolver & Gateway Sync", כתובות פוליטיות ADDR-1/ADDR-5  
**דרישה מוקדמת:** Schema du registre dans [`registry-schema.md`](./registry-schema.md), contrat d'API du registrar dans [`registrar-api.md`](./registrar-api.md), מדריך UX d'adresses dans [`address-display-guidelines.md`](./address-display-guidelines.md), et regles de structure de comptes dans [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Ce Playbook decrit comment les organes de governance du Sora Name Service (SNS)
מאמץ תרשימים, אישורי רישום, escaladent les litiges, ועוד
פרובנט que les etats du resolver et du gateway restent סינכרון. אני מרוצה
l'exigence du מפת הדרכים selon laquelle la CLI `sns governance ...`, les manifestes
Norito et les artefacts d'audit partagent une reference cote operator ייחודי
avant N1 (לציבור lancement).

## 1. Portee et public

קובץ המסמך:

- Membres du Conseil de gouvernance qui votent sur les chartes, les politiques de
  סיומת et les resultats de litige.
- Membres du conseil des guardians qui emettent des gels d'urgence et examinent
  les reversions.
- דיילים דה סיומת qui gerent les files du registrar, approuvent les encheres
  et gerent les partages de revenus.
- האחראים של מפעילי פתרון/שער להפצה SoraDNS, des mises a
  jour GAR, et des garde-fous de telemetrie.
- ציוד התאמה, תמיכה ותומכת בכל מה שצריך
  action de governance a laisse des artefacts Norito ביקורת.

Il couvre les phases de beta fermee (N0), lancement public (N1) והרחבה (N2)
Enumerees dans `roadmap.md` ב-reliant workflow chaque aux preuves requises,
לוחות מחוונים et voies d'escalade.

## 2. תפקידים וכרטיסי קשר| תפקיד | מנהלי אחריות | חפצי אומנות וטלמטריה | אסקלייד |
|------|-----------------------------|----------------------------------------|--------|
| Conseil de Governance | Redige et ratifie les charts, politiques de suffixe, dispositions de litige and rotations de stewards. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, bulletins du conseil stockes דרך `sns governance charter submit`. | President du conseil + suivi du docket de governance. |
| Conseil des guardians | Emet des gels soft/hard, canons d'urgence, et revues 72 h. | כרטיסים אפוטרופוס emis par `sns governance freeze`, manifestes d'override consignes sous `artifacts/sns/guardian/*`. | אפוטרופוס תורן (<=15 דקות ACK). |
| דיילים דה סיומת | Gerent les files du registrar, les encheres, les niveaux de prix et la client communication; reconnaissent les conformites. | Politiques de steward dans `SuffixPolicyV1`, פיches de reference de prix, acknowledgements de steward מחזיק ב-cote des memos reglementaires. | Lead du program steward + PagerDuty עם סיומת. |
| רשם פעולות ותפקוד | נקודות קצה פתוחות `/v1/sns/*`, תשלומים מתפשרים, טלמטריה ותחזוקה של צילומי SLI. | רשם API ([`registrar-api.md`](./registrar-api.md)), מדדים `sns_registrar_status_total`, preuves de paiement archivees sous `artifacts/sns/payments/*`. | מנהל תורנות דו רשם ותאגיד קשר. |
| מפעילים פותר ושער | Maintiennent SoraDNS, GAR et l'etat du gateway aligne avec les evenements du registrar; Les metriques de transparence מפוזר. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | פותר SRE בכוננות + שער פעולות גשר. |
| Tresorerie et finance | יישום של חלוקה 70/30, les carve-outs de referral, les depots fiscaux/tresorerie et les attestations SLA. | Manifestes d'accumulation de revenus, יצוא Stripe/tresorerie, נספחים KPI trimestriels sous `docs/source/sns/regulatory/`. | מפקח כספים + קונפורמייט אחראי. |
| Liaison conformite et reglementaire | Suit les obligations globales (EU DSA, וכו'), עמדו ב-Jour les covenants KPI ו-depose des divulgations. | Memos reglementaires dans `docs/source/sns/regulatory/`, decks de reference, entrees `ops/drill-log.md` pour les rehearsals tableop. | Lead du program de conformite. |
| תמיכה / SRE כוננות | יש תקריות (התנגשויות, יצירת תוצאות, פתרונות פתרון), קואורדונה של לקוח תקשורת, ועוד ספרי הפעלה. | Models d'incident, `ops/drill-log.md`, preuves de labo mises en scene, תעתיקים רפיון/חדרי מלחמה ארכיונים sous `incident/`. | רוטציה כוננית SNS + ניהול SRE. |

## 3. חפצי אמנות קנוניקים ומקורות קודש| חפץ | מיקום | Objectif |
|--------|-------------|--------|
| תרשים + תוספות KPI | `docs/source/sns/governance_addenda/` | חתימות רשומות בעלות שליטה בגרסה, אמנות KPI והחלטות ממשל מצביעות על הצבעות CLI. |
| Schema du registre | [`registry-schema.md`](./registry-schema.md) | מבנים Norito canoniques (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Contrat du registrar | [`registrar-api.md`](./registrar-api.md) | מטענים REST/gRPC, מדדים `sns_registrar_status_total` ו-attentes des hooks de governance. |
| מדריך UX d'adresses | [`address-display-guidelines.md`](./address-display-guidelines.md) | Rendus canoniques IH58 (מועדף) וקומפרס (בחירה שניה) משחזרים עבור ארנקים/חוקרים. |
| Docs SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | הגזירה קובעת את המארחים, זרימת העבודה עם התאמה של שקיפות ותקנות. |
| תזכירי תקנון | `docs/source/sns/regulatory/` | הערות לשמירה על שיפוט (לדוגמה, DSA של האיחוד האירופי), מנהל תודות, נספחים דגם. |
| Journal de drill | `ops/drill-log.md` | כתב העת לחזרות כאוס ו-IR דורשים מיונים מהפאזה. |
| מלאי חפצי אמנות | `artifacts/sns/` | Preuves de paiement, שומר כרטיסים, פותר הבדלים, יצוא KPI ו-CLI חתם מוצרים לפי `sns governance ...`. |

Toutes les actions de governance doivent referencer au moins un artefact du
tableau ci-dessus afin que les auditeurs puissent reconstruire la trace de
החלטה ב-24 שעות.

## 4. Playbooks de cycle de vie

### 4.1 Motions de Charte and Steward

| אטאפה | קניין | CLI / Preuve | הערות |
|-------|--------------|--------|-------|
| Rediger l'addendum et les deltas KPI | Rapporteur du conseil + דייל מוביל | תבנית Markdown stocke sous `docs/source/sns/governance_addenda/YY/` | כלול את מזהי האמנה של KPI, ווים לטלמטריה ותנאי הפעלה. |
| Soumettre la proposition | הנשיא du conseil | `sns governance charter submit --input SN-CH-YYYY-NN.md` (מוצר `CharterMotionV1`) | La CLI emet un manifeste Norito stocke sous `artifacts/sns/governance/<id>/charter_motion.json`. |
| הצבעה ואישור אפוטרופוס | קונסיל + אפוטרופוסים | `sns governance ballot cast --proposal <id>` et `sns governance guardian-ack --proposal <id>` | הצטרפו ל-hashs et les preuves de quorum. |
| דייל קבלה | מנהל התוכנית | `sns governance steward-ack --proposal <id> --signature <file>` | Requis avant changement des politiques de suffixe; רשום l'enveloppe sous `artifacts/sns/governance/<id>/steward_ack.json`. |
| הפעלה | אופס דו רשם | Mettre a jour `SuffixPolicyV1`, rafraichir les caches du registrar, publier une note dans `status.md`. | חותמת זמן להפעלה בכתובת `sns_governance_activation_total`. |
| Journal d'audit | קונפורמית | Ajouter une entree a `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` ו-au journal de drill עם אפקט שולחן. | כלול את הפניות aux לוחות מחוונים של טלמטריה ו-aux diffs de politique. |

### 4.2 אישורי רישום, אישורים ומחירים1. **טיסה מוקדמת:** חקירת הרשם `SuffixPolicyV1` pour confirmer le niveau
   de prix, les termes disponibles et les fenetres de grace/גאולה. גארדר לס
   fiches de prix Syncees avec le tableau de niveaux 3/4/5/6-9/10+ (niveau
   de base + coefficients de suffixe) documente dans le roadmap.
2. **Encheres sealed-bid:** Pour les pools premium, executer le cycle 72 h conmit /
   חשיפה של 24 שעות דרך `sns governance auction commit` / `... reveal`. Publier la List
   des commits (יחודיות גיבוב) sous `artifacts/sns/auctions/<name>/commit.json`
   afin que les auditeurs puissent verifier l'aleatoire.
3. **אימות תשלום:** הרשמים תקפים `PaymentProofV1` par rapport
   aux repartitions de tresorerie (70% tresorerie / 30% דייל עם carve-out de
   הפניה <=10%). Stocker le JSON Norito sous `artifacts/sns/payments/<tx>.json`
   et le lier dans la reponse du registrar (`RevenueAccrualEventV1`).
4. **Hook de governance:** Attacher `GovernanceHookV1` pour les noms premium/guarded
   en referencant les ids de proposition du conseil et les signatures de steward.
   Les hooks manquants declenchent `sns_err_governance_missing`.
5. **מסדר הפעלה + סינכרון:** Une fois que Torii emet l'evenement de registre,
   declencher le tailer de transparence du resolver pour confirmer que le nouvel
   etat GAR/zone s'est propage (voir 4.5).
6. **לקוח גילוי:** Mettre a jour le Ledger oriented client (ארנק/חוקר)
   via les fixtures partages dans [`address-display-guidelines.md`](./address-display-guidelines.md),
   en s'assurant que les rendus IH58 et compresses correspondent aux guides copy/QR.

### 4.3 חידושים, עיבוד ופיוס- **זרימת עבודה של חידוש:** Les registrars appliquent la fenetre de grace de
  30 ז'ור + la fenetre de redemption de 60 jours specifiees dans `SuffixPolicyV1`.
  לפני 60 יו"ר, la sequence de reouverture hollandaise (7 יו"ר, פריס 10x
  decroissant de 15%/jour) se declenche automatiquement via `sns governance reopen`.
- **Repartition des revenus:** Chaque renovellement ou transfert cree un
  `RevenueAccrualEventV1`. Les exports de tresorerie (CSV/Parquet) doivent
  reconciler ces evenements quotidiennement; joindre les preuves א
  `artifacts/sns/treasury/<date>.json`.
- **Carve-outs de referral:** Les pourcentages de referral optionnels sont suivis
  par suffixe en ajoutant `referral_share` a la politique steward. אלה הרשמים
  emettent le split final et stockent les manifestes de referral a cote de la
  preuve de paiement.
- **Cadence de reporting:** La finance publie des annexes KPI mensuelles
  (רישום, חידושים, ARPU, utilization des litiges/אג"ח) sous
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. לוחות המחוונים אינם מתאימים
  s'appuyer sur les memes טבלאות ייצוא afin que les chiffres Grafana
  correspondent aux preuves du ledger.
- **Revue KPI mensuelle:** Le checkpoint du premier mardi associe le lead finance,
  le steward de service et le PM תוכנית. Ouvrir le [לוח מחוונים SNS KPI](./kpi-dashboard.md)
  (הטמע portail de `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`),
  יצואן les tables de throughput + revenus du registrar, consigner les deltas
  dans l'anexe, et joindre les artefacts au memo. Declencher un incident si la
  revue trouve des breaches SLA (fenetres de freeze >72 h, pics d'erreurs du
  רשם, לגזור ARPU).

### 4.4 ג'לים, משפטים ואפלים

| שלב | קניין | Action et preuve | SLA |
|-------|--------------|----------------|-----|
| Demande de gel soft | דייל / תמיכה | מפקיד את כרטיס `SNS-DF-<id>` עם קדם תשלום, אסמכתא של בונד ליטיג ובחר/ים משפיעים. | <=4 שעות לפני הפתיחה. |
| אפוטרופוס כרטיס | אפוטרופוס קונסיל | `sns governance freeze --selector <IH58> --reason <text> --until <ts>` produit `GuardianFreezeTicketV1`. Stocker le JSON du ticket sous `artifacts/sns/guardian/<id>.json`. | <=30 דקות ACK, <=2 שעות ביצוע. |
| Ratification du conseil | Conseil de Governance | Approuver ou rejeter les gels, documenter la decision avec lien vers le ticket guardian et le digest du bond de litige. | Prochaine session du conseil אתה מצביע אסינכרוני. |
| פאנל ד'ארביטראז' | קונפורמית + דייל | Convoquer un פאנל של 7 מושבעים (סלון מפת דרכים) עם גיבוב של עלונים דרך `sns governance dispute ballot`. Joindre les recus de vote אנונימיז au paquet d'incident. | פסק דין <=7 ז'ור אפר דיפו דו בונד. |
| אפל | שומר + קונסיל | Les appels doublent le bond et repetent le processus des מושבעים; רשום le manifeste Norito `DisputeAppealV1` et referencer le ticket primaire. | <=10 ימים. |
| דגל ותיקון | רשם + פותר פעולות | המבצע `sns governance unfreeze --selector <IH58> --ticket <id>`, עם תקנון של הרשם, ומפיץ את ההבדלים GAR/פותר. | מיידית לפני פסק הדין. |Les canons d'urgence (ג'לים מדכאים לשומר <=72 שעות) suivent le meme flux
mais exigent une revue retroactive du conseil et une note de transparence sous
`docs/source/sns/regulatory/`.

### 4.5 פותר ריבוי ושער

1. **Hook d'evenement:** Chaque evenement de registre emet vers le flux d'evenements
   פותר (`tools/soradns-resolver` SSE). Les ops resolver s'abonnent et
   נרשם les diffs דרך le tailer de transparence
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Mise a jour du template GAR:** Les gateways doivent mettre a jour les templates
   הפניות ל-GAR לפי `canonical_gateway_suffix()` וחתימה מחדש ברשימה
   `host_pattern`. Stocker les diffs dans `artifacts/sns/gar/<date>.patch`.
3. **Publication de zonefile:** Utiliser le squelette de zonefile decrit dans
   `roadmap.md` (שם, ttl, cid, הוכחה) et le pousser vers Torii/SoraFS. ארכיון
   le JSON Norito sous `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **אימות השקיפות:** מפעל `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   pour s'assurer que les alertes restent vertes. Joindre la sortie texte
   Prometheus au rapport de transparence Hebdomadaire.
5. **שער ביקורת:** Enregistrer des echantillons d'en-tetes `Sora-*` (מטמון מדיניות,
   CSP, digest GAR) et les joindre au journal de governance afin que les
   Operators puissent prouver que le gateway a servi le nouveau nom avec les
   garde-fous prevus.

## 5. טלמטריה ודיווח

| אות | מקור | תיאור / פעולה |
|--------|--------|---------------------|
| `sns_registrar_status_total{result,suffix}` | רשם הריון Torii | Compteur success/erreur pour enregistrements, renovellements, gels, transferts; alerte lorsque `result="error"` augmente par suffixe. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Metriques Torii | SLO de latence pour les handlers API; alimente des לוחות מחוונים issus de `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` & `soradns_bundle_cid_drift_total` | Tailer de Transparence פותר | Detecte des preuves perimees ou des derives GAR; garde-fous definis dans `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | ניהול CLI | Compteur incremente a chaque activation de charte/תוספת; השתמש ב-Pour Reconciler Les decisions du conseil לעומת addenda publies. |
| מד `guardian_freeze_active` | אפוטרופוס CLI | Suit les fenetres de gel רך/קשה לבחירה; עמוד SRE si la valeur reste `1` au-dela du SLA declare. |
| לוחות מחוונים נספחים KPI | כספים / מסמכים | רולאפים mensuels publies avec les memos reglementaires; le portail les integre באמצעות [לוח המחוונים של SNS KPI](./kpi-dashboard.md) כדי שהדיילים והרגולטורים יגיעו ל-Grafana. |

## 6. דרישות הוכחות וביקורת| פעולה | עדות לארכיון | מלאי |
|--------|----------------------|--------|
| Changement de Charte / פוליטיקה | סימן מניפסט Norito, תמליל CLI, KPI שונה, מנהל אישור. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| רישום / חידוש | מטען `RegisterNameRequestV1`, `RevenueAccrualEventV1`, preuve de paiement. | `artifacts/sns/payments/<tx>.json`, יומן API של הרשם. |
| Enchere | מניפסט מתחייב/חושף, גרעין ד'אלאטואר, טבלת חישובים. | `artifacts/sns/auctions/<name>/`. |
| ג'ל / דגל | אפוטרופוס כרטיס, hash de vote du conseil, כתובת URL של יומן תקרית, מודל לקוח תקשורת. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| פותר ריבוי | Diff zonefile/GAR, exit JSONL du tailer, תמונת מצב Prometheus. | `artifacts/sns/resolver/<date>/` + יחסי שקיפות. |
| תקנון הכנסה | תזכורת, עוקב אחר מועדים, מנהל הכרה, KPI של קורות חיים לשינויים. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. רשימת רשימות של שער שלב

| שלב | קריטריונים דה מיון | צרור הוכחות |
|-------|------------------------|----------------|
| N0 - Beta fermee | סכימת רישום SN-1/SN-2, מדריך רשם CLI, שומר תרגיל מלא. | Motion de Charte + דייל ACK, יומני ריצה יבשים של הרשם, גורם פתרון שקיפות, מנה ראשונה ב-`ops/drill-log.md`. |
| N1 - Lancement public | Encheres + tiers de prix fixes actifs pour `.sora`/`.nexus`, שירות עצמי של רשם, פותר סנכרון אוטומטי, דה-פקטורציה של לוחות מחוונים. | הבדל דל פריס, תוצאות CI du registrar, נספח תשלום/KPI, מיון דו tailer de transparence, הערות לתקרית החזרות. |
| N2 - הרחבה | `.dao`, משווק ממשקי API, portail de litige, מנהל כרטיסי ניקוד, ניתוח לוחות מחוונים. | לוכד ecran du portail, מדדי SLA de litige, ייצוא של כרטיסי ניקוד דייל, Charte de Governance Mise a jour avec politiques משווק. |

Les sorties de phase דרושות מקדחות על גבי שולחן (parcours registre
שביל שמח, ג'ל, פנוי פאן) עם חפצי אמנות מצרף ב-`ops/drill-log.md`.

## 8. תגובה לאירועים והסלמה| דקלנצ'ור | חמור | קניין מיידי | פעולות חובה |
|------------|--------|----------------------|------------------------|
| נגזרת רזולובר/GAR ou preuves perimees | סב 1 | פותר SRE + אפוטרופוס קונסיל | ביפר ב-Call פותר, לוכד למסירה tailer, מחליטים כמה שמות משפיעים על האפשרויות, כרזה ותקנון של 30 דקות. |
| רשם Panne, Echec de facturation, או שגיאות ה-API מכלילות | סב 1 | מנהל חובה דו רשם | Arreter les nouvelles encheres, basculer sur CLI manuel, notifier stewards/tresorerie, joindre les logs Torii au doc ​​d'incident. |
| Litige sur un seul nom, mismatch de paiement, ou escalation client | סב' 2 | תמיכת דייל + מוביל | אספן les preuves de paiement, קובע סי un gel soft est ncessaire, repondre au demandeur dans le SLA, consigner le resultat dans le tracker de litige. |
| Constat d'audit de conformite | סב' 2 | Liaison conformite | Rediger un plan de remediation, deposer un memo sous `docs/source/sns/regulatory/`, planifier une session de conseil de suivi. |
| תרגיל או חזרה | סוו 3 | תוכנית ראש הממשלה | מבצע את תסריט התרחיש `ops/drill-log.md`, מאחסן חפצי אמנות, כללי נימוס לפערים שהגיעו למפת הדרכים. |

Tous les incidents doivent creer `incident/YYYY-MM-DD-sns-<slug>.md` avec des
tables de propriete, des logs de commandes et des references aux preuves
Produites tout au long de ce playbook.

## 9. הפניות

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (סעיפים SNS, DG, ADDR)

Garder ce playbook a jour chaque fois que le texte des chartes, les surfaces CLI
ou les contrats de telemetrie changent; Les מנות ראשונות דו מפת הדרכים qui referencent
`docs/source/sns/governance_playbook.md` doivent toujours correspondre a la
תיקון דרני.