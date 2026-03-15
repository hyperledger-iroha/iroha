---
lang: he
direction: rtl
source: docs/portal/docs/da/threat-model.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::הערה מקור קנוניק
Reflete `docs/source/da/threat_model.md`. גרסאות גרדז לסינכרון
:::

# Modele de menaces זמינות נתונים Sora Nexus

_Revision Derniere: 2026-01-19 -- עדכון Prochaine Planifiee: 2026-04-19_

קצב תחזוקה: קבוצת עבודה זמינות נתונים (<=90 יו"ר). צ'אק
revision doit apparaitre dans `status.md` avec des liens vers les tickets de
אקטיביות למיתון ואמנות סימולציה.

## אבל והיקף

התוכנית זמינות נתונים (DA) שומרת על שידורים Taikai, les blobs
Nexus lane et les artefacts de gouvernance recuperables sous des fautes
ביזנטיים, ריזו ואופרה. Ce modele de menaces ancre le travail
הנדסה לשפוך DA-1 (ארכיטקטורה et modele de menaces) et sert de baseline
pour les taches DA suivantes (DA-2 ל-DA-10).

מרכיבים בהיקף:
- Extension d'ingest DA Torii et writers de metadata Norito.
- Arbres de stockage de blobs addosses a SoraFS (רמות חם/קר) et politiques de
  שכפול.
- Commitments de bloc Nexus (פורמטים של חוטים, הוכחות, APIs light-client).
- Hooks d'enforcement PDP/PoTR specifiques aux payloads DA.
- הפעלת זרימות עבודה (הצמדה, פינוי, חיתוך) וצינורות
  ד'צפיות.
- הסכמות הממשל qui admettent ou expulsent les operateurs et
  contenus DA.

היקף Hors pour ce מסמך:
- מודליזציה כלכלית הושלמה (capturee dans le workstream DA-7).
- בסיס פרוטוקולים SoraFS deja couverts par le modele de menaces SoraFS.
- Ergonomie des SDK לקוחות au-dela des considerations de surface de menace.

## Vue d'ensemble architecturale

1. **משלוח:** Les clients soumettent des blobs via l'API d'ingest DA Torii.
   Le noeud decoupe les blobs, מקודד les manifests Norito (סוג de blob, ליין,
   תקופה, flags de codec), et stocke les chunks dans le tier hot SoraFS.
2. **הכרזה:** כוונות סיכות ורמזים לשכפול של תומכים
   ספקים דרך הרישום (SoraFS marketplace) avec des tags de politique qui
   indiquent les objectifs de retention חם/קר.
3. **התחייבות:** Les sequencers Nexus כוללות את ההתחייבויות של הבלובים (CID +
   racines KZG optionnelles) dans le bloc canonique. Les light clients se basent
   sur le hash de commitment et la metadata annoncee pour verifier
   זמינות.
4. **שכפול:** Les noeuds de stockage tirent les shares/chunks assigns,
   satisfont les אתגרים PDP/PoTR, and promeuvent les donnees entre tiers hot
   וסלון קר לפוליטיקה.
5. **אחזר:** Les consommateurs recuperent les donnees via SoraFS ou des gateways
   מודע ל-DA, מאמת הוכחות ומשמעות של דרישות פיצויים
   des replicas disparaissent.
6. **ממשל:** Le Parlement et le comite de supervision DA approuvent les
   מבצעים, לוחות זמנים להשכרה והסלמות אכיפה. Les artefacts de
   governance sont stocks via la meme voie DA pour gargar la transparence.## Actifs et proprietaires

Echelle d'impact: **ביקורת** casse la securite/vivacite du ledger; **גובה**
bloque le backfill DA ou les clients; **Modere** degrade la qualite mais reste
ניתן להחלים; **מגבלה** יעילות מוגבלת.

| אקטיב | תיאור | שלמות | Disponibilite | סודי | בעלים |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (נתחים + מניפסטים) | Blobs Taikai, lane, governance stocks dans SoraFS | ביקורת | ביקורת | מודר | DA WG / צוות אחסון |
| Manifests Norito DA | סוג מטא-נתונים מגדיר את הכתמים | ביקורת | Eleve | מודר | Core Protocol WG |
| התחייבויות דה בלוק | CIDs + racines KZG dans les blocs Nexus | ביקורת | Eleve | פייבל | Core Protocol WG |
| לוחות זמנים PDP/PoTR | Cadence d'enforcement pour les replicas DA | Eleve | Eleve | פייבל | צוות אחסון |
| מפעיל הרישום | ספקי מלאי מאשרים ופוליטיות | Eleve | Eleve | פייבל | מועצת ממשל |
| רישומים להשכרה ותמריצים | ספר ראשונות לשכור DA et penalites | Eleve | מודר | פייבל | משרד האוצר |
| לוחות מחוונים לצפייה | SLOs DA, profondeur de replication, התראות | מודר | Eleve | פייבל | SRE / צפיות |
| כוונות דה תיקון | Requetes pour rehydrater des chunks manquants | מודר | מודר | פייבל | צוות אחסון |

## יריבים ויכולות

| שחקנית | קיבולות | מניעים | הערות |
| --- | --- | --- | --- |
| מזיק לקוח | Soumettre des blobs malformes, rejouer des manifests מיושן, tenter DoS sur l'ingest. | Perturber les משדרים את Taikai, injecter des donnees invalides. | Pas de cles privilegiees. |
| Noeud de stockage byzantin | מטפטף של מומחי העתקים, מזייף הוכחות PDP/PoTR, שותף. | Reduire la retentie DA, eviter la rent, retenir les donnees. | בעל תעודות תוקף. |
| פשרה ברצף | Omettre des מחויבויות, equivoker sur les blocs, reordonner la metadata. | Cacher des submissions DA, creer des incoherences. | גבול לעיקרי קונצנזוס. |
| מפעיל מתמחה | מתעלל בממשל, מניפולטור פוליטיקה של שימור, אישור אישורים. | להשיג חסכון, חבלה. | גישה לתשתית חמה/קרה. |
| Adversaire reseau | מחיצה ל-noudes, retarder la replication, מזריק תנועה MITM. | הפחת את הזמינות, משפיל את SLOs. | אין ספק, TLS ו-TLS, ו-Trapper/ralentir les liens. |
| צפיות אטקוונטית | חבל על לוחות מחוונים/התראות, אירועי הפתעה. | Cacher les הפסקות DA. | הכרחי גישה לטלמטריה של צינור. |

## Frontieres de confiance- **כניסה לגבולות:** לקוח לעומת הרחבה DA Torii. דרוש אישור סעיף
  בקש, הגבלת תעריף ואימות מטען.
- **שכפול גבולות:** Noeuds de stockage echangent chunks and proofs. לס
  noeuds sont mutuellement מאמת את mais peuvent se comporter en byzantin.
- **ספר חשבונות גבולות:** התחייבויות של Donnees de bloc לעומת מלאי מחוץ לשרשרת. לה
  קונצנזוס שומר על אינטגרציה, אבל הזמינות דורשת אכיפה
  מחוץ לשרשרת.
- **ממשל גבולות:** מפעילי המועצה/הפרלמנט מאשרים החלטות,
  תקציבים וקיצוץ. Les bris ici impactent direction le deploiement DA.
- **צפיות בגבולות:** איסוף מדדים/יומנים של מיצוא לעומת לוחות מחוונים
  כלי התראה. הפסקות המטמון מחבלות או תקיפות.

## תרחישים של איום ושליטה

### Attaques sur le chemin d'ingest

**תרחיש:** לקוח משבש soumet des payloads Norito malformes ou des
כתמים סורגים לשפוך את המשאבים או הזרקת מטא נתונים
אינו חוקי.

**בקרות**
- אימות סכימה Norito עם גרסה קפדנית של משא ומתן; דוחה les
  דגלים אינקוננוס.
- הגבלת קצב ואימות על נקודת הקצה של ה-Torii.
- גודל הנתח והקידוד קובע את כוחות החתיכה SoraFS.
- Pipeline d'admission ne persiste les manifests qu'apres match du checksum.
- הפעלה חוזרת של מטמון deterministe (`ReplayCache`) suit les fenetres `(נתיב, עידן,
  sequence)`, persiste des high-water marks sur disque, et rejette les duplicates
  et שידורים חוזרים מיושנים; רותמת נכסים וטביעות אצבע
  divergents et submissions hors order. [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Lacunes residuelles**
- Torii להטמיע את המטמון של ההשמעה החוזרת לכניסה ולתחזק את המקללים
  de sequence a travers les redemarrages.
- Les schemas Norito DA אנט תחזוקה ורתמה מטושטשת
  (`fuzz/da_ingest_schema.rs`) pour stresser les invariants d'encode/decode; les
  לוחות מחוונים דה couverture מאפשרים התראה על נסיגה.

### החזקה במניעת שכפול

**תרחיש:** Operators de stockage byzantins acceptent les pins mais dropent les
נתחים, מאתגרים PDP/PoTR דרך תשובות התשובות מזייפות או קנוניה.

**בקרות**
- לוח הזמנים של האתגרים PDP/PoTR s'etend aux payloads DA avec couverture par
  תקופה.
- שכפול רב מקורות avec seuils de quorum; l'orchestrateur detecte les
  רסיסים manquants et declenche la reparation.
- חיתוך הממשל lie aux proofs echoues et aux replicas manquantes.
- Job de reconciliation automatisee (`cargo xtask da-commitment-reconcile`) qui
  השווה בין קבלות עם התחייבויות DA (SignedBlockWire,
  `.norito` או JSON), emet un bundle JSON d'evidence pour la governance, et
  echoue sur tickets manquants/mismatches pour que Alertmanager puisse pager.**Lacunes residuelles**
- רתמת סימולציית `integration_tests/src/da/pdp_potr.rs` (סמויה
  par `integration_tests/tests/da/pdp_potr_simulation.rs`) תרגיל תרחישים
  de collusion et partition, תקף que le לוח הזמנים PDP/PoTR לזהות את זה
  comportement byzantin de facon deterministe. המשך ל-l'etendre avec DA-5
  pour couvrir de nouvelles surfaces de proof.
- La Politique d'Eviction קר שכבה מחייבת un trail audit signe pour prevenir
  les drops furtifs.

### מניפולציה של התחייבויות

**תרחיש:** פשרה של רצף, publie des blocs omettant ou modifiant les
מחויבויות דא, פרובוקטיבית של כשלי אחזור או חוסר קוהרנטיות קל-לקוח.

**בקרות**
- להסכמה לאמת את הצעות הגוש נגד קבצי ההגשה
  דא; les peers דוחה את ההצעות ללא מחויבויות הנדרשות.
- Les light clients מאמתים Les inclusion הוכחות Avant d'exposer les ידיות
  דה להביא.
- מעקב אחר הביקורת בהשוואה לקבלות בעלות התחייבויות בלוק.
- Job de reconciliation automatisee (`cargo xtask da-commitment-reconcile`) qui
  השווה בין קבלות עם התחייבויות DA (SignedBlockWire,
  `.norito` או JSON), emet un bundle JSON d'evidence pour la governance, et
  echoue sur tickets manquants ou חוסר התאמה pour que Alertmanager puisse pager.

**Lacunes residuelles**
- Couvert par le job de reconciliation + Hook Alertmanager; les paquets de
  ניהול אינו מתחזק לצרור JSON d'evidence par defaut.

### מחיצה וביקורת

**תרחיש:** Adversaire partitionne le reseau de replication, Empechant les
noeuds d'obtenir les chunks מקצה לאתגרים אחרים PDP/PoTR.

**בקרות**
- דרישות ספקים רב אזורי garantissent des chemins reseau divers.
- Fenetres de challenge avec jitter et fallback vers des canaux de reparation
  hors bande.
- לוחות מחוונים מעקבים מעקבים למקצוענים של שכפול, les
  success de challenge et la latence de fetch avec seuils d'alerte.

**Lacunes residuelles**
- Simulations de partition pour les evenements Taikai live הדרן manquantes;
  בסואין דה בדיקות.
- Politique de reservation de bande passante de reparation pas encore codifiee.

### אינטרנט לרעה

**תרחיש:** מפעיל בעל גישה לרישום מניפולציה לפוליטיקה
שימור, רשימת היתרים של ספקים מחליפים או מעודדות התראות.

**בקרות**
- פעולות הממשל המחייבות חתימות מרובות מפלגות ורשומות
  נוטריונים Norito.
- Les changements de politique emettent des evenements vers monitoring and logs
  d'archive.
- Le pipeline d'observabilite impose des logs Norito הוספה בלבד עם hash
  שרשור.
- L'automatisation d'audit trimestriel (`cargo xtask da-privilege-audit`) parcourt
  les repertoires manifest/replay (plus paths fournis par operateurs), signale les
  ערכים manquantes/non-directory/world-writable, et emet un bundle JSON signe
  יוצקים לוחות מחוונים דה שלטון.

**Lacunes residuelles**
- לוח המחוונים La preuve de tamper מחייב את סימני התמונות.

## רישום סיכון שאריות| ריסק | הסתברות | השפעה | בעלים | תכנית הפחתה |
| --- | --- | --- | --- | --- |
| Replay de manifests DA avant l'arrivee du sequence cache DA-2 | אפשרי | מודר | Core Protocol WG | מטמון רצף מיישם + validation de nonce en DA-2; ajouter des tests de regression. |
| קנוניה PDP/PoTR quand >f nouds sont compromis | Peu סביר | Eleve | צוות אחסון | הפק את לוח הזמנים החדש של אתגרים עם דגימה בין ספקים; תוקף באמצעות רתם דה סימולציה. |
| Gap d'audit d'eviction קר דרג | אפשרי | Eleve | SRE / צוות אחסון | Attacher des logs שלטים וקבלות על פינוי יוצקים בשרשרת; מוניטור באמצעות לוחות מחוונים. |
| Latence de detection d'omission de sequencer | אפשרי | Eleve | Core Protocol WG | `cargo xtask da-commitment-reconcile` nocturne השווה קבלות לעומת התחייבויות (SignedBlockWire/`.norito`/JSON) ודף ניהול כרטיסים או אי-התאמה. |
| מחיצת חוסן לשפוך זרמים Taikai בשידור חי | אפשרי | ביקורת | רשתות TL | Executer des drills de partition; reserver la bande passante de reparation; מתעד SOP de failover. |
| נגזר דה פריבילגיה דה שלטון | Peu סביר | Eleve | מועצת ממשל | `cargo xtask da-privilege-audit` trimestriel (dirs manifest/replay + paths extra) avec JSON signe + gate dashboard; ancrer les artefacts d'audit on-chain. |

## דרוש מעקב

1. Publier les schemas Norito d'ingest DA et des vecteurs d'exemple (porte dans
   DA-2).
2. Brancher le replay cache dans l'ingest DA Torii et persister les curseurs de
   רצף a travers les redemarrages.
3. **סיום (2026-02-05):** תחזוקת תרגילי PDP/PoTR לרתמה לסימולציה
   קנוניה + מחיצה עם מודליזציה של צבר QoS; voir
   `integration_tests/src/da/pdp_potr.rs` (בודק סו
   `integration_tests/tests/da/pdp_potr_simulation.rs`) pour l'implementation et
   les resumes deterministes לוכד ci-dessous.
4. **סיום (2026-05-29):** `cargo xtask da-commitment-reconcile` השווה בין
   קבלות d'ingest aux התחייבויות DA (SignedBlockWire/`.norito`/JSON), emet
   `artifacts/da/commitment_reconciliation.json`, et est branche a
   Alertmanager/Paquets de governance pour alertes d'omission/tampering
   (`xtask/src/da.rs`).
5. **Termine (2026-05-29):** `cargo xtask da-privilege-audit` parcourt le spool
   מניפסט/שידור חוזר (בתוספת נתיבים ארבעת מפעילים), כניסות סימנים
   manquantes/non-directory/world-writable, et produit un bundle JSON signe pour
   לוחות מחוונים/revues de governance (`artifacts/da/privilege_audit.json`),
   פרמננט la lacune d'automatisation d'acces.

**אתה רואה חדר רחצה צמוד:**- Le replay cache et la persistence des curseurs ont atterri en DA-2. Voir
  היישום ב-`crates/iroha_core/src/da/replay_cache.rs` (לוגיקה דה
  cache) et l'integration Torii ב-`crates/iroha_torii/src/da/ingest.rs`, qui thread
  les checks de טביעת אצבע דרך `/v1/da/ingest`.
- סימולציות של הזרמת תרגילי PDP/PoTR באמצעות זרימת הוכחה לרתמה
  dans `crates/sorafs_car/tests/sorafs_cli.rs`, couvrant les flux de requete
  PoR/PDP/PoTR ושאר תרחישי כישלון אנימות במודלים של איומים.
- תוצאות קיבולת ותיקון להשרות חיים חיים
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, tandis que la matrice de
  להשרות Sumeragi פלוס גדול est suivie dans `docs/source/sumeragi_soak_matrix.md`
  (כולל וריאציות מקומיות). חפצי אמנות Ces לוכדים תרגילים לטווח ארוך
  referencees dans le registre de risques residuels.
- התאמה אוטומטית + הרשאות-ביקורת vit dans
  `docs/automation/da/README.md` et les nouvelles commandes
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit`; utilisez
  les sorties par defaut sous `artifacts/da/` lors de l'attachement d'evidence aux
  חבילות שלטון.

## Evidence de Simulation and Modelization QoS (2026-02)

יוצקים יותר את המשך ה-DA-1 #3, קוד חדש של סימולציה
PDP/PoTR deterministe sous `integration_tests/src/da/pdp_potr.rs` (couvert par
`integration_tests/tests/da/pdp_potr_simulation.rs`). לה רתמה אללו דה
nouds sur trois regions, הזרקת מחיצות/קנוניה selon les probabilites du
מפת הדרכים, התאמה ל-PoTR של איחור, ומזון למודל של צבר תיקון
qui reflecte le budget de reparation du tier hot. L'execution du scenario par
defaut (12 עידנים, 18 אתגרים PDP + 2 fenetres PoTR par epoch) a produit les
מדדים סויבantes:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| מטריק | Valeur | הערות |
| --- | --- | --- |
| מגלים כשלים ב-PDP | 48 / 49 (98.0%) | Les partitions declenchent הדרן la detection; un seul echec non detecte vient d'un jitter honnete. |
| PDP ממוצע זמן זיהוי | 0.0 עידנים | Les echecs sont signales dans l'epoch d'origine. |
| מגלים כשלים ב-PoTR | 28 / 77 (36.4%) | La detection se declenche quand un noeud rate >=2 fenetres PoTR, laissant la plupart des evenements dans le registre de risques residuels. |
| PoTR ממוצע חביון זיהוי | 2.0 עידנים | Correspond au seuil de lateness a deux epochs integre dans l'escalation d'archivage. |
| שיא תור לתיקון | 38 מניפסטים | Le backlog monte quand les partitions s'empilent plus vite que les quatre reparations disponibles par epoch. |
| השהיית תגובה p95 | 30,068 אלפיות השנייה | Reflete la fenetre de challenge 30 שניות עם ריצוד של +/-75 ms applique au sampling QoS. |
<!-- END_DA_SIM_TABLE -->

Ces מחלץ את אבות הטיפוס של לוח המחוונים DA וסיפוק
les criteres d'acceptation "רתמת סימולציה + דוגמנות QoS" הפניות
מפת הדרכים.

L'automatisation vit derriere
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
qui appelle le harness partage et emet Norito JSON vers
`artifacts/da/threat_model_report.json` ברירת מחדל. Les jobs nocturnes
consomment ce fichier pour rafraichir les matrices dans ce document et alerter
sur la derive des taux de detection, des queues de reparation ou des samples QoS.Pour rafraichir la table ci-dessus pour les docs, executer `make docs-da-threat-model`,
qui invoque `cargo xtask da-threat-model-report`, regenere
`docs/source/da/_generated/threat_model_report.json`, וכתבו את הסעיף
דרך `scripts/docs/render_da_threat_model_tables.py`. Le miroir `docs/portal`
(`docs/portal/docs/da/threat-model.md`) est mis a jour dans le meme passage pour
que les deux copies restent en sync.