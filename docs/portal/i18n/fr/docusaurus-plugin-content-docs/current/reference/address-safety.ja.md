---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/reference/address-safety.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aacb778729b6a378d7c925c40a5c0b3d1d3e9c28d850f6f4b90d24b824d78567
source_last_modified: "2026-01-28T17:58:57+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: fr
direction: ltr
source: docs/portal/docs/reference/address-safety.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: Securite et accessibilite des adresses
description: Exigences UX pour presenter et partager les adresses Iroha en securite (ADDR-6c).
---

Cette page capture le livrable de documentation ADDR-6c. Appliquez ces contraintes aux wallets, explorers, outils SDK et a toute surface du portail qui rend ou accepte des adresses destinees aux humains. Le modele de donnees canonique se trouve dans `docs/account_structure.md`; la checklist ci-dessous explique comment exposer ces formats sans compromettre la securite ou l'accessibilite.

## Flux de partage surs

- Par defaut, toute action de copie/partage doit utiliser l'adresse IH58. Affichez le domaine resolu comme contexte d'appui afin que la chaine avec checksum reste au centre.
- Proposez une action "Partager" qui regroupe l'adresse en texte brut et un QR derive du meme payload. Permettez aux utilisateurs d'inspecter les deux avant de confirmer.
- Lorsque l'espace impose la troncature (cartes minuscules, notifications), conservez le prefixe lisible, affichez des ellipses et gardez les 4-6 derniers caracteres pour que l'ancre de checksum survive. Offrez un geste/raccourci clavier pour copier la chaine complete sans troncature.
- Empechez la desynchronisation du presse-papiers en emettant un toast de confirmation qui previsualise la chaine IH58 exacte copiee. La ou la telemetrie existe, comptez les tentatives de copie vs les actions de partage afin de detecter rapidement les regressions UX.

## IME et protections d'entree

- Rejetez toute entree non ASCII dans les champs d'adresse. Lorsque des artefacts de composition IME (full width, Kana, marques de tonalite) apparaissent, affichez un avertissement inline expliquant comment passer le clavier en saisie latine avant de reessayer.
- Fournissez une zone de collage en texte brut qui supprime les marques combinees et remplace les espaces par des espaces ASCII avant validation. Cela evite de perdre la progression si l'utilisateur desactive l'IME en cours de flux.
- Durcissez la validation contre les zero-width joiners, variation selectors et autres points de code Unicode furtifs. Journalisez la categorie du point de code rejete pour que les suites de fuzzing puissent importer la telemetrie.

## Attentes pour les technologies d'assistance

- Annotez chaque bloc d'adresse avec `aria-label` ou `aria-describedby` qui epelle le prefixe lisible et segmente le payload en groupes de 4-8 caracteres ("ih dash b three two ..."). Cela evite que les lecteurs d'ecran produisent un flux inintelligible de caracteres.
- Annoncez les actions de copie/partage reussies via une mise a jour de live region en mode polite. Incluez la destination (presse-papiers, partage, QR) pour que l'utilisateur sache que l'action s'est terminee sans deplacer le focus.
- Fournissez un texte `alt` descriptif pour les apercus QR (par ex., "Adresse IH58 pour `<account>` sur la chaine `0x1234`"). Offrez un fallback "Copier l'adresse en texte" a cote du canvas QR pour les utilisateurs malvoyants.

## Adresses compressees Sora-only

- Gating: cachez la chaine compressee `sora...` derriere une confirmation explicite. La confirmation doit reiterer que le format ne fonctionne que sur les chaines Sora Nexus.
- Etiquetage: chaque occurrence doit inclure un badge visible "Sora-only" et un tooltip expliquant pourquoi les autres reseaux exigent la forme IH58.
- Guardrails: si le discriminant de chaine active n'est pas l'allocation Nexus, refusez de generer l'adresse compressee et redirigez vers IH58.
- Telemetrie: enregistrez la frequence de demande et de copie de la forme compressee afin que le playbook d'incident detecte les pics de partage accidentel.

## Quality gates

- Etendez les tests UI automatises (ou les suites a11y de storybook) pour verifier que les composants d'adresse exposent les metadonnees ARIA requises et que les messages de rejet IME apparaissent.
- Incluez des scenarios de QA manuels pour l'entree IME (kana, pinyin), un passage lecteur d'ecran (VoiceOver/NVDA) et la copie de QR en themes a fort contraste avant release.
- Faites remonter ces verifications dans les checklists de release aux cotes des tests de parite IH58 afin que les regressions restent bloquees jusqu'a correction.
