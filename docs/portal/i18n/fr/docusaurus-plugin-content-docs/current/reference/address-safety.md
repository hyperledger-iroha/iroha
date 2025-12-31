<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/reference/address-safety.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sécurité et accessibilité des adresses
description: Exigences UX pour présenter et partager les adresses Iroha en sécurité (ADDR-6c).
---

Cette page capture le livrable de documentation ADDR-6c. Appliquez ces contraintes aux wallets, explorers, outils SDK et à toute surface du portail qui rend ou accepte des adresses destinées aux humains. Le modèle de données canonique se trouve dans `docs/account_structure.md`; la checklist ci-dessous explique comment exposer ces formats sans compromettre la sécurité ou l'accessibilité.

## Flux de partage sûrs

- Par défaut, toute action de copie/partage doit utiliser l'adresse IH-B32. Affichez le domaine résolu comme contexte d'appui afin que la chaîne avec checksum reste au centre.
- Proposez une action “Partager” qui regroupe l'adresse en texte brut et un QR dérivé du même payload. Permettez aux utilisateurs d'inspecter les deux avant de confirmer.
- Lorsque l'espace impose la troncature (cartes minuscules, notifications), conservez le préfixe lisible, affichez des ellipses et gardez les 4–6 derniers caractères pour que l'ancre de checksum survive. Offrez un geste/raccourci clavier pour copier la chaîne complète sans troncature.
- Empêchez la désynchronisation du presse-papiers en émettant un toast de confirmation qui prévisualise la chaîne IH-B32 exacte copiée. Là où la télémétrie existe, comptez les tentatives de copie vs les actions de partage afin de détecter rapidement les régressions UX.

## IME et protections d'entrée

- Rejetez toute entrée non ASCII dans les champs d'adresse. Lorsque des artefacts de composition IME (full width, Kana, marques de tonalité) apparaissent, affichez un avertissement inline expliquant comment passer le clavier en saisie latine avant de réessayer.
- Fournissez une zone de collage en texte brut qui supprime les marques combinées et remplace les espaces par des espaces ASCII avant validation. Cela évite de perdre la progression si l'utilisateur désactive l'IME en cours de flux.
- Durcissez la validation contre les zero-width joiners, variation selectors et autres points de code Unicode furtifs. Journalisez la catégorie du point de code rejeté pour que les suites de fuzzing puissent importer la télémétrie.

## Attentes pour les technologies d'assistance

- Annotez chaque bloc d'adresse avec `aria-label` ou `aria-describedby` qui épelle le préfixe lisible et segmente le payload en groupes de 4–8 caractères (“ih dash b three two …”). Cela évite que les lecteurs d'écran produisent un flux inintelligible de caractères.
- Annoncez les actions de copie/partage réussies via une mise à jour de live region en mode polite. Incluez la destination (presse-papiers, partage, QR) pour que l'utilisateur sache que l'action s'est terminée sans déplacer le focus.
- Fournissez un texte `alt` descriptif pour les aperçus QR (par ex., “Adresse IH-B32 pour `<alias>@<domain>` sur la chaîne `0x1234`”). Offrez un fallback “Copier l'adresse en texte” à côté du canvas QR pour les utilisateurs malvoyants.

## Adresses compressées Sora-only

- Gating: cachez la chaîne compressée `snx1…` derrière une confirmation explicite. La confirmation doit réitérer que le format ne fonctionne que sur les chaînes Sora Nexus.
- Étiquetage: chaque occurrence doit inclure un badge visible “Sora-only” et un tooltip expliquant pourquoi les autres réseaux exigent la forme IH-B32.
- Guardrails: si le discriminant de chaîne active n'est pas l'allocation Nexus, refusez de générer l'adresse compressée et redirigez vers IH-B32.
- Télémétrie: enregistrez la fréquence de demande et de copie de la forme compressée afin que le playbook d'incident détecte les pics de partage accidentel.

## Quality gates

- Étendez les tests UI automatisés (ou les suites a11y de storybook) pour vérifier que les composants d'adresse exposent les métadonnées ARIA requises et que les messages de rejet IME apparaissent.
- Incluez des scénarios de QA manuels pour l'entrée IME (kana, pinyin), un passage lecteur d'écran (VoiceOver/NVDA) et la copie de QR en thèmes à fort contraste avant release.
- Faites remonter ces vérifications dans les checklists de release aux côtés des tests de parité IH-B32 afin que les régressions restent bloquées jusqu'à correction.
