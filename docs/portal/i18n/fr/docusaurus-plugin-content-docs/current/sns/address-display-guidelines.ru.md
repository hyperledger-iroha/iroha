---
lang: fr
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importer ExplorerAddressCard depuis '@site/src/components/ExplorerAddressCard' ;

:::note Канонический источник
Cette page indique `docs/source/sns/address_display_guidelines.md` et prend en charge
служит канонической копией портала. Il s'agit d'un rendez-vous pour les relations publiques.
:::

Les kits, les applications et les premiers SDK sont également disponibles pour les comptes d'adresses.
к неизменяемым charge utile. Exemple de module Android
`examples/android/retail-wallet` vous permet de démonter le modèle UX :- **Две цели копирования.** Précédent deux boutons de copie : I105
  (Pредпочтительно) et le formulaire Sora uniquement (`sora...`, juste en avant-première). I105 est totalement gratuit
  Supprimez-le et utilisez-le dans la charge utile QR. Cette forme doit être sélectionnée
  Il est généralement prévu que vous puissiez travailler uniquement dans les applications prenant en charge Sora.
  L'exemple Android inclut les boutons Matériel et les info-bulles dans
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, un
  La démo iOS SwiftUI fournit l'UX à partir de `AddressPreviewCard` dans
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Моноширинный, выбираемый текст.** Отрисовывайте обе строки моноширинным
  et avec `textIsSelectable="true"`, que vous pouvez prouver
  значения без вызова IME. Избегайте редактируемых полей: IME может переписать
  kana ou внедрить нулевой ширины кодовые точки.
- **Подсказки для неявного домена по умолчанию.** Le sélecteur s'ouvre sur
  Nouvelle maison `default`, veuillez contacter l'opérateur, c'est-à-dire
  суффикс не требуется. Les personnes qui vous intéressent doivent choisir un endroit canonique
  ярлык, когда селектор кодирует digest.
- **QR pour I105.** Les QR-codes doivent coder le bouton I105. Comment générer le QR
  провалилась, покажите явную ошибку вместо пустого изображения.
- **Сообщение буфера обмена.** После копирования сжатой отправьте toast
  ou un snack-bar, un restaurant populaire, qui est sur Sora-only et подвержена
  искажению IME.L'installation de ces garde-corps prévient l'utilisation d'Unicode/IME et la conversion
Les critères de sélection des cartes ADDR-6 pour les équipements/départs UX.

## Écrans pour les serviettes

Utilisez les configurations appropriées pour la localisation des fournisseurs, ce qui est le cas
Boutons, info-bulles et pré-installation des plates-formes :

- Code Android : `/img/sns/address_copy_android.svg`

  ![Copie Android pour Android](/img/sns/address_copy_android.svg)

- Nom iOS : `/img/sns/address_copy_ios.svg`

  ![Copie iOS pour iOS](/img/sns/address_copy_ios.svg)

## SDK vidéo

Le SDK fournit un assistant supplémentaire, qui prend en charge les formulaires I105 et
En prélude à cette action, les liens UI s'installent :

- Javascript : `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspecteur JavaScript : `inspectAccountId(...)` permet de prévisualiser
  C'est une forme et je l'utilise dans `warnings`, alors vous préférerez le littéral
  `sora...`, les panneaux/panneaux de protection peuvent être utilisés avant la mise à jour
  Sora-only est pour les options/valorisations, mais pas seulement pour les formes de génération.
-Python : `AccountAddress.display_formats(network_prefix: int = 753)`
-Swift : `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin : `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)Utilisez ces assistants pour permettre l'encodage des logiciels dans les interfaces utilisateur.
L'assistant JavaScript permet de récupérer la charge utile `selector` et `domainSummary`.
(`tag`, `digest_hex`, `registry_id`, `label`), les interfaces utilisateur peuvent être téléchargées
Si vous utilisez le sélecteur Local-12 ou le registre, vous ne pouvez pas utiliser la charge utile automatiquement.

## Démonstration des instruments utilisés



Les utilisateurs doivent utiliser le téléphone et installer le téléphone :- Ajoutez `data-copy-mode="i105|i105_default|qr"` aux boutons de copie, qui sont
  Les frontends peuvent émettre des signaux en utilisant une fréquence métrique Torii
  `torii_address_format_total`. Le composant de démonstration doit être utilisé
  associer `iroha:address-copy` à `{mode,timestamp}` - vous connecter à votre compte
  analyse/télémétrie du réseau (par exemple, dans le segment ou NORITO
  collecteur), les tableaux de bord peuvent corréler l'utilisation des formats
  Adresses du serveur avec les clients de copie. Также зеркальте счетчики
  les maisons Torii (`torii_address_domain_total{domain_kind}`) pour que vous puissiez les voir
  livraisons locales-12 mois d'exportation 30 jours de documentation
  `domain_kind="local12"` pour les panneaux Grafana `address_ingest`.
- Сопоставляйте каждому контролу отдельные `aria-label`/`aria-describedby`,
  объясняющие, безопасен ли littéral для обмена (I105) ou только для Sora
  (сжатый). Cliquez sur le nouveau domaine dans la description de ce que vous avez prévu
  Les technologies sont adaptées à tout le contexte.
- Utilisez la région live (par exemple, `<output aria-live="polite">...</output>`),
  comment obtenir les résultats de la copie et de la préparation, en fonction de
  Utilisez VoiceOver/TalkBack pour utiliser les modèles Swift/Android.

Cet instrument définit l'ADDR-6b, ce qui signifie que les opérateurs peuvent
Ouvrir et saisir Torii, et procéder à la copie du client pour l'ouverture Local
sélecteurs.

## Набор миграции Local -> GlobalUtiliser [Local -> Global Toolkit](local-to-global-toolkit.md) pour
L'audit automatique et la conversion sont effectués par des sélecteurs locaux. Helper выводит и
JSON-obtient l'audio et convertit le code I105/сжатых значений, который
les opérateurs fournissent des tickets de préparation et un runbook client
Les panneaux Grafana et Alertmanager permettent de gérer le basculement strict.

## Lecteurs binaires de haut niveau (ADDR-1a)

Le SDK fournit des adresses d'instruments (inspecteurs, inspecteurs)
валидации, manifestes constructeurs), направляйте разработчиков к каноническому
Forme de fil `docs/account_structure.md`. Раскладка всегда
`header · selector · controller`, voici les bits d'en-tête suivants :

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) deuxième ; ненулевые значения зарезервированы и
  veuillez utiliser `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` permet de contrôler un seul (`0`) et un multisig (`1`).
- `norm_version = 1` correspond au sélecteur Norm v1. Будущие нормы будут
  переиспользовать то же 2-битовое поле.
- `ext_flag` par rapport à `0` ; Les bits usagés ne sont pas disponibles
  charge utile.

Le sélecteur s'ouvre après l'en-tête :

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

L'interface utilisateur et le SDK permettent de sélectionner le type de sélecteur :- `0x00` = неявный домен по умолчанию (sans charge utile).
- `0x01` = résumé local (`blake2s_mac("SORA-LOCAL-K:v1", label)` 12 octets).
- `0x02` = запись глобального registre (`registry_id:u32` big-endian).

Les principes hexadécimaux canoniques, les instruments que les cocherkov peuvent utiliser ou
встраивать в docs/tests :

| Sélection de type | Hex canonique |
|--------------------|---------------|
| Nouveau projet | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Résumé local (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Registre mondial (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

См. `docs/source/references/address_norm_v1.md` pour le sélecteur/état des tables principales
et `docs/account_structure.md` pour le diagramme du batteur.

## Formulaire canonique

Opérateurs de conversion de codes locaux en canonique I105 ou
Si vous utilisez des touches, vous devez simplement sélectionner le fichier CLI depuis ADDR-5 :

1. `iroha tools address inspect` vous permet de créer une structure JSON avec I105,
   charge utile hexagonale compressée et canonique. Резюме также включает объект `domain`
   Avec les polymères `kind`/`warning` et l'amour pour la maison précédente
   `input_domain`. Lorsque `kind` s'ouvre sur `local12`, la CLI est disponible avant la mise à jour
   stderr, la réponse JSON vous permet de télécharger les cartes CI et les SDK
   ее показывать. Prenez le `legacy  suffix`, si vous êtes intéressé par votre projet.
   codage de conversion selon `<i105>@<domain>`.
2. Le SDK peut afficher l'avertissement/résumé de l'assistant JavaScript :```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.i105.value, summary.i105Warning);
   ```
  Helper сохраняет I105 префикс, извлеченный из littéral, если только вы явно не
  Si vous utilisez le `networkPrefix`, la reprise n'est pas effectuée par défaut et n'est pas configurée par défaut.
  тихо с дефолтным префиксом.3. Convertissez la charge utile canonique en utilisant `i105.value` ou
   `i105_default` à partir du résumé (ou à utiliser le codage suivant pour `--format`). Eti
   Les coups sont parfaits pour tout le monde.
4. Ouvrir les manifestes, les registres et les documents clients sous forme canonique et
   Vous pouvez également activer les sélecteurs locaux après le basculement.
5. Pour les plus grands emplois du temps
   `iroha tools address audit --input addresses.txt --network-prefix 753`. Commanda
   читает littéraux, разделенные переводом строки (комментарии с `#` игнорируются,
   (`--input -` ou le drapeau standard utilisant STDIN), vous pouvez utiliser JSON
   canoniques/I105/sжатыми резюме для каждой записи и считает ошибки парсинга
   и предупреждения Local домена. Utilisez `--allow-errors` pour auditer
   `strict CI post-check`, les opérateurs doivent sélectionner les sélecteurs locaux dans CI.
6. Quand il faut commencer, utilisez
  Pour l'assainissement des tables, les sélectionneurs locaux utilisent
  Pour l'exportation CSV `input,status,format,...`, le contenu est canonique
  codes, pré-commandes et analyses possibles pour la procédure à suivre.
   Helper по умолчанию пропускает не-Local строки, конвертирует каждую оставшуюся
   Utilisez l'encodage automatique (I105/сжатый/hex/JSON) et votre domaine d'activité
   par `legacy  suffix`. Connectez-vous avec `--allow-errors` pour effectuer une analyse à partir de maintenant
   если dump содержит поврежденные littéraux.7. L'automatisation CI/lint peut utiliser `ci/check_address_normalize.sh`, ici
   Utilisez les sélecteurs locaux `fixtures/account/address_vectors.json`,
   convertissez-le en `iroha tools address normalize` et installez-le automatiquement
   `iroha tools address audit`, que vous avez téléchargé, que les réponses ne sont pas grandes
   эмитят Résumés locaux.

`torii_address_local8_total{endpoint}` est à votre disposition
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}` et panneau Grafana
`dashboards/grafana/address_ingest.json` дают сигнал application: когда
Les entreprises du secteur local ouvrent le droit local et le numéro Local-12
Lors d'une collision technique d'une durée de 30 jours, Torii a dépassé la porte Local-8 en cas de panne matérielle
mainnet, le site Local-12, ainsi que les domaines mondiaux sont actuellement disponibles
registre. Veuillez consulter l'opérateur CLI pour résoudre ce problème -
Cette étape consiste à utiliser les info-bulles et l'automatisation du SDK.
сохранять паритет с критериями выхода дорожной карты. Torii est prêt à être utilisé
кластерах при диагностике регрессий. Продолжайте зеркалировать
`torii_address_domain_total{domain_kind}` à Grafana
(`dashboards/grafana/address_ingest.json`), ce paquet contient des documents ADDR-7
Je pense que le `domain_kind="local12"` est uniquement disponible en technologie
Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) est disponible trois fois
garde-corps :- `AddressLocal8Resurgence` s'applique, ce contexte est lié à l'augmentation la plus récente
  Local-8. Installez les déploiements en mode strict, recherchez le SDK problématique dans votre pays et,
  Le signal s'affiche à l'intérieur de l'écran (`true`).
- `AddressLocal12Collision` est disponible pour le local-12 local
  один digérer. Приостановите promotions manifestes, запустите Local -> Global
  boîte à outils pour les résumés de cartographie de l'audit et la coordination avec la gouvernance Nexus avant
  Entrez l'entrée de registre ou activez les déploiements en aval.
- `AddressInvalidRatioSlo` prévient, alors que le rapport invalide (par exemple
  отказы Local-8/strict-mode) prévient SLO 0,1% dans la technologie actuelle. Utiliser
  `torii_address_invalid_total` pour la prise en charge du contexte/du produit et
  Essayez d'utiliser le SDK avant d'activer le mode strict.

### Fragment релизной заметки (кошелек и обозреватель)

Cliquez sur Bullet dans les notes de version pour le basculement :

> **Adresse :** Ajout de l'assistant `iroha tools address normalize`
> и подключен в CI (`ci/check_address_normalize.sh`), чтобы пайплайны кошелька/
> Vous pouvez utiliser les sélecteurs locaux dans les canaux canoniques
> I105/Formulaires de blocage Local-8/Local-12 sur le réseau principal. Обновите любые
> Exportations douanières, qui commandent et normalisent
> список к paquet de preuves реLISа.