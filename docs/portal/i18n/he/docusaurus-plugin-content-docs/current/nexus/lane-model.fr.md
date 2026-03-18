---
lang: he
direction: rtl
source: docs/portal/docs/nexus/lane-model.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-lane-model
כותרת: Modele de lanes Nexus
תיאור: הלוגיקה הטקסונומית של הנתיבים, הגיאומטריה של התצורה וההתמזגות של המדינה העולמית ל- Sora Nexus.
---

# דגם הנתיבים Nexus ומחיצות WSV

> **סטטוס:** ניתן ל-NX-1 - טקסונומיה של מסלולים, גיאומטריה של תצורה ופריסה של מלאי לפני יישום.  
> **בעלים:** Nexus Core WG, Governance WG  
> **מפת דרכים:** NX-1 ב-`roadmap.md`

Cette page du portail reflete le brief canonique `docs/source/nexus_lanes.md` afin que les operators Sora Nexus, בעלים SDK et relecteurs puissent lire la guidance lanes sans explorer l arbre mono-repo. L architecture cible garde le world state deterministe tout en permettant aux data spaces (lanes) d executer des ensembles de validateurs publics ou prives avec des עומסי עבודה בודדים.

## מושגים

- **Lane:** לוגיקה של רסיס לפי חשבונות Nexus עם האנסמבל הנכון של validateurs ו-backlog d execution. זהה את הערך של `LaneId` יציב.
- **מרחב נתונים:** דלי ניהול מחדש של נתיבים או מסלולים נוספים המשותפים למדיניות של ציות, ניתוב ויישוב.
- **מניפסט הנתיב:** מטא-נתונים נשלטים לפי תוקף גורמי שלטון, DA Politique, Token de Gas, Regles de Settlement and Permissions de routing.
- **מחויבות גלובלית:** preuve emise par une lane qui resume les nouveaux roots d etat, les donnees de settlement et les transferts cross-lane optionnels. L anneau NPoS global ordonne les התחייבויות.

## טקסונומיה דה ליין

סוגי הנתיב מונעים את הראייה הקנונית, משטח השלטון וההתנחלות. הגיאומטריה של התצורה (`LaneConfig`) ללכוד את תכונות התכונות הבאות, SDKs ו-tooling puistender raisonner sur le layout sans logique sur mesure.

| סוג דה ליין | Visibilite | Membership des validateurs | תערוכה WSV | ממשל par defaut | פוליטיקה דה התנחלות | טיפוסי שימוש |
|----------------------------------------------------|
| `default_public` | ציבורי | ללא רשות (הימור גלובלי) | Replica d etat complete | SORA הפרלמנט | `xor_global` | Ledger public de base |
| `public_custom` | ציבורי | ללא רשות או מוגן יתד | Replica d etat complete | מודול pondere par stake | `xor_lane_weighted` | יישומים מפרסמים תפוקה עילית |
| `private_permissioned` | מוגבל | Ensemble fixe de validateurs (approuve par governance) | התחייבויות והוכחות | מועצה פדרציה | `xor_hosted_custody` | CBDC, עומסי עבודה בקונסורציום |
| `hybrid_confidential` | מוגבל | Membership mixte; enrobe des ZK הוכחות | התחייבויות + גילוי סלקטיבי | מודול דה מונה לתכנות | `xor_dual_fund` | Monnaie ניתן לתכנות שומר לפרטיות |

Tous les types de lane doivent מצהיר:- Alias ​​de dataspace - ארגון מחדש ליסible par humans liant les politiques de compliance.
- Handle de Governance - החלטת זיהוי באמצעות `Nexus.governance.modules`.
- Handle de Settlement - מזהה consomme par le settlement router pour debiter les buffers XOR.
- Metadata de telemetrie optionnelle (תיאור, איש קשר, דומיין עסקי) לחשוף באמצעות `/status` ולוחות מחוונים.

## Geometrie de configuration des lanes (`LaneConfig`)

`LaneConfig` נגזר זמן ריצה גיאומטרי מתוקף קטלוג הנתיבים. Il ne remplace pas les manifests de gouvernance; il fournit plutot des identifiants de stockage deterministes et des hints de telemetrie pour chaque lane configuree.

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` חשב מחדש את הגיאומטריה quand la configuration est chargee (`State::set_nexus`).
- Les aliases sont sanitises en slugs en minuscules; les caracteres non alphanumeriques consecutifs se compressent en `_`. זה מוצר כינוי לסרטון, ב-`lane{id}`.
- `shard_id` נגזר מפתח מטא-נתונים `da_shard_id` (באופן חד משמעי `lane_id`) ו-pilote le journal de curseur de shard מתמידים ב-relecture DA deterministe entre redemarrages/resharding.
- Les prefixes de cle garantissent que le WSV maintient des plages de cles par lane disjointes meme si le backend est partage.
- Les noms de segments Kura sont deterministes entre hosts; les auditeurs peuvent verifier les repertoires de segment et les manifests sans tooling sur mesure.
- Les segments merge (`lane_{id:03}_merge`) stockent les derniers roots de merge-hint et commitments d etat global pour cette lane.

## Partitionnement du state world- Le world state logique de Nexus est l union des espaces d etat par lane. Les lanes publiques מתמשך l etat complet; les lanes privees/confidential exportent des roots Merkle/commitment vers le merge book.
- Le stockage MV prefixe chaque cle avec le prefixe 4 octets de `LaneConfigEntry::key_prefix`, produisant des cles comme `[00 00 00 01] ++ PackedKey`.
- Les tables partagees (חשבונות, נכסים, טריגרים, רישום ניהול) stockent des entrees groupees par prefixe de lane, gardant les range scans deterministes.
- La metadata du merge-ledger reflete le meme layout: chaque lane ecrit des roots de merge-hint et des roots d etat global reduit dans `lane_{id:03}_merge`, permettant une retention ou viceviction ciblee quand une lane se retire.
- Les indexs cross-lane (כינויים של חשבונות, רישומי נכסים, מניפסטים של ממשל) מלאי קידומות דה ליין מפורש pour que les operateurs מיושם מהירות למנות ראשונות.
- **Politique de retention** - les lanes publiques conservent les block bodies complets; התחייבויות les lanes רק peuvent compacter les bodies anciens אפר-מחסום מכונית פחות התחייבויות sont autoritatifs. Les lanes confidential gardent des journals chiffres dans des segments dedies pour ne pas bloquer d autres עומסי עבודה.
- **כלים** - les utilitaires de maintenance (`kagami`, פקודות admin CLI) doivent referencer le namespace עם slug lors de l exposition des metriques, תוויות Prometheus או archivage des segments Kura.

## ניתוב וממשקי API

- נקודות הקצה Torii REST/gRPC מקובלות ללא אפשרות `lane_id`; l היעדר implique `lane_default`.
- ה-SDKs exposents des selecteurs de lane and mappt des aliases conviviaux vers `LaneId` en utilisant le catalog de lanes.
- Les regles de routing פתוח על תוקף הקטלוג ו-Peuvent Choisir Lane ו-dataspace. `LaneConfig` fournit des aliases friendly pour la telemetrie dans boards and logs.

## הסדר ושחרור

- Chaque lane paie des frais XOR au set global de validateurs. Les lanes peuvent collecter des tokens de gas natifs mais doivent escrow des equivalents XOR with les התחייבויות.
- Les proofs de settlement incluent le montant, la metadata de conversion et la preuve d escrow (לדוגמה, transfert vers le vault global de frais).
- נתב ההתיישבות מאחד (NX-3) מחייב את המאגרים ומשתמשים בקידומות ממים של הנתיב, ומיישרים את ה-Setlement Telemetrie de Settlement עם ה-Geometrie de stockage.

## ממשל

- Les lanes declarent leur module de governance via le catalogue. `LaneConfigEntry` porte l alias et le slug d origine pour garder la telemetrie et les audit trails lisibles.
- רישום Nexus מפיץ את נתיב מניפסטים הכוללים `LaneId`, מחייב את מרחב הנתונים, טיפול בניהול, טיפול בהסדר ומטא נתונים.
- המשך השדרוג בזמן ריצה ליישום פוליטיקה (`gov_upgrade_id` par defaut) ו-journalisent les diffs via le telemetry bridge (אירועים `nexus.config.diff`).## טלמטריה וסטטוס

- `/status` לחשוף את הכינויים של ליין, חיבורי מרחב נתונים, טיפול בניהול ופרופילים של יישוב, שואב את הקטלוג ואת `LaneConfig`.
- Les metriques du scheduler (`nexus_scheduler_lane_teu_*`) מזהים כינויים/שבלולים מתאימים למפה מהיר של צבר צבר ולחץ על TEU.
- `nexus_lane_configured_total` יש צורך לחשב מחדש את שינוי התצורה. La telemetrie emet des diffs signes quand la geometrie des lanes change.
- המדדים של מרחב הנתונים של צבר המידע כולל את הכינוי של מטא-נתונים/תיאור עבור מפעילים ומחבר לחיצה על הקובץ לתחום העסקים.

## תצורה וסוגי Norito

- `LaneCatalog`, `LaneConfig`, et `DataSpaceCatalog` vivent dans `iroha_data_model::nexus` et fournissent des structures au format Norito pour les manifests et SDKs.
- `LaneConfig` vit dans `iroha_config::parameters::actual::Nexus` et est derive automatiquement du catalogue; il ne requiert pas d encodage Norito car c est un helper runtime intern.
- La configuration cote utilisateur (`iroha_config::parameters::user::Nexus`) להמשיך d accepter desscripteurs declaratifs de lane and dataspace; לניתוח נגזרת תחזוקה לגיאומטריה ו-rejette les aliases invalides או les IDs de lane dupliques.

## חסין תנועה

- עדכונים שלם של נתב ההתנחלות (NX-3) עם גישה גיאומטרית חדשה לחיובים וחזרות לחצץ.
- ניהול כלי עזר מפורט יותר למשפחות עמודות, לגמלאים מצטמצמים של מסלולים ומפקח על יומני גושים בנתיב דרך מרחב השמות.
- מסיים את אלגוריתם המיזוג (הזמנה, גיזום, זיהוי קונפליקטים) ומחבר את תקנות הרגרסיה לשידור חוזר במסלול חוצה.
- Ajouter des hooks de compliance pour lists whitelists/blacklists and politiques de Monnaieable programable (suivi sos NX-12).

---

*המשך עמוד ההמשך למעקבים NX-1 ועבור NX-2 יופיע עם NX-18. Merci de remonter les question ouvertes dans `roadmap.md` ou le tracker de gouvernance afin que le portail reste aligne avec les docs canoniques.*