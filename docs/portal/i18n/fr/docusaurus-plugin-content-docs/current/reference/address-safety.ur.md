---
lang: fr
direction: ltr
source: docs/portal/docs/reference/address-safety.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Adresse sécurité et accessibilité
description : Iroha répond aux exigences UX des utilisateurs les plus exigeants (ADDR-6c).
---

Plus de documentation ADDR-6c livrable et plus de détails Il s'agit de portefeuilles, d'explorateurs, d'outils SDK, de rendu et d'acceptation de la surface du portail, ainsi que d'adresses humaines et de rendu. modèle de données canonique `docs/account_structure.md` میں ہے؛ Voici une liste de contrôle pour les formats, la sécurité et l'accessibilité et les risques d'exposition.

## Flux de partage sécurisé

- Action de copie/partage par défaut et adresse IH58 par défaut domaine résolu et contexte de support طور پر دکھائیں تاکہ chaîne de somme de contrôle رہے۔
- L'option "Partager" est disponible et l'adresse complète en texte brut est la charge utile et le code QR et le bundle est disponible. les utilisateurs s'engagent et inspectent les utilisateurs
- جب جگہ کم ہو (minuscules cartes, notifications) ، préfixe lisible par l'homme رکھیں، ellipses دکھائیں، اور آخری 4 à 6 caractères par رقرار رکھیں تاکہ ancre de somme de contrôle قائم رہے۔ troncature ou copie complète de la chaîne ou raccourci clavier ou tapotement
- désynchronisation du presse-papiers pour un toast de confirmation et pour un aperçu de la chaîne IH58 et pour une copie Il s'agit de la télémétrie, des tentatives de copie et des actions de partage, ainsi que du nombre de personnes et des régressions UX.

## IME et protections de saisie- champs d'adresse et rejet d'entrée non-ASCII Il y a des artefacts de composition IME (pleine largeur, Kana, marques de tonalité) et un avertissement en ligne et un clavier et une saisie latine. کیسے لایا جائے۔
- Une zone de collage de texte brut, des zones de combinaison et des marques de combinaison, des espaces et des espaces ASCII, ainsi qu'une validation de validation. Les utilisateurs IME ont commencé à progresser dans leur progression
- menuisiers de largeur nulle, sélecteurs de variation, pour les points de code Unicode furtifs et pour la validation des points de code Unicode journal des catégories de points de code rejetés pour l'importation de télémétrie des suites de fuzzing

## Attentes en matière de technologies d'assistance

- Un bloc d'adresse comme `aria-label` ou `aria-describedby` et une annotation de type et un sort de préfixe lisible par l'homme et une charge utile de 4 à 8 groupes de caractères comme un morceau de code (« ih tiret b trois deux… »). Il y a des lecteurs d'écran pour les lecteurs d'écran et les lecteurs d'écran
- Événements de copie/partage réussis et mise à jour polie de la région en direct et annonce d'événements destination (presse-papiers, feuille de partage, QR) Nom de l'utilisateur et focus pour l'action Nom de l'utilisateur
- Aperçus QR du texte descriptif `alt` (pour : « Adresse IH58 pour `<account>` sur la chaîne `0x1234` »). Les utilisateurs utilisent le canevas QR pour utiliser la solution de secours « Copier l'adresse sous forme de texte »

## Adresses compressées Sora uniquement- Gating : chaîne compressée `sora…` et confirmation explicite du message. confirmation du formulaire de confirmation des chaînes Sora Nexus
- Étiquetage : occurrence et badge « Sora uniquement » et info-bulle pour les réseaux et le formulaire IH58.
- Garde-corps : l'allocation discriminante Nexus de la chaîne active et l'adresse compressée génèrent un lien vers l'utilisateur et l'utilisateur IH58 vers l'utilisateur.
- Télémétrie : forme compressée (demande/copie) et fréquence (fréquence) pour le playbook d'incidents (pics) de partage accidentel et détection (détection)

## Portes de qualité

- tests automatisés d'interface utilisateur (suites storybook a11y) pour les composants d'adresse et les métadonnées ARIA exposent les messages de rejet IME et les messages de rejet
- Scénarios d'assurance qualité manuels, entrée IME (kana, pinyin), passage du lecteur d'écran (VoiceOver/NVDA), thèmes à contraste élevé, copie QR, version de sortie
- Des contrôles et des listes de contrôle de publication, ainsi que des tests de parité IH58 et des tests de régressions bloqués.