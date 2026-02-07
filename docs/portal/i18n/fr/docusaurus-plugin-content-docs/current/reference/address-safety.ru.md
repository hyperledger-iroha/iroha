---
lang: fr
direction: ltr
source: docs/portal/docs/reference/address-safety.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Безопасность и доступность адресов
description : UX-TреBOвания for безопасного отображения и передачи адресов Iroha (ADDR-6c).
---

Cette page de fixation du document ADDR-6c. Veuillez indiquer cette organisation du navigateur, des explorateurs, du SDK de l'instrument et du portail public, que vous recherchez ou indiquez l'adresse pour людей. Le modèle canonique est actuellement disponible dans `docs/account_structure.md` ; La liste de contrôle n'est pas disponible pour obtenir ces formulaires sans utilisation pour la maintenance et la maintenance.

## Безопасные сценарии обмена

- Pour pouvoir effectuer une copie/modification, utilisez l'adresse IH58. Показывайте разрешенный домен как подддерживающий контекст, чтобы строка с checksum была на виду.
- Avant de créer « Partager », vous trouverez l'adresse en texte brut et le code QR correspondant à la charge utile. Il est préférable de vérifier votre état avant de le confirmer.
- S'il y a peu de cartes (petites cartes, уведомления), indiquez les préfixes indiqués, sélectionnez plusieurs et installez-les après 4 à 6 symboles, чтобы сохранить якорь la somme de contrôle. Maintenant, appuyez sur le clavier pour copier les coups sans utilisation.
- Avant de rallumer l'appareil du tampon, placez le toast sur le niveau IH58 correspondant à votre copie. Par conséquent, lorsque vous installez la télémétrie, vous devez effectuer une copie et une création de « promotion », afin que vous puissiez déterminer la régression UX.

## IME et защита ввода- Ouvrir l'adresse non-ASCII à l'adresse locale. Lorsque vous utilisez des objets IME (pleine largeur, Kana, tons), sélectionnez la prévisualisation en ligne, afin de fermer le clavier sur latina ввод перед повтором.
- Pour les paramètres en texte brut, vous pouvez combiner les zones et les sondes ASCII avant la validation. Cela ne permettra pas de terminer le processus en ouvrant IME au poste de travail.
- Utilisez des outils de validation pour les menuisiers de largeur nulle, les sélecteurs de variation et les points de code Unicode. Si vous enregistrez une catégorie de codes ouverts, les suites de fuzzing peuvent importer la télémétrie.

## Organisation de la technologie d'assistance

- Annotez l'adresse du bloc avec `aria-label` ou `aria-describedby`, en définissant le préfixe de votre choix et en répartissant la charge utile dans les groupes. 4 à 8 символов (« ih tiret b trois deux… »). Ce ne sont pas des lecteurs d'écran qui peuvent utiliser des symboles précis.
- Сообщайте об успешных событиях копирования/поделиться через poli live région mise à jour. Choisissez une page (presse-papiers, feuille de partage, QR) qui contient un message, ce que vous avez fait sans vous concentrer sur votre sujet.
- Ajoutez le texte du texte `alt` pour le code QR (par exemple, « Adresse IH58 pour `<account>` sur la chaîne `0x1234` »). La solution QR-Canvas propose la solution de repli « Copier l'adresse sous forme de texte » pour les utilisateurs qui n'ont pas besoin de le faire.

## Adresse de l'adresse de Sora- Gating : appuyez sur la touche `sora…` pour que vous puissiez la modifier. Il est temps de mettre à jour le format de travail uniquement sur l'ensemble Sora Nexus.
- Étiquetage : il est possible d'afficher automatiquement la mention « Sora uniquement » et l'info-bulle relative à l'utilisation du formulaire IH58.
- Garde-corps : si les sections discriminantes actives ne sont pas disponibles pour Nexus, vous pouvez généralement générer l'adresse de votre choix et направляйте пользователя обратно к IH58.
- Télémétrie : affichez les informations fournies et les formulaires de copie, ce qui est un livre de jeu pour les incidents qui peuvent vous aider à résoudre vos problèmes.

## Cachet

- Réalisez les tests d'interface utilisateur automatiques (ou les suites Storybook A11y), afin de mettre à jour les nouvelles métadonnées ARIA et la mise en œuvre des applications. отказе IME.
- Créez des scénarios d'assurance qualité pour votre IME (kana, pinyin), testez un lecteur d'écran (VoiceOver/NVDA) et copiez les QR dans vos thèmes avant la publication.
- Vérifiez ces résultats dans la liste de contrôle de publication lors des tests de parité IH58, pour éviter les blocages lors de l'exécution.