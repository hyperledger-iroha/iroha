---
lang: fr
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importer ExplorerAddressCard depuis '@site/src/components/ExplorerAddressCard' ;

:::note مستند ماخذ
یہ صفحہ `docs/source/sns/address_display_guidelines.md` کی عکاسی کرتا ہے اور اب
پورٹل کی کینونیکل کاپی ہے۔ سورس فائل ترجمہ PRs کے لئے برقرار رہتی ہے۔
:::

Il s'agit d'un SDK qui prend en charge la charge utile et la charge utile.
Android کی ریٹیل والٹ مثال
`examples/android/retail-wallet` UX UX est en charge:- **دو کاپی اہداف۔** دو واضح کاپی بٹنز دیں: IH58 (ترجیحی) et Sora والا
  کمپریسڈ فارم (`sora...`, deuxième meilleur). IH58 ہمیشہ بیرونی شیئرنگ کے لئے محفوظ ہے اور QR
  charge utile بناتا ہے۔ Le système de connexion en ligne est également disponible pour les utilisateurs en ligne.
  Sora-aware est une solution pour vous Android مثال دونوں Material بٹنز اور ٹول ٹپس
  par `examples/android/retail-wallet/src/main/res/layout/activity_main.xml` میں
  Application iOS SwiftUI `examples/ios/NoritoDemo/Sources/ContentView.swift`
  Le modèle `AddressPreviewCard` est doté d'une interface utilisateur UX
- **Monospace, قابل انتخاب متن۔** Chaînes de caractères et monospace فونٹ اور
  `textIsSelectable="true"` L'interface utilisateur de l'IME est disponible
  دیکھ سکیں۔ Type de fichier : IME kana est un code de largeur nulle
  کوڈ پوائنٹس داخل کر سکتا ہے۔
- **غیر ضمنی ڈیفالٹ ڈومین کے اشارے۔** جب selector ضمنی `default` ڈومین کی طرف
  ہو تو ایک légende دکھائیں جو آپریٹرز کو یاد دلائے کہ suffixe درکار نہیں۔
  Vous pouvez utiliser un sélecteur canonique pour sélectionner un sélecteur
  digérer encoder کرے۔
- ** Charges utiles IH58 QR ** QR encodage de chaîne IH58 encodage en ligne اگر QR
  génération ناکام ہو تو خالی تصویر کے بجائے واضح erreur دکھائیں۔
- ** کلپ بورڈ پیغام۔** کمپریسڈ فارم کاپی کرنے کے by toasts یا snack-bar دکھائیں
  جو صارفین کو یاد دلائے ہو سکتا ہے۔ Sora et IME

Il s'agit d'une corruption Unicode/IME et d'une application UX/UX.
Acceptation de la feuille de route ADDR-6

## اسکرین شاٹ فکسچرزلوکلائزیشن ریویوز کے دوران درج ذیل فکسچرز استعمال کریں تاکہ بٹن لیبلز، ٹول ٹپس
اور وارننگز پلیٹ فارمز کے درمیان ہم آہنگ رہیں:

- Version Android : `/img/sns/address_copy_android.svg`

  ![Android کاپی ریفرنس](/img/sns/address_copy_android.svg)

- Version iOS : `/img/sns/address_copy_ios.svg`

  ![iOS کاپی ریفرنس](/img/sns/address_copy_ios.svg)

## Assistants du SDK

Le SDK est un outil d'aide pour le SDK et IH58 est un outil de développement rapide.
string دیتا ہے تاکہ UI لیئرز مستقل رہیں:

- Javascript : `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspecteur JavaScript : `inspectAccountId(...)` chaîne de caractères en anglais
  اور اسے `warnings` میں شامل کرتا ہے جب کالرز `sora...` littéral دیں، تاکہ والٹ/
  La méthode de collage/validation est utilisée pour Sora uniquement et la méthode de collage est terminée.
  صرف تب جب وہ کمپریسڈ فارم خود بنائیں۔
-Python : `AccountAddress.display_formats(network_prefix: int = 753)`
-Swift : `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin : `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

Les helpers sont des outils d'interface utilisateur qui encodent les fichiers pour les utilisateurs. Javascript
assistant `domainSummary` et charge utile `selector` (`tag`, `digest_hex`, `registry_id`,
`label`) Utilisez un sélecteur d'interface utilisateur Local-12 et un sélecteur Local-12
Il s'agit d'analyser la charge utile brute

## ایکسپلورر instrumentation ڈیمو



Il y a aussi la télémétrie et l'accessibilité et le miroir:- کاپی بٹنز پر `data-copy-mode="ih58|compressed (`sora`)|qr"` لگائیں تاکہ فرنٹ اینڈز Torii
  Il s'agit des compteurs d'utilisation `torii_address_format_total`. اوپر والا
  `{mode,timestamp}` et `iroha:address-copy` sont en vente libre.
  Pipeline d'analyse/télémétrie (collecteur soutenu par NORITO)
  جوڑیں تاکہ tableaux de bord سرور سائیڈ format d'adresse استعمال اور کلائنٹ کاپی
  موڈز کو corréler کر سکیں۔ Compteurs de domaine Torii
  (`torii_address_domain_total{domain_kind}`) Il s'agit d'un local-12 local
  ریٹائرمنٹ ریویوز `address_ingest` Grafana pour 30 jours
  `domain_kind="local12"` pour le client
- ہر کنٹرول کے لئے الگ `aria-label`/`aria-describedby` ہنٹس دیں جو بتائیں کہ
  littéral شیئر کرنے کے لئے محفوظ ہے (IH58) یا صرف Sora (کمپریسڈ)۔ ضمنی ڈومین
  légende et description description de la technologie d'assistance et de la technologie d'assistance
  جو بصری طور پر نظر آتا ہے۔
- Région en direct de la région (مثلاً `<output aria-live="polite">...</output>`) رکھیں جو کاپی
  Les versions plus récentes de Swift/Android sont disponibles avec VoiceOver/TalkBack
  رویے کے مطابق۔

Instrumentation ADDR-6b pour les appareils électroniques et les appareils électroniques Local
sélecteurs pour l'ingestion Torii et les modes de copie côté client
دونوں کا مشاہدہ کر سکتے ہیں۔

## Local -> Boîte à outils de migration globaleSélecteurs locaux pour audit et conversion assistant JSON audit رپورٹ اور
IH58/کمپریسڈ لسٹ دونوں بناتا ہے جنہیں آپریٹرز tickets de préparation کے ساتھ منسلک
Il s'agit d'un runbook Grafana pour les tableaux de bord et d'Alertmanager.
Il s'agit d'un basculement en mode strict et d'un portail.

## Mise en page de mise en page en anglais (ADDR-1a)

Outils d'adressage avancés des SDK (inspecteurs, conseils de validation, générateurs de manifestes)
Les développeurs utilisent `docs/account_structure.md` pour le fil canonique
فارمیٹ کی طرف بھیجیں۔ layout ہمیشہ `header · selector · controller` ہوتا ہے، جہاں
bits d'en-tête یہ ہیں :

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) موجودہ ہے؛ non nul اقدار ریزرو ہیں اور
  `AccountAddressError::InvalidHeaderVersion` اٹھانی چاہئیں۔
- Contrôleurs `addr_class` simples (`0`) et multisig (`1`) pour les contrôleurs
- Sélecteur `norm_version = 1` Norm v1 pour encoder les fichiers مستقبل کے normes اسی
  2 bits pour les cartes de crédit
- `ext_flag` pour `0` pour Les bits et les extensions de charge utile sont disponibles

le sélecteur et l'en-tête sont ci-dessous :

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

Les interfaces utilisateur et les SDK et le sélecteur sont des outils de sélection :

- `0x00` = ضمنی ڈیفالٹ ڈومین (کوئی payload نہیں)۔
- `0x01` = résumé لوکل (`blake2s_mac("SORA-LOCAL-K:v1", label)` 12 octets).
- `0x02` = گلوبل رجسٹری انٹری (`registry_id:u32` big-endian).

کینونیکل hex مثالیں جنہیں والٹ ٹولنگ docs/tests میں لنک یا embed کر سکتی ہے:| Sélecteur | Hex canonique |
|--------------------|---------------|
| ضمنی ڈیفالٹ | `0x02000001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| لوکل résumé (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| گلوبل رجسٹری (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

مکمل sélecteur/état ٹیبل کے لئے `docs/source/references/address_norm_v1.md` اور
Diagramme d'octets pour `docs/account_structure.md`

## Formes canoniques نافذ کرنا

Voici comment ADDR-5 fonctionne avec le flux de travail CLI :

1. `iroha tools address inspect` pour IH58, pour les charges utiles hexadécimales canoniques et pour les charges utiles hexadécimales canoniques.
   résumé JSON structuré ici résumé `kind`/`warning` et `domain`
   Le lecteur `input_domain` est en train de lire l'écho
   کرتا ہے۔ `kind` `local12` et CLI stderr sont disponibles en JSON
   résumé et détails des pipelines CI et des SDK en surface
   Vous devez convertir le codage en `<ih58>@<domain>` et utiliser la relecture.
   Il s'agit de `--append-domain` دیں۔
2. SDK pour l'assistance/résumé et l'assistant JavaScript pour la description :

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed (`sora`));
   ```
  littéral d'assistance سے détecter کیا گیا Préfixe IH58 محفوظ رکھتا ہے جب تک آپ
  `networkPrefix` واضح طور پر فراہم نہ کریں؛ اس لئے réseaux non par défaut کے
  résumés خاموشی سے préfixe par défaut کے ساتھ دوبارہ rendu نہیں ہوتے۔3. charge utile canonique `ih58.value` et `compressed (`sora`)` pour la réutilisation
   کریں (یا `--format` کے ذریعے دوسری encodage مانگیں)۔ یہ chaînes پہلے سے
   بیرونی شیئرنگ کے لئے محفوظ ہیں۔
4. manifestes, registres et documents destinés aux clients et documents canoniques
   Les sélecteurs locaux sont disponibles pour le cutover et les sélecteurs locaux.
   ہوں گے۔
5. بلک ڈیٹا سیٹس کے لئے
   `iroha tools address audit --input addresses.txt --network-prefix 753` چلائیں۔ کمانڈ
   littéraux séparés par une nouvelle ligne پڑھتی ہے ( `#` سے شروع ہونے والے commentaires نظرانداز
   Il s'agit d'un `--input -` qui correspond à la norme STDIN) ، ہر
   Voici les résumés canoniques/IH58 (ترجیحی)/compressés (`sora`) (`sora`, deuxième meilleur) pour les résumés JSON رپورٹ بناتی
   rangées comme `--allow-errors` pour les sélecteurs locaux et les sélecteurs locaux
   CI میں بلاک کرنے کے لئے تیار ہوں تو `--fail-on-warning` آٹومیشن گیٹ کریں۔
6. Comment réécrire une nouvelle ligne à une nouvelle ligne
  Feuilles de calcul de correction du sélecteur local
  Il s'agit d'un fichier CSV `input,status,format,...` pour les encodages canoniques.
  avertissements et échecs d'analyse assistant ڈیفالٹ طور پر
  les lignes non locales contiennent des entrées et des entrées encodées (IH58 ترجیحی/compressé (`sora`) deuxième meilleur/hex/JSON)
  میں بدلتا ہے، اور `--append-domain` پر اصل ڈومین محفوظ رکھتا ہے۔ `--allow-errors`کے ساتھ جوڑیں تاکہ خراب littéraux et dumps پر بھی scan جاری رہے۔
7. Automatisation CI/lint `ci/check_address_normalize.sh` pour plus de détails
   `fixtures/account/address_vectors.json` سے Sélecteurs locaux نکال کر
   `iroha tools address normalize` pour le paiement en ligne
   `iroha tools address audit --fail-on-warning` دوبارہ چلاتی ہے تاکہ ثابت ہو کہ
   publie اب Résumés locaux نہیں نکالتے۔

`torii_address_local8_total{endpoint}` en cours
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
Carte `torii_address_collision_domain_total{endpoint,domain}`, ou Grafana
Signal d'application `dashboards/grafana/address_ingest.json` دیتے ہیں: جب
tableaux de bord de production مسلسل 30 دن تک صفر legit Soumissions locales اور صفر
Collisions locales 12 avec Torii Porte Local-8 pour le réseau principal en cas de défaillance matérielle
Il s'agit de Local-12 et de domaines globaux correspondant aux entrées de registre correspondantes.
Geler la sortie CLI et le message face à l'opérateur - et la chaîne d'avertissement
Info-bulles du SDK sur l'automatisation et les critères de sortie de la feuille de route
ہے؛ les régressions diagnostiquent les clusters de développement/test et `false`
`torii_address_domain_total{domain_kind}` et Grafana (`dashboards/grafana/address_ingest.json`)
Miroir miroir pour le pack de preuves ADDR-7
sélecteurs et désactiver Pack Gestionnaire d'alertes
(`dashboards/alerts/address_ingest_rules.yml`) Pour les garde-corps, il s'agit de :- `AddressLocal8Resurgence` pour la page et le contexte Local-8
  incrément رپورٹ کرے۔ déploiements en mode strict, tableau de bord et SDK offensant
  Il s'agit d'un signal par défaut ou d'un signal par défaut (`true`) par défaut
- `AddressLocal12Collision` est un résumé des étiquettes Local-12
  پر hash ہوں۔ manifeste promotions روکیں، Local -> Global toolkit چلا کر digest
  cartographie de la gouvernance et des coordonnées Nexus coordination des coordonnées
  entrée de registre دوبارہ جاری ہو یا déploiements en aval بحال ہوں۔
- `AddressInvalidRatioSlo` خبردار کرتا ہے جب taux d'invalidité à l'échelle de la flotte (Local-8/
  rejets en mode strict (par exemple) 10 millions d'euros 0,1 % SLO par rapport à la moyenne
  `torii_address_invalid_total` استعمال کر کے متعلقہ contexte/raison کی نشاندہی
  Vous possédez un SDK qui possède des coordonnées et un mode strict.

### ریلیز نوٹ اسنیپٹ (والٹ اور ایکسپلورر)

cutover کے وقت والٹ/ایکسپلورر ریلیز نوٹس میں درج ذیل bullet شامل کریں:

> **Adresses :** `iroha tools address normalize --only-local --append-domain` helper شامل
> کیا گیا اور اسے CI (`ci/check_address_normalize.sh`) میں وائر کیا گیا تاکہ
> Un réseau principal local-8/Local-12 est disponible sur le réseau principal Local-8/Local-12. کسی بھی
> exportations personnalisées comme une liste normalisée
> publier un ensemble de preuves کے ساتھ منسلک کریں۔