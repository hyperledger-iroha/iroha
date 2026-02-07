---
lang: fr
direction: ltr
source: docs/portal/docs/reference/address-safety.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Seguranca e acessibilidade de enderecos
description : Conditions requises pour l'UX pour présenter et partager les utilisateurs Iroha avec sécurité (ADDR-6c).
---

Cette page capture ou entregavel de documentation ADDR-6c. Cette application restreint les portefeuilles, les explorateurs, les outils du SDK et toute surface du portail qui rend le rendu ou l'huile utilisée pour les personnes. Le modèle de données canoniques est présent dans `docs/account_structure.md` ; a checklist abaixo explica como expor esses formatsos sem comprometer seguranca ou acessibilidade.

## Flux de sécurité de partage- Par exemple, toute l'eau de copie/compartiment doit être utilisée avec l'endereco IH58. Exiba o dominio resolvido como contexteo de apoio para manter a string com checksum em destaque.
- Offre un article de « compartiment » qui inclut ou envoie un texte pur et un code QR dérivé de ma charge utile. Permita que a pessoa usuaria inspecione ambos antes de confirmar.
- Lorsque l'espace demande une troncature (cartes petites, notifications), conserve le préfixe légal initial, utilise les réticences et préserve les derniers 4 à 6 caractères pour que l'ancre fasse la somme de contrôle sur la durée. Disponibilisez un geste de toque/atalho de teclado pour copier une chaîne complète sans troncage.
- Éviter la désinscription du presse-papiers affichant un toast de confirmation qui montre exactement une chaîne IH58 copiée. Quand vous avez accès à la télémétrie, il y a des tentatives de copie par rapport aux coûts de partage pour détecter rapidement les régressions de l'UX.

## Salvaguardas para IME et entrada- Rejeite entrada non-ASCII em campos de endereco. Lorsque vous surgirez les artefatos de IME (pleine largeur, Kana, marques de tom), montrez un avis en ligne expliquant comment troquer ou teclado pour l'entrée latine avant de tenter nouvellement.
- Forneca une zone de collage dans du texte pur qui supprime les marques combinatoires et remplace les espaces en blanc par les espaces ASCII avant la validation. Cela évite de perdre la progression lorsque l'utilisateur désativa ou IME n'a pas le temps de flux.
- Fortaleca valide les menuisiers de largeur nulle, les sélecteurs de variations et d'autres points de code Unicode furtifs. Enregistrez une catégorie de point de code rejeté pour que les suites de fuzzing possam incorporent une télémétrie.

## Attentes pour les technologies d'assistance- Notez chaque bloc d'endereco avec `aria-label` ou `aria-describedby` qui détaille le préfixe légal et le groupe de charge utile dans les blocs de 4 à 8 caractères ("ih dash b three two ..."). Cela empêche les leitores de tela de produire un flux de caractères inintelligibles.
- Anuncie eventos de copy/share bem-sucedidos por meio d'une région live "polie". Inclut le destin (presse-papiers, feuille de partage, QR) pour que l'utilisateur sache qu'il a fini par déménager ou se déplacer.
- Forneca texto `alt` description para pre-visualizacoes de QR (par exemple, "Endereco IH58 para `<account>` na chain `0x1234`"). Coloquez le côté de la toile du QR un botao "Copiar endereco em texto" pour les personnes avec un visa.

## Enderecos comprimidos somente Sora

- Gating : occulter une chaîne compressée `sora...` à une confirmation explicite. Il faut confirmer que ce format fonctionne dans les chaînes Sora Nexus.
- Rotulage : toute la correspondance doit inclure un badge visible "Somente Sora" et une info-bulle expliquant ce qui est nécessaire au format IH58.
- Proteccoes: se o discriminante de chain ativo nao for a alocacao Nexus, recuse gerar o endereco comprimido e redirecione o usuario de volta para IH58.
- Télémétrie : enregistrez les quantités telles que le format compressé, sollicité et copié pour que le manuel d'incidents consomme des pics de compartimentage acide.## Portes de qualité

- Des tests d'interface utilisateur automatisés (ou des suites d'accessibilité dans un livre d'histoires) pour garantir que les composants de l'entreprise exposent les métadonnées ARIA nécessaires et que les messages de rejeicao de IME apparaissent.
- Inclut des scénarios de manuel d'assurance qualité pour l'entrée via IME (kana, pinyin), un passage avec le lecteur de téléphone (VoiceOver/NVDA) et une copie via QR dans les thèmes de haut contraste avant le lancement.
- Torne esses checks visiveis nas checklists de release, conjointement avec les tests de paridade IH58, pour que les régressions continuent à être bloquées mais soient toujours corrigées.