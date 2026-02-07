---
lang: fr
direction: ltr
source: docs/portal/docs/reference/address-safety.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Sécurité et accessibilité des adresses
description : Exigences UX pour présenter et partager les adresses Iroha en sécurité (ADDR-6c).
---

Cette page capture le livrable de la documentation ADDR-6c. Appliquez ces contraintes aux portefeuilles, explorateurs, outils SDK et à toute surface du portail qui rend ou accepte des adresses destinées aux humains. Le modèle de données canoniques se trouve dans `docs/account_structure.md`; la liste de contrôle ci-dessous explique comment ces formats exposés sans nuire à la sécurité ou à l'accessibilité.

## Flux de partage sur- Par défaut, toute action de copie/partage doit utiliser l'adresse IH58. Affichez le domaine résolu comme contexte d'appui afin que la chaîne avec checksum reste au centre.
- Proposez une action "Partager" qui regroupe l'adresse en texte brut et un QR dérive du meme payload. Permettez aux utilisateurs d'inspecter les deux avant de confirmer.
- L'espace impose la troncature (cartes minuscules, notifications), conservez le préfixe lisible, affichez des ellipses et gardez les 4-6 derniers caractères pour que l'ancre de checksum survive. Offrez un geste/raccourci clavier pour copier la chaîne complète sans troncature.
- Empechez la désynchronisation du presse-papiers en emettant un toast de confirmation qui prévisualise la chaîne IH58 exacte copiée. La ou la télémétrie existe, comptez les tentatives de copie vs les actions de partage afin de détecter rapidement les régressions UX.

## IME et protections d'entrée- Rejetez toute entrée non ASCII dans les champs d'adresse. Lorsque des artefacts de composition IME (pleine largeur, Kana, marques de tonalite) apparaissent, affichez un avertissement en ligne qui pourrait comment passer le clavier en saisie latine avant de réessayer.
- Fournissez une zone de collage en texte brut qui supprime les marques combinées et remplace les espaces par des espaces ASCII avant validation. Cela évite de perdre la progression si l'utilisateur désactive l'IME en cours de flux.
- Durcissez la validation contre les menuisiers de largeur nulle, les sélecteurs de variation et autres points de code Unicode furtifs. Journalisez la catégorie du point de code rejeté pour que les suites de fuzzing puissent importer la télémétrie.

## Attentes pour les technologies d'assistance- Annotez chaque bloc d'adresse avec `aria-label` ou `aria-describedby` qui épelle le préfixe lisible et segmente la charge utile en groupes de 4-8 caractères ("ih dash b three two..."). Cela évite que les lecteurs d'écran produisent un flux inintelligible de caractères.
- Annoncez les actions de copie/partage réussies via une mise à jour de live région en mode poli. Incluez la destination (presse-papiers, partage, QR) pour que l'utilisateur sache que l'action s'est terminée sans déplacer le focus.
- Fournissez un texte `alt` descriptif pour les apercus QR (par ex., "Adresse IH58 pour `<account>` sur la chaine `0x1234`"). Offrez un repli "Copier l'adresse en texte" à côté du canevas QR pour les utilisateurs malvoyants.

## Adresses compressées Sora uniquement- Gating : cachez la chaîne compressée `sora...` derrière une confirmation explicite. La confirmation doit réitérer que le format ne fonctionne que sur les chaînes Sora Nexus.
- Etiquetage : chaque occurrence doit inclure un badge visible "Sora-only" et une info-bulle expliquant pourquoi les autres réseaux exigent la forme IH58.
- Garde-corps : si le discriminant de chaîne active n'est pas l'allocation Nexus, refusez de générer l'adresse compressée et redirigez vers IH58.
- Télémétrie : enregistrez la fréquence de demande et de copie de la forme compressée afin que le playbook d'incident détecte les photos de partage accidentel.

## Portes de qualité

- Etendez les tests UI automatisés (ou les suites a11y de storybook) pour vérifier que les composants d'adresse exposant les métadonnées ARIA requises et que les messages de rejet IME apparaissent.
- Incluez des scénarios de manuels QA pour l'entrée IME (kana, pinyin), un lecteur de passage d'écran (VoiceOver/NVDA) et la copie de QR en thèmes à fort contraste avant la sortie.
- Faites remonter ces vérifications dans les checklists de release aux cotes des tests de parite IH58 afin que les régressions restent bloquées jusqu'à une correction.