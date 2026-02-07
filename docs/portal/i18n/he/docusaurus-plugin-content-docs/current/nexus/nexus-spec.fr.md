---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-spec.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-spec
כותרת: Specification technique de Sora Nexus
תיאור: Miroir complet de `docs/source/nexus.md`, couvrant l'architecture et les contraintes de conception pour le Ledger Iroha 3 (Sora Nexus).
---

:::הערה מקור קנוניק
Cette עמוד reprend `docs/source/nexus.md`. Gardez les deux copies alignees jusqu'a ce que le backlog de traduction arrive sur le portail.
:::

#! Iroha 3 - Sora Nexus Ledger: Specification technique de conception

Ce document propose l'architecture du Sora Nexus Ledger pour Iroha 3, evolution evolution Iroha 2 vers un Ledger global unique and logiquement unifie organization autour des Data Spaces (DS). Les Data Spaces fournissent des domaines de confidentialite forts ("מרחבי נתונים פרטיים") ו- une participation ouverte ("מרחבי נתונים ציבוריים"). Le design Preserve la composabilite and travers le Ledger Global Tout en garantsant une isolation stricte et la confidentialite des donnees private-DS, and introduit l'echelle de disponibilite des donnees via l'effacement code sur Kura (אחסון בלוק) ו-WSV (World State View).

Le meme depot construit Iroha 2 (reseaux auto-heberges) et Iroha 3 (SORA Nexus). L'execution est assuree par l'Iroha Machine Virtual (IVM) partagee et la toolchain Kotodama, afin que les contrats and artefacts de bytecode restent portables entre les deployments ledger05 global et.108

אובייקטים
- לוגיקה גלובלית של ספרי חשבונות, המרכיבים את השמות של משתפי אימות ו-Data Spaces.
- Des Data Spaces prives pour une permissionnee (עמ' למשל, CBDC), avec des donnees qui ne quittent jamais le DS prive.
- הציבור של Des Data Spaces עם השתתפות פתוחה, גישה ללא הרשאה מסוג Ethereum.
- Des contrats intelligents composables entre Data Spaces, soumis and des permissions explicites pour l'acces aux actifs private-DS.
- Isolation de performance afin que l'activite publicque ne degrade pas les עסקאות פנימיות פרטיות-DS.
- Disponibilite des donnees a grande echelle: Kura et WSV avec fefacement code pour supporter des donnees effectments illimitees tout en gardant les donnees private-DS privates.

Non-objectifs (שלב ראשי תיבות)
- Definir l'economie de token ou les incitations des validateurs; les politiques de scheduling et de staking sont pluggables.
- Introduire une nouvelle גרסה ABI; les changements ciblent ABI v1 avec des extensions explicites de syscalls et pointer-ABI selon la politique IVM.טרמינולוגיה
- Nexus ספר חשבונות: הלוגיקה הגלובלית של פנקסי החשבונות ב-Blocs Data Space (DS) עם היסטוריה ייחודית ואיחוד אירוסין.
- מרחב נתונים (DS): Domaine d'execution et de stockage borne withec ses propres validateurs, governance, classe de confidentialite, politique DA, quotas and politique de frais. קיימות מחלקות Deux: DS ציבורי ו-DS פרטי.
- מרחב נתונים פרטי: אישורי אימות ובקרת גישה; les donnees de transaction et d'etat ne quittent jamais le DS. Seuls des engagements/metadonnees sont ancres globalment.
- מרחב נתונים ציבורי: השתתפות ללא רשות; les donnees completes et l'etat sont publics.
- מניפסט מרחב נתונים (מניפסט DS): קוד מניפסט ב-Norito כדי להצהיר על הפרמטרים של DS (מאמינים/קליקים QC, classe de confidentialite, פוליטיקה ISI, פרמטרים DA, שמירה, מכסות, פוליטיקה ZK, פריס). Le hash du manifest est ancre sur la chaine nexus. מעקף, les certificates de quorum DS utilisent ML-DSA-87 (classe Dilithium5) comme schema de signature post-quantique par defaut.
- ספריית שטח: ניגוד ספריות על-שרשרת גלובליות המאפשרות לעקוב אחר גילויים של DS, גרסאות ואירועים של ניהול/רוטציה עבור פתרון וביקורות.
- DSID: זיהוי עולמי ייחודי למרחב נתונים. השתמש במרווחי שמות של חפצים ואזכורים.
- עוגן: Engagement cryptographique d'un bloc/header DS inclusive dans la chaine nexus pour lier l'historique DS au ledger global.
- Kura: Stockage de blocks Iroha. Etendu ici avec stockage de blobs קוד מחיקת התקשרויות.
- WSV: Iroha World State View. תקצירים של קטעי גרסאות, תמונות מצב וקודים למחיקה.
- IVM: Iroha Machine Virtual pour l'execution de contrats intelligents (bytecode Kotodama `.to`).
  - AIR: ייצוג ביניים אלגברי. Vue algebrique du calcul pour des preuves סוג STARK, decrivant l'execution comme des traces basees sur des champs avec contraintes de transition et de frontiere.מודל מרחבי נתונים
- זיהוי: `DataSpaceId (DSID)` זהה עם DS ומרחב שמות. Les DS peuvent etre instancies a deux granularites:
  - Domain-DS: `ds::domain::<domain_name>` - ביצוע ותחום תחום.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - ביצוע etat scopes a une definition d'actif unique.
  Les deux formes דו-קיום; les עסקאות peuvent toucher plusieurs DSID de maniere atomique.
- Cycle de vie du manifest: יצירת DS, mises a jour (רוטציה דה cles, changements de politique) ו-Retrait retrait registers in Space Directory. Chaque artefact DS par חריץ הפניה le hash du manifest le plus לאחרונה.
- שיעורים: DS ציבורי (השתתפות auverte, DA publique) ו- DS פרטי (רשות, DA confidentielle). Des politiques hybrides sont possibles via des flags de manifest.
- Politiques par DS: הרשאות ISI, פרמטרים DA `(k,m)`, chiffrement, שמירה, מכסות (חלק min/max de tx par bloc), politique de preuve ZK/optimiste, frais.
- Gouvernance: la gouvernance de membres DS et la rotation des validateurs sont definies par la section governance du manifest (הצעות על שרשרת, multisig, ou governance externe ancree par transactions nexus et attestations).

גילויי יכולות ו-UAID
- Comptes universels: chaque participant recoit un UAID deterministe (`UniversalAccountId` in `crates/iroha_data_model/src/nexus/manifest.rs`) qui couvre tous les dataspaces. Les manifests de capacites (`AssetPermissionManifest`) שוכן ל-UAID וספציפי מרחב נתונים, תקופות של הפעלה/תפוגה ורשימה חוקית לאפשר/להכחיש `ManifestEntry` qui bornent Norito,1005,1005, Norito, `method`, `asset` ואפשרויות התפקידים של AMX. Les regles להכחיש gagnent toujours; l'evaluateur emet `ManifestVerdict::Denied` עם קיומו של ביקורת או מענק `Allowed` עם מטא-נתונים מתכתבים.
- קצבאות: chaque entree לאפשר transporte des buckets deterministes `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) בתוספת `max_amount` אופציונלי. המארחים וצרכן SDK למטען זיכרון Norito, היישום שאר חומרה זהה ו-SDK.
- Telemetrie d'audit: Space Directory diffuse `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) chaque fois qu'un manifest change d'etat. La nouvelle surface `SpaceDirectoryEventFilter` permet aux abonnes Torii/data-event de monitorer les mises a jour de manifest UAID, les revocations and les decisions deny-wins sans plumbing sur mesure.

Pour des preuves operationur מקצה לקצה, הערות המעבר SDK ו-Checklists de publication de manifest, miroitez cette section with the Universal Account Guide (`docs/source/universal_accounts_guide.md`). מסמכי Gardez les deux alignes quand la politique ou les outils UAID changent.אדריכלות דה רמה גבוהה
1) ספת קומפוזיציה גלובל (Nexus שרשרת)
- שמירה על הזמנה קנונית ייחודית של בלוקים Nexus לאחר 1 שניה סופית של עסקאות אטומיות אבטחה או מרחבי נתונים נוספים (DS). התחייבות לעסקאות צ'אקה נפגשה עם איחוד עולמי של המדינה העולמית (vecteur de roots par DS).
- Contient des metadonnees minimales plus des preuves/QC agreges pour assurer la composabilite, la finalite et la detection de fraude (DSIDs touches, roots d'etat par DS avant/apres, engagements DA, preuves de validite par DS, et le certificat de quorum DSSA avec-8-ML). Aucune donnee privee n'est inclusive.
- קונצנזוס: comite BFT Global pipeline de taille 22 (3f+1 avec f=7), selectne parmi un pool de jusqu'a ~200k validateurs potentials through un mecanism VRF/stake par epochs. רצף הקשר בין העסקאות וסיים את הגוש ב-1.

2) מרחב נתונים ספות (ציבורי/פרטי)
- בצע את השברים של DS des transactions globales, פגשתי את ה-WSV local du DS et produit des artefacts de validite par bloc (preuves par DS agregees et engagements DA) qui remontent dans le bloc Nexus de 1 seconde.
- Les Private DS chiffrent les donnees au repos et en transit entre validateurs autorises; Seuls les engagements and preuves de validite PQ quittent le DS.
- Les Public DS exportent les corps complets de donnees (via DA) et les preuves de validite PQ.

3) טרנזקציות אטומיות בין נתונים-מרחב (AMX)
- דגם: chaque transaction utilisateur peut toucher plusieurs DS (עמ' למשל, דומיין DS et un ou plusieurs asset DS). Elle est commit atomiquement dans un bloc Nexus ייחודי או elle avorte; aucun effet partiel.
- Prepare-Commit ב-1s: pour chaque transaction candidate, les DS touches executent and parallele contre le meme snapshot (שורשי DS ב-Slot) ו-produisent des preuves de validite PQ par DS (FASTPQ-ISI) et des engagements DA. Le comite nexus commit la transaction seulement si toutes les preuves DS דורש אימות ותעודות DA arrivent a temps (objectif <=300 ms); Sinon la Transaction est re-planifiee pour le slot suivant.
- עקביות: les ensembles lecture/ecriture sont מצהיר; la detection de conflits a lieu au commit contre les roots debut de slot. L'execution optimiste sans locks par DS evite les stalls globaux; l'atomicite est imposee par la regle de commit nexus (tout ou rien entre DS).
- סודי: היחודיות הפרטית של DS יצואנית des preuves/engagements lies aux roots DS pre/post. Aucune donnee privee brute ne quitte le DS.4) Disponibilite des donnees (DA) עם קוד מחיקה
- Kura stocke les corps de blocs ו-Snapshots WSV comme des blobs קוד מחיקה. Les blobs publics sont largement sharde; les blobs prives sont stocks ייחודיות chez les validateurs private-DS, avec des chunks chiffres.
- Les engagements DA sont registeres a la fois dans les artefacts DS et dans les blocs Nexus, permettant des garanties de sampling et de recuperation sans reveler de contenu prive.

Structure de bloc et de commit
- Artefact de preuve Data Space (par slot de 1s, par DS)
  - Champs: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Les Private-DS exportent des artefacts sans corps de donnees; les Public DS permettent la recuperation via DA.

- Bloc Nexus (קדנס 1s)
  - Champs: block_number, parent_hash, slot_time, tx_list (טרנזקציות atomiques cross-DS avec DSIDs touches), ds_artifacts[], nexus_qc.
  - פונקציה: לסיים עסקאות פחות אטומיים שאינם חפצי אמנות DS דורש אימות; פגשתי ב-Jour le Vecteur de roots DS du World State Global en Une Etape.

קונצנזוס ותזמון
- קונצנזוס Nexus שרשרת: צינור BFT עולמי (מחלקה Sumeragi) עם ועדת 22 נקודות (3f+1 עם f=7) ב-Blocs de 1s et une finalite 1s. Les membres du comite sont selectionnes par epochs via VRF/stake parmi ~200k מועמדים; הסיבוב שומר על ביזור והתנגדות א-לה.
- מרחב נתונים קונצנזוס: צ'ק DS בצע את ה-BFT הרצוי לפני אישורי אישורי חפצי אמנות לפי משבצת (preuves, התקשרויות DA, DS QC). Les comites lane-relay sond dimensionnes a `3f+1` and utilisant le parameter `fault_tolerance` du dataspace and sont echantillonnes de waye deterministe par epoch depuis le pool de validateurs du dataspace and utilisant la seed VRF liee a I1080070X. Les Private DS sont permissionnes; les Public DS permettent la liveness ouverte sous politiques anti-Sybil. Le comite Global nexus reste inchange.
- תזמון עסקאות: les utilisateurs soumettent des transactions atomiques declarant les DSIDs touches et les ensembles הרצאה/כתבה. Les DS executent en parallele dans le slot; הקשר בין החברות כולל את העסקה בגוש 1, ומספר חפצי אמנות DS אימות ואישורי DA sont a l'heure (<=300 אלפיות השנייה).
- בידוד ביצועים: צ'ק DS a des mempools et une execution independants. Les quotas par DS bornent combien de transactions touchant un DS peuvent etre commit par bloc pour eviter le head of-line blocking and protecter la latence des Private DS.Modele de Donnees ו-Namespace
- מזהים כשירים ל-DS: toutes les entites (domaines, comptes, actifs, roles) sont qualifies par `dsid`. דוגמה: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- הפניות גלובליות: אין התייחסות גלובלית ל-`(dsid, object_id, version_hint)` ו-Peut etre placee on-chain dans la couch nexus או dans desscripteurs AMX pour un use cross-DS.
- סדרה Norito: יש לך הודעות חוצות DS (תיאורים AMX, preuves) שימושיות לקודקים Norito. Pas de serde en production.

ניגודים אינטליגנטים ותוספות IVM
- הקשר לביצוע: ajouter `dsid` au contexte d'execute IVM. הניגודים Kotodama הם שירי ביצוע ב-Data Space ספציפי.
- פרימיטיבים אטומיים חוצי DS:
  - `amx_begin()` / `amx_commit()` מגביל את העסקה מרובת DS במארח IVM.
  - `amx_touch(dsid, key)` להצהיר על כוונה קריאה/כתיבה pour la detection de conflits contre les roots snapshot du slot.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> תוצאה (הרשאת הפעלה seulement si la politique l'autorise et si le handle est valide)
- טיפול בנכסים:
  - Les operations d'actifs sont autorisees par les politiques ISI/roles du DS; les frais sont payes en token de gas du DS. אסימוני היכולת אופציונליים ופוליטיקה פלוס עשירות (מרובה מאשרים, מגבלות תעריפים, גיאופנסינג) peuvent etre ajoutes plus tard sans changer le modele atomique.
- דטרמיניזם: toutes les nouvelles syscalls sont pures et deterministes donnees les entrees et les ensembles read/write AMX מצהיר. Pas d'effets caches de temps ou d'environnement.Preuves de validite post-quantiques (ISI מכליל)
- FASTPQ-ISI (PQ, sans trusted setup): א טיעון מבוסס-hash המאפשר להכליל את העברת העיצוב ל-FastPQ-ISI ל-Familles ISI toout and visant une preuve sos la seconde pour des batches a l'echelle 20k sur du hardware classe GPU.
  - תפעול פרופיל:
    - Les noeuds de production construisent le prover via `fastpq_prover::Prover::canonical`, qui initialize desormais toujours le backend de production; le mock deterministe a ete לפרוש. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) et `irohad --fastpq-execution-mode` קבוע aux operators de figer l'execution CPU/GPU de maniere deterministe tandis que l'observer hook enregistrer les triples demande/resolu/backend pour les audits de flotte. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/888.rs:8
- אריתמטיזציה:
  - KV-עדכון AIR: WSV אופייני לסוג מפתח-ערך מפה, צור קשר דרך Poseidon2-SMT. Chaque ISI s'etend un petit ensemble de lignes read-check-write sur des cles (comptes, actifs, roles, domaines, metadata, supply).
  - Contraintes gatees par opcode: une seule table AIR avec colonnes selecteurs impose des regles par ISI (שימור, מחשבים מונוטוניים, הרשאות, בדיקות טווח, mises a jour de metadata bornees).
  - טיעוני חיפוש: טבלאות שקופות עוסקות בפרטי הרשאות/תפקידים, דיוק פעולות ופרמטרים פוליטיים ברורים לנגד עיניים.
- אירוסין et mises a jour d'etat:
  - Preuve SMT מסכים: toutes les cles touchees (pre/post) sont prouvees contre `old_root`/`new_root` in utilisant un frontier compresse withec dedupes.
  - Invariants: invariants globaux (עמ' למשל, supply total par actif) sont imposes via egalite de multiensembles entre lignes d'effet et compteurs suivis.
- Systeme de preuve:
  - אירוסין פולינומי בסגנון FRI (DEEP-FRI) med forte arite (8/16) ו-blow-up 8-16; hashes Poseidon2; תמלול פיאט-שמיר avec SHA-2/3.
  - Recursion optionnelle: מקומי רקורסיבי צבירה DS יוצקים מדחס של מיקרו-אצטות ויחידה קודמת לפי חריץ זה הכרחי.
- Portee et exemples couverts:
  - אקטיבים: העברה, טביעה, צריבה, רישום/ביטול רישום הגדרות נכסים, הגדרת דיוק (נושא), הגדרת מטא נתונים.
  - Comptes/Domains: יצירה/הסרה, הגדר מפתח/סף, הוספה/הסרה של חותמים (את ייחודיות; les checks de signatures sont attestes par les validateurs DS, pas prouves dans l'AIR).
  - תפקידים/הרשאות (ISI): הענק/בטל תפקידים והרשאות; כופה באמצעות טבלאות חיפוש ובדיקות פוליטיות מונוטוניות.
  - ניגודים/AMX: מרקרים מתחילים/מבצעים AMX, יכולת נחוש/ביטול סי פעיל; prouves comme transitions d'etat et compteurs de politique.
- בודק hors AIR pour preserverval la latence:- חתימות והצפנה (עמ' דוגמה, חתימות משתמשות ML-DSA) הן מאומתות עם DS validateurs et attestees dans le DS QC; la preuve de validite couvre seulement la coherence d'etat et la conformite de politique. Cela garde des preuves PQ et rapides.
- אביזרי ביצועים (איורים, מעבד 32 ליבות + יחידת GPU מודרנית):
  - 20k ISI תערובות עם מפתח-מגע petit (<=8 cles/ISI): ~0.4-0.9 שניות קודמת, ~150-450 KB קדם, ~5-15 מילישניות אימות.
  - ISI plus lourdes (plus de cles/contraintes riches): מיקרו-אצווה (עמ' דוגמה, 10x2k) + רקורסיה יציקת גרדר <1 s par חריץ.
- Configuration du DS Manifest:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (חתימות מאומתות לפי DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (באופן חד משמעי; חלופות דומות להצהרה מפורשת)
- נפילות:
  - מתחמי ISI/מותאמים אישית משתמש ללא STARK כללי (`zk.policy = "stark_fri_general"`) עם הבדלים קודמים וסופיים 1 שניות באמצעות אישור QC + חיתוך על פסולים.
  - אפשרויות שאינן PQ (עמ' דוגמה, Plonk avec KZG) התקנה מהימנה דחופה ו-ne sont plus supportees dans le build par defaut.

מבוא AIR (מזוג Nexus)
- Trace d'execution: matrice avec largeur (קולונים דה רישום) et longueur (איטפס). Chaque ligne est une etape logique du traitement ISI; les colonnes contiennent valeurs pre/post, selecteurs et flags.
- מתנות:
  - Contraintes de transition: imposent des relations de ligne a ligne (עמ' למשל, post_balance = pre_balance - כמות pour une ligne debet quand `sel_transfer = 1`).
  - Contraintes de frontiere: שעבוד I/O public (old_root/new_root, compteurs) a la premiere/derniere ligne.
  - חיפושים/תמורות: אבטחת מראה ותמורות מרובות קונטרה של טבלאות עוסקות (הרשאות, פרמטרים של אקטיביים) ללא מעגלים ביטים.
- התקשרות ואימות:
  - Le prover engage les traces via des encodages hash et construit des polynomes de faible degre valides si les contraintets tiennent.
  - Le Verifier לאמת le faible degre באמצעות FRI (מבוסס גיבוב, פוסט-quantique) avec quelques ouvertures Merkle; le cout est logarithmique en etapes.
- דוגמה (העברה): הרישומים כוללים pre_balance, amount, post_balance, nonce et selecteurs. Les contraintetes imposent non-negativite/range, conservation et monotonicite de nonce, tandis qu'une multi-preuve SMT acgregee lie les feuilles pre/post aux roots old/new.Evolution ABI et syscalls (ABI v1)
- קורא ajouter (נומים ממחישים):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- סוג מצביע-ABI ajouter:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- פספוס ביקור דורש:
  - Ajouter a `ivm::syscalls::abi_syscall_list()` (garder l'ordre), gate par politique.
  - Mapper les numeros inconnus a `VMError::UnknownSyscall` למארחים.
  - בדיקות Mettre a jour les: רשימת syscall זהובה, ABI hash, מזהה מסוג מצביע זהב, מבחני מדיניות.
  - מסמכים: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

דגם סודי
- Contention des donnees privees: les corps de transaction, diffs d'etat ו-Snapshots WSV des private DS ne quittent jamais le sous-ensemble de validateurs prives.
- פרסום תערוכות: מציג כותרות, התקשרויות DA ותקפות PQ לאחר ייצוא.
- Preuves ZK optionnelles: les Private DS peuvent produire des preuves ZK (p. ex., solde suffisant, politique satisfaite) permettant des actions cross-DS sans reveler l'etat intern.
- Controle d'acces: l'autorisation est imposee par les politiques ISI/rolles dans le DS. אסימוני היכולת הם אופציונליים ופוטנציאלים אחרים בתוספת זמן.

בידוד ביצועים ו-QoS
- קונצנזוס, mempools ו-stockage מפרידים בין DS.
- מכסות של תזמון קשר ל-DS pour borner le temps d'inclusion des anchors and eviter le head of-line blocking.
- Budgets de resources de contrats par DS (מחשוב/זיכרון/IO), מטיל על מארח IVM. La טענה ציבורית DS ne peut pas consommer les budgets private-DS.
- Appels חוצה DS אסינכרונים ברורים les longues attentes synchrones dans l'execution private-DS.

חופשה במלאי ועיצוב מלאי
1) קוד חיסול
- Utiliser Reed-Solomon systematique (p. ex., GF(2^16)) pour l'effacement code au niveau blob des blocs Kura et snapshots WSV: parametres `(k, m)` avec `n = k + m` shards.
- Parametres par defaut (מציע, DS ציבורי): `k=32, m=16` (n=48), permettant la recuperation jusqu'a 16 shards perdus avec ~1.5x d'expansion. יוצקים DS פרטי: `k=16, m=8` (n=24) ברשות האנסמבל. Les deux sont configurables par DS Manifest.
- ציבורים של כתמים: רסיסים מחלקים דרך de nombreux noeuds DA/validateurs avec checks de disponibilite par sampling. Les engagements DA dans les headers permettent la Verification par light clients.
- Blobs prives: shards chiffres et מפיץ ייחודי parmi les validateurs פרטיים-DS (או מעצבים אפוטרופוסים). La chaine globale ne porte que des engagements DA (sans emplacement de shard ni cles).2) התקשרויות ודגימה
- יוצקים כתם צ'אקה: מחשבון une racine Merkle sur les shards et l'inclure dans `*_da_commitment`. Rester PQ en evitant les engagements a courbe elliptic.
- אישורי DA: מוכיחים אזורי אקנה לפורמט VRF (עמ' דוגמה, אזור 64) בעל אישור ML-DSA-87 מוכיח ודגימה מחדש של רסיסים. Objectif de latence d'attestation DA <=300 ms. Le comite nexus valide les certificats au lieu de tirer les shards.

3) אינטגרציה Kura
- Les blocks stockent les corps de transaction comme blobs a elimination code withec engagements Merkle.
- כותרות הכותרות מסמלות את ההתקשרויות דה כתם; les corps sont recuperables via le reseau DA pour public DS et via des canaux prives pour private DS.

4) אינטגרציה WSV
- Snapshots WSV: periodiquement, checkpoint de l'etat DS ו-snapshots chunkes וקודים למחיקה של התקשרויות שנרשמות בכותרות. הזן צילומי מצב, יומני שינוי בניהול. Les צילומי מצב ציבוריים sont largement shads; les snapshots prives restent dans les validateurs prives.
- Acces porteur de preuves: les contrats peuvent fournir (ou demander) des preuves d'etat (Merkle/Verkle) ancrees par des engagements de snapshot. Les Private DS peuvent fournir des attestations zero-knowledge au lieu de preuves brutes.

5) שמירה וגיזום
- Pas de pruning pour public DS: conserver tous les corps Kura ו-Snapshots WSV via DA (scalabilite horizontale). Les Private DS peuvent definier une rettention intern, mais les engagements מייצאת חומרי שמירה עצומים. La couche nexus conserve tous les blocs Nexus et les engagements d'artefacts DS.

Reseau et roles de noeuds
- Validateurs globaux: משתתף au consensus nexus, valident les blocs Nexus et les artefacts DS, effectuent des checks DA pour public DS.
- מרחב נתונים לאימות: executent le consensus DS, executent les contrats, gerent Kura/WSV local, gerent DA pour leur DS.
- Noeuds DA (optionnel): stockent/publient des blobs publics, facilitent le sampling. יוצקים DS פרטיים, les nouds DA sont co-localises with les validateurs ou des custodians de confiance.מערכת שיפורים ושיקולים
- רצף ניתוק/מפולג: מאמץ את mempool DAG (עמ' דוגמה, סגנון Narwhal) alimentant un BFT pipeline a la couche nexus pour reduire la latence and ameliorer le throughput sans changer le modele logique.
- מכסות DS והוגנות: מכסות DS par bloc et caps de poids pour eviter le head of line blocking and assurer une latence previsible pour private DS.
- אישור DS (PQ): les certificates de quorum DS utilisent ML-DSA-87 (classe Dilithium5) par defaut. EC'est post-quantique et plus volumineux que les signatures EC מקובל על משבצת QC par. Les DS peuvent explicitement opter pour ML-DSA-65/44 (plus petits) ou des signatures EC si declare dans le DS Manifest; les Public DS sont fortment מעודד גרדר ML-DSA-87.
- אישורי DA: יוצקים DS ציבוריים, משתמשי אישורים אזוריים של VRF qui emettent des certificats DA. Le comite nexus valide les certificats au lieu du sampling brut des shards; les Private DS gardent les attestations DA interns.
- Recursion et preuves par epoch: optionnellement aggreger plusieurs micro-batches dans un DS en une preuve recursive par slot/epoch pour garder taille de preuve et temps de validation stables sos charge elevee.
- קנה מידה של מסלולים (לפי הכרחי): יש לזהות ייחודיות עולמיות לגולוט, להציג נתיבי K של רצף מקבילים עם מיזוג קביעות. Cela לשמור על un order global unique tout en scalant horizontalement.
- תאוצה קובעת: ארבעת הגרעינים של SIMD/CUDA שערי תכונה ל-hashing/FFT עם מעבד חילופין סיביות מדויקת לשמר חומרה צולבת.
- Seuils d'activation des lanes (הצעה): activer 2-4 lanes si (a) la finalite p95 depasse 1.2 s pendant >3 minutes consecutives, ou (b) l'occupation par bloc depasse 85% pendant >5 minutes, ou (c) le debit entrant de tx deloc requerx de tx. soutenus. Les lanes bucketisent les transactions de maniere deterministe par hash de DSID et se merge dans le bloc nexus.

Frais et Economic (valeurs initiales)
- Unite de gas: token de gas par DS avec compute/IO metes; les frais sont payes en actif de gas natif du DS. La conversion entre DS est une conoccupation applicative.
- Priorite d'inclusion: round-robin entre DS avec quotas par DS pour preserver la fairness et les SLOs 1s; dans un DS, la mise en avant par fees peut departager.
- עתיד: un marche global de fees ou des politiques minimisant MEV peuvent etre בוחן sans changer l'atomicite ni le design de preuves PQ.זרימת עבודה בין נתונים-מרחב (דוגמה)
1) Un utilisateur soumet une transaction AMX touchant un public DS P et un private DS S: deplacer l'actif X de S vers le beneficiaire B dont le compte est dans P.
2) Dans le slot, P et S executent leur fragment contre le snapshot du slot. S לאמת l'autorisation et la disponibilite, פגשה בן יום ואינטרנט, et produit une preuve de validite PQ et un engagement DA (aucune donnee privee ne fuite). P להכין la mise a jour d'etat correspondante (עמ' דוגמה, נענע/צריבה/נעילה dans P selon la politique) et sa preuve.
3) Le comite nexus לאמת les deux preuves DS et les certificats DA; si les deux verifient dans le slot, la transaction est commit atomiquement dans le bloc Nexus de 1s, mettant a jour les deux roots DS dans le vecteur מדינת העולם העולמית.
4) Si une preuve ou un certificat DA est manquant/invalide, la עסקאות אבורט (aucun effet), ו-le client peut re-soumettre pour le slot suivant. Aucune donnee privee ne quitte S a aucun רגע.

- שיקולי אבטחה
- Execution deterministe: les syscalls IVM restent deterministes; התוצאות חוצות DS מכתיבות את התחייבות AMX וסופיות, מהן ההוריות או רזולוציית התזמון.
- בקרת גישה: הרשאות ISI ב-DS פרטיות רשומות qui peut soumettre des transactions and quelles operations sont autorisees. Les capability tokens encodent des droits fins pour שימוש cross-DS.
- סודי: chiffrement מקצה לקצה pour donnees פרטי-DS, רסיסי קוד מחיקה מלאי ייחודי parmi les membres autorises, preuves ZK optionnelles pour attestations externes.
- התנגדות DoS: בידוד au niveau mempool/consensus/stockage empeche la congestion publicque d'impacter la progression des private DS.

Changements des composants Iroha
- iroha_data_model: introduire `DataSpaceId`, תעודות זהות מתאימים ל-DS, מתארים AMX (הרצאות/הרצאות של הרכבים), סוגי קדם/התעסקות DA. סידורי ייחוד Norito.
- ivm: ajouter des syscalls et des types pointer-ABI pour AMX (`amx_begin`, `amx_commit`, `amx_touch`) et preuves DA; mettre a jour les tests/docs ABI selon la politique v1.