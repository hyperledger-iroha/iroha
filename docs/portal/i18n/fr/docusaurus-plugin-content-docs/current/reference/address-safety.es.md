---
lang: fr
direction: ltr
source: docs/portal/docs/reference/address-safety.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Sécurité et accessibilité des directions
description : Conditions requises pour l'UX pour présenter et partager les instructions de Iroha avec sécurité (ADDR-6c).
---

Cette page capture le contenu de la documentation ADDR-6c. L'application est limitée aux portefeuilles, aux explorateurs, aux outils du SDK et à toute surface du portail qui rend ou accepte les directions orientées vers les personnes. Le modèle de données canoniques vive en `docs/account_structure.md` ; la liste de contrôle d'abajo explique comment expliquer ces formats sans compromettre la sécurité ou l'accessibilité.

## Flux sécurisés de répartition- Par défaut, chaque action de copie/compartiment doit être effectuée par la direction I105. Montrez le résultat du domaine comme contexte d'apoyo pour que la chaîne avec somme de contrôle soit permanente devant.
- Offrir une action « Compartir » qui inclut la direction dans le texte plan et un QR dérivé de la même charge utile. Permettez que les personnes soient inspectées avant de confirmer.
- Lorsque l'espace oblige à une troncature (tarjetas petites, notifications), conserve le préfixe lisible, doit avoir des points suspensifs et conserver les derniers 4 à 6 caractères pour que l'étiquette de la somme de contrôle soit respectée. Prouvez une toque/atajo de teclado pour copier la chaîne complète sans truncamiento.
- Evitez la désincronisation des ports portables émettant un toast de confirmation que prévisualisez la chaîne I105 exacte qui est copiée. Il y a donc des télémétries, des comptes d'intentions de copie et des actions de partage pour détecter les régressions rapides de l'UX.

## IME et sauvegardes d'entrée- Rechaza entradas no ASCII dans les champs de direction. Lorsque vous montrez des objets de composition IME (pleine largeur, Kana, marques de ton), vous aurez une publicité en ligne qui explique comment changer le clavier à l'entrée en latin avant de réintenter.
- Prouvez une zone de péage en texte plan qui élimine les marques combinées et remplace les espaces en blanc par les espaces ASCII avant de valider. Cela évite que la personne ne progresse en désactivant l'IME avec un flux.
- Endure la validation des menuisiers de largeur nulle, des sélecteurs de variation et d'autres points de code Unicode sigilosos. Enregistrez la catégorie du point de code recherché pour que les suites fuzzing puissent importer la télémétrie.

## Attentes de la technologie d'assistance- Anota chaque bloc de direction avec `aria-label` ou `aria-describedby` qui supprime le préfixe lisible et groupe la charge utile en blocs de 4 à 8 caractères ("ih dash b three two ..."). Cela évite que les lecteurs de l'écran produisent un flux de caractères inintelligibles.
- Annonce des événements de copie/compartition exitosos au milieu d'une actualisation de la région en direct de manière polie. Inclut le destin (portapapeles, hoja de compartir, QR) pour que la personne sepa que l'action se complète sans déplacer le foco.
- Prouvez le texte `alt` descriptif pour les vues précédentes du QR (p. ex., "Direction I105 pour `<account>` dans la chaîne `0x1234`"). Inclut une solution de secours "Copiar direccion como texto" avec la toile de QR pour les personnes avec une faible vision.

## Direcciones comprimidas solo Sora- Gating : occulter la chaîne compressée `sora...` pour une confirmation explicite. La confirmation doit être répétée que le format fonctionne uniquement sur les chaînes Sora Nexus.
- Étiquette : chaque apparition doit inclure un insigne visible "Solo Sora" et une info-bulle qui explique pourquoi d'autres redes nécessitent la forme I105.
- Garde-corps : si le discriminant de la chaîne active n'est pas l'assignation de Nexus, il doit générer la direction comprimida et diriger la personne de vuelta vers I105.
- Télémétrie : enregistrez la fréquence qui est sollicitée et copiez la forme compressée pour que le playbook d'incidents détecte des pics de comparaison accidentels.

## Portes de qualité

- Étendre les évaluations automatisées de l'interface utilisateur (ou les suites de a11y dans le livre d'histoires) pour confirmer que les composants de direction exposent les métadonnées ARIA requises et que les messages de réponse par IME apparaissent.
- Inclut des scénarios de manuel d'assurance qualité pour l'entrée IME (kana, pinyin), une passe de lecteur d'écran (VoiceOver/NVDA) et une copie de QR en thèmes de haut contraste avant la sortie.
- Réfléchissez à ces vérifications dans les listes de contrôle de publication en même temps que les tests de parité I105 pour que les régressions soient bloquées jusqu'à ce qu'elles soient corrigées.