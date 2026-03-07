---
lang: he
direction: rtl
source: docs/portal/docs/sns/address-display-guidelines.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

לייבא את ExplorerAddressCard מ-'@site/src/components/ExplorerAddressCard';

:::הערה מקור קנוניק
Cette page reflete `docs/source/sns/address_display_guidelines.md` et sert
Maintenant de copie canonique du portail. Le fichier source reste pour les PRs
דה טראduction.
:::

Les portefeuilles, explorateurs and exemples de SDK דובר כתובות
de compte comme des payloads immuables. L'exemple de portefeuille קמעונאי אנדרואיד
ב-`examples/android/retail-wallet` מתחזק את דפוס UX דרישות:

- **Deux cibles de copie.** Fournissez deux boutons de copie מפורשים: IH58
  (עדיף) et la forme compressee Sora בלבד (`sora...`, בחירה שנייה). IH58 est toujours
  בטח a partager en externe et alimente le payload du QR. La variante compressee
  doit inclure un avertissement inline parce qu'elle ne fonctionne que dans des
  apps prises en charge par Sora. L'exemple Android branche les deux boutons Material et
  leurs tooltips dans
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, et la
  הדגמה iOS SwiftUI reflete le meme UX דרך `AddressPreviewCard` dans
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **מונוספייס, ניתן לבחירה טקסטים.** Affichez les deux chaines avec une police
  monospace et `textIsSelectable="true"` afin que les utilisateurs puissent
  inspecter les valeurs sans invoquer un IME. Evitez les champs editables: les
  IME peuvent reecrire le kana ou injecter des points de code אפס גדול יותר.
- **Indications de domaine par defaut implicite.** Quand le selecteur pointe sur
  le domaine implicite `default`, affichez une legende rappelant aux operators
  qu'aucun סיומת n'est requis. Les explorateurs doivent aussi mettre en avant
  le label de domaine canonique quand le selecteur encode un digest.
- **QR IH58.** קודי QR דואגים למקודד לרשת IH58. סי לה דור דו
  QR echoue, une erreur explicite au lieu d'une image video.
- **הודעה press-papiers.** Apres avoir copie la forme compressee, emettez un
  טוסט או מזנון חטיפים aux utilisateurs qu'elle est Sora-only et sujette
  a la distorsion IME.

Suivre ces garde-fous evite la corruption Unicode/IME et satisfait les criteres
d'acceptation du מפת הדרכים ADDR-6 pour l'UX portefeuille/explorateur.

## לכידת התייחסות

Utilisez les references suivantes lors des revues de localization pour garantir
que les libelles de boutons, tips tool and adertissements restent alignes entre
פלטפורמות:

- אנדרואיד עזר: `/img/sns/address_copy_android.svg`

  ![עתק אנדרואיד כפול עותק](/img/sns/address_copy_android.svg)

- התייחסות ל-iOS: `/img/sns/address_copy_ios.svg`

  ![עתק iOS כפול](/img/sns/address_copy_ios.svg)

## Helpers SDK

Chaque SDK לחשוף un helper de convenance qui retourne les formes IH58 et
compressee ainsi que la chaine d'avertissement pour que les couchs UI restent
קוהרנטיות:- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- מפקח JavaScript: `inspectAccountId(...)` retourne la chaine
  d'avertissement compressee et l'ajoute a `warnings` quand les appelants
  fournissent un literal `sora...`, pour que les explorateurs/tableaux de bord de
  portefeuille puissent afficher l'avertissement Sora-only תליון les flux de
  קולאז'/אימות plutot que seulement lorsqu'ils generent eux-memes la forme
  דחוס.
- פייתון: `AccountAddress.display_formats(network_prefix: int = 753)`
- סוויפט: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

Utilisez ces helpers au lieu de reimplementer la logique d'encoding dans les
ממשק המשתמש של ספות. העוזר JavaScript לחשוף אוסי ללא מטען `selector` dans
`domainSummary` (`tag`, `digest_hex`, `registry_id`, `label`) pour que les UIs
puissent indiquer si un selecteur est Local-12 ou adosse a un registre sans
reparser le payload brut.

## הדגמה של מכשירים לחקר

<ExplorerAddressCard />

החוקרים לא מתארים את העבודה
fait pour le portefeuille:

- Appliquez `data-copy-mode="ih58|compressed|qr"` aux boutons de copie afin que
  les front-ends puissent emettre des compteurs d'usage en parallele de la
  metrique Torii `torii_address_format_total`. Le composant demo ci-dessus
  envoie un evenement `iroha:address-copy` avec `{mode,timestamp}` - reliez cela
  a votre pipeline d'analytique/telemetrie (למשל שליח a Segment ou a un
  אספן בסיס על NORITO) pour que les לוחות מחוונים puissent correler l'usage
  de format d'adresse cote serverur avec les modes de copie cote לקוח. רפלטז
  aussi les compteurs de domaine Torii (`torii_address_domain_total{domain_kind}`)
  dans le meme flux pour que les revues de retrait Local-12 puissent exporter une
  preuve de 30 jours `domain_kind="local12"` directement depuis le tableau
  `address_ingest` de Grafana.
- Associez chaque control a des indications `aria-label`/`aria-describedby`
  distinctes qui expliquent si un literal est sur a partager (IH58) ou Sora-only
  (דחיסה). Incluez la legende de domaine implicite dans la description pour
  que les technologys d'assistance refletent le meme contexte que l'affichage.
- Exposez une region live (ex. `<output aria-live="polite">...</output>`) qui
  annonce les resultats de copie et les avertissements, en alignant le
  מכשיר VoiceOver/TalkBack עם כבלים לדוגמאות של Swift/Android.

Cette מכשור satisfait ADDR-6b en prouvant que les operators peuvent
observer a la fois l'ingestion Torii et les modes de copie client avant que les
selecteurs Local soient desactives.

## ערכת כלים להעברה מקומית -> גלובליתUtilisez le [ערכת כלים מקומית -> גלובלי](local-to-global-toolkit.md) pour
automatiser l'audit et la conversion des selecteurs תורשתי מקומי. לה עוזר
Emet a la fois le rapport d'audit JSON et la list convertie IH58/compressee que
les operateurs joignent aux tickets de readiness, tandis que le runbook associe
lie les לוחות מחוונים Grafana et les regles Alertmanager qui verrouillent le
cutover en mode strict.

## הפניה ל-Happy Du Layout Binaire (ADDR-1a)

Quand les SDK exposent un outillage avance d'adresse (בודקים, אינדיקציות de
אימות, constructeurs de manifest), pointez les developpeurs vers le format
לכידת wire canonique dans `docs/account_structure.md`. Le layout est toujours
`header · selector · controller`, או חלקי הכותרת הראשיים:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (סיביות 7-5) aujourd'hui; les valeurs non zero sont reservees
  et doivent מנוף `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` distingue les controlleurs simples (`0`) des multisig (`1`).
- `norm_version = 1` קידוד les regles de selecteur Norm v1. חוזים עתידיים של Les Norm
  reutiliseront le meme champ 2 ביטים.
- `ext_flag` vaut toujours `0`; les bits actifs indiquent des extensions de
  מטען ללא תשלום.

Le selecteur suit מיידיות הכותרת הראשית:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

ה-UIs ו-SDK מתחייבים לסוג הבורר:

- `0x00` = domaine par defaut implicit (sans payload).
- `0x01` = תקציר מקומי (12-בייט `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = entree de registre global (`registry_id:u32` big-endian).

דוגמאות hex canoniques que les outils de portefeuille peuvent lier ou integrer
aux docs/בדיקות:

| Type de selecteur | Hex canonique |
|--------------|--------------|
| Implicit par defaut | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| תקציר מקומי (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| הרשמה גלובלית (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Voir `docs/source/references/address_norm_v1.md` pour la table completed
selecteur/etat et `docs/account_structure.md` pour le diagramme complet des
בתים.

## Imposer les formes canoniques

Les operators qui convertissent les encodages local herites in IH58 canonique
או שרשראות דחיסות לא מצליחות לצעוד את זרימת העבודה CLI תיעוד של ADDR-5:

1. `iroha tools address inspect` Emet Maintenant un resume JSON structure avec IH58,
   compresse et des payloads hex canoniques. קורות החיים כוללים אוסי ואובייקט
   `domain` avec les champs `kind`/`warning` et reflete tout domaine fourni via
   le champ `input_domain`. Quand `kind` vaut `local12`, la CLI imprime un
   avertissement sur stderr et le resume JSON reflete la meme consigne pour que
   les pipelines CI et les SDK puissent l'afficher. Passez `legacy  suffix`
   lorsque vous vous voulez rejouer l'encodage converti sous la forme `<ih58>@<domain>`.
2. Les SDK peuvent afficher le meme avertissement/resume via le helper
   JavaScript:```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  Le helper prefix le prefixe IH58 detecte depuis le literal sauf si vous
  מפורש של fournissez `networkPrefix`, המשך קורות חיים pour des reseaux
  non defaut ne sont pas re-rendus silencieusement avec le prefixe par defaut.

3. Convertissez le payload canonique en reutilisant les champs `ih58.value` ou
   `compressed` אתה קורות חיים (אתה דורש קידוד אמיתי דרך `--format`). Ces
   chaines sont deja sures a partager en externe.
4. Mettez a jour les manifests, registers and documents orients client with la
   forme canonique et notifiez les contreparties que les selecteurs Seront מקומי
   מסרב une fois le cutover termine.
5. Pour les jeux de donnees בהמוניהם, executez
   `iroha tools address audit --input addresses.txt --network-prefix 753`. לה קומנדה
   lit des literaux separes par nouvelle ligne (les commentaires commencant par
   `#` מתעלם, ו-`--input -` או דגל אוקון מנצלים STDIN), emet un rapport
   JSON avec des resumes canoniques/IH58/compresse pour chaque entree, et compte
   les erreurs de parse ainsi que les avertissements de domaine Local. Utilisez
   `--allow-errors` lors de l'audit de dumps herites contenant des lignes
   טפילים, et bloquez l'automatisation via `strict CI post-check` lorsque les
   Operators sont prets a bloquer les selecteurs Local dans CI.
6. Quand vous avez besoin d'une reecriture ligne a ligne, utilisez
  Pour les feuilles de calcul de remediation des selecteurs Local, utilisez
  pour exporter un CSV `input,status,format,...` qui met en avant les encodages
  canoniques, avertissements et echecs de parse en une seule passe.
   Le helper ignore les lignes non Local par defaut, convertit chaque entree
   restante dans l'encodage demande (IH58/compresse/hex/JSON), et preserve le
   דומיין מקורי quand `legacy  suffix` משוער פעיל. Associez-le a
   `--allow-errors` יוצקים ממשיך לניתוח meme quand un dump contient des
   literaux mal formes.
7. L'automatisation CI/lint peut executer `ci/check_address_normalize.sh`, qui
   extrait les selecteurs Local de `fixtures/account/address_vectors.json`, les
   המרה באמצעות `iroha tools address normalize`, et rejoue
   `iroha tools address audit` pour prouver que les releases
   n'emetent plus de digests מקומי.`torii_address_local8_total{endpoint}` פלוס
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, et le tableau Grafana
`dashboards/grafana/address_ingest.json` קיימת גישה לאות יישום:
une fois que les לוחות מחוונים דה ייצור montrent zero soumissions מקומי
חוקיות ואפס התנגשויות תליון מקומי-12 30 יו"ר ברציפות, Torii
basculera la porte Local-8 en echec strict sur mainnet, suivi par Local-12 une
fois que les domaines globaux ont des entrees de registre correspondantes.
שקול למסך CLI comme l'avis operateur pour ce gel - la meme chaine
 d'avertissement est utilisee dans les tooltips SDK et l'automatization pour
maintenir la parite avec les criteres de sortie du מפת הדרכים. Torii להשתמש
que sur des clusters dev/test Lors du diagnostic de regressions. המשך א
miroiter `torii_address_domain_total{domain_kind}` ב-Grafana
(`dashboards/grafana/address_ingest.json`) pour que le pack de preuve ADDR-7
puisse montrer que `domain_kind="local12"` est reste a zero pendel la fenetre
Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) ajoute trois
מחסומים:

- `AddressLocal8Resurgence` page chaque fois qu'un contexte signale une nouvelle
  תוספת מקומית-8. Stoppez les rollouts en mode strict, localisez la
  משטח SDK אופייני בלוח המחוונים, ובכך, הגדרה זמנית
  defaut (`true`).
- `AddressLocal12Collision` se declenche lorsque deux תוויות Local-12 hashent
  vers le meme digest. Mettez en pause les promotions de manifest, executez le
  ערכת כלים מקומית -> גלובלית לבדיקת מיפוי של תקצירים ותיאום
  avec la governance Nexus avant de reemettre l'entree de registre ou de
  reenclencher les rollouts en aval.
- `AddressInvalidRatioSlo` נמנע lorsque le ratio invalide a l'echelle de la
  יופי (בכלל דחיות מקומיות-8/מצב קפדני) יורד ל-SLO ב-0.1%
  תליון דיקס דקות. Utilisez `torii_address_invalid_total` pour identifier le
  Contexte/Raison Responsable et coordonnez avec l'equipe SDK proprietaire avant
  de reenclencher le mode strict.

### Extrait de note de release (portefeuille et explorateur)

כלול את כדורי הכדורים בהערות השחרור/חוקר
lors du cutover:

> **כתובות:** Ajoute le helper `iroha tools address normalize`
> et l'a branche dans CI (`ci/check_address_normalize.sh`) pour que les pipelines
> portefeuille/explorateur puissent convertir les selecteurs מורשת מקומית vers
> des formes canoniques IH58/compressees avant que Local-8/Local-12 soient
> Bloques sur mainnet. Mettez a jour les exports personnalises pour executer la
> commande et joindre la list normalisee au bundle de preuve de release.