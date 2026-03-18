---
lang: fr
direction: ltr
source: docs/source/gpuzstd_metal_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 019b3aa25ae224c1595467ac809f2c53290813e91a78b78b94ca71c3dd950264
source_last_modified: "2026-01-31T19:25:45.072449+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Pipeline GPU Zstd (Métal)

Ce document décrit le pipeline GPU déterministe utilisé par l'assistant Metal
pour la compression zstd. Il s'agit d'un guide de conception et de mise en œuvre pour le
Assistant `gpuzstd_metal` qui émet des trames zstd standard et des octets déterministes
pour un flux de séquence donné. Les sorties doivent faire un aller-retour avec les décodeurs CPU ; octet pour
la parité d'octets avec le compresseur CPU n'est pas requise car la génération de séquence
diffère.

## Objectifs

- Émettre des trames zstd standard qui décodent de manière identique avec le CPU zstd ; parité d'octet
  avec le compresseur CPU n'est pas nécessaire.
- Sorties déterministes sur le matériel, les pilotes et la planification des threads.
- Vérifications de limites explicites et durées de vie des tampons prévisibles.

## Note de mise en œuvre actuelle

- Recherche de correspondance et génération de séquences exécutées sur GPU.
- Assemblage de trames et codage entropique (Huffman/FSE) actuellement exécutés sur l'hôte
  en utilisant l'encodeur intégré à la caisse ; Les noyaux GPU Huffman/FSE sont testés par parité mais pas
  mais câblé dans le chemin plein format.
- Decode utilise le décodeur de trames intégré avec un repli CPU zstd pour les trames non prises en charge ;
  Le décodage complet du bloc GPU reste en cours.

## Pipeline d'encodage (haut niveau)

1. Mise en scène d'entrée
   - Copiez l'entrée dans un tampon de périphérique.
   - Partition en morceaux de taille fixe (pour la génération de séquences) et en blocs (pour
     assemblage de cadre zstd).
2. Recherche de correspondance et émission de séquence
   - Les noyaux GPU analysent chaque morceau et émettent des séquences (longueur littérale, correspondance
     longueur, décalage).
   - L'ordre des séquences est stable et déterministe.
3. Préparation littérale
   - Collecter les littéraux référencés par des séquences.
   - Créez des histogrammes littéraux et sélectionnez le mode de bloc littéral (brut, RLE ou
     Huffman) de manière déterministe.
4. Tables de Huffman (littéraux)
   - Générez des longueurs de code à partir de l'histogramme.
   - Créez des tables canoniques avec un bris d'égalité déterministe qui correspond au processeur
     sortie zstd.
5. Tableaux FSE (LL/ML/OF)
   - Normaliser les décomptes de fréquence.
   - Construire des tables de décodage/codage FSE de manière déterministe.
6. Écrivain Bitstream
   - Pack de bits little-endian (LSB-first).
   - Flush sur les limites d'octets ; pad avec des zéros uniquement.
   - Masquez les valeurs selon les largeurs de bits déclarées et appliquez des contrôles de capacité.
7. Assemblage du bloc et du cadre
   - Émettre des en-têtes de bloc (type, taille, indicateur du dernier bloc).
   - Sérialisez les littéraux et les séquences en blocs compressés.
   - Émettre des en-têtes de trame zstd standard et des sommes de contrôle facultatives.

## Pipeline de décodage (haut niveau)

1. Analyse de trame
   - Validez les octets magiques, les paramètres de fenêtre et les champs d'en-tête de trame.
2. Lecteur Bitstream
   - Lisez les séquences de bits LSB en premier avec des contrôles stricts des limites.
3. Décodage littéral
   - Décodez les blocs littéraux (bruts, RLE ou Huffman) dans le tampon littéral.
4. Décodage de séquence
   - Décoder les valeurs LL/ML/OF à l'aide des tables FSE.
   - Reconstruisez les correspondances à l'aide de la fenêtre coulissante.
5. Sortie et somme de contrôle
   - Écrivez les octets reconstruits dans le tampon de sortie.
   - Vérifiez les sommes de contrôle facultatives lorsqu'elles sont activées.

## Durée de vie et propriété des tampons- Tampon d'entrée : hôte -> périphérique, lecture seule.
- Tampon de séquence : dispositif produit par recherche de correspondance et consommé par entropie
  codage; pas de réutilisation entre blocs.
- Tampon littéral : périphérique, produit pour chaque bloc et libéré après le bloc
  émission.
- Tampon de sortie : périphérique, conserve les derniers octets de la trame jusqu'à ce que l'hôte les copie
  dehors.
- Tampons de travail : réutilisés dans tous les noyaux, mais toujours écrasés de manière déterministe.

## Responsabilités du noyau

- Noyaux de recherche de correspondance : recherche de correspondances et émission de séquences (LL/ML/OF + littéraux).
- Noyaux de construction Huffman : dériver les longueurs de code et les tables canoniques.
- Noyaux de construction FSE : construction de tables et de machines à états LL/ML/OF.
- Bloquer les noyaux d'encodage : sérialiser les littéraux et les séquences dans le flux binaire.
- Bloquer les noyaux de décodage : analyser le flux binaire et reconstruire les littéraux/séquences.

## Déterminisme et contraintes de parité

- Les constructions de tables canoniques doivent utiliser le même classement et le même principe de départage que le processeur.
  zstd.
- Pas d'atomes ou de réductions qui dépendent de la planification des threads pour tout octet de sortie.
- Le packaging Bitstream est petit-boutiste, LSB-first ; tampons d'alignement d'octets avec des zéros.
- Toutes les vérifications de limites sont explicites ; les entrées non valides échouent de manière déterministe.

## Validation

- Vecteurs dorés du processeur pour l'écrivain/lecteur bitstream.
- Tests de parité de corpus comparant les sorties GPU et CPU.
- Couverture floue pour les images mal formées et les conditions limites.

## Analyse comparative

Exécutez `cargo test -p gpuzstd_metal gpu_vs_cpu_benchmark -- --ignored --nocapture` pour
Comparez la latence d'encodage CPU et GPU selon les tailles de charge utile. Le test saute sur les hôtes
sans appareil compatible métal ; capturer la sortie avec les détails du matériel
lors de l'ajustement des seuils de déchargement du GPU. Norito applique la même coupure dans
`gpu_zstd::encode_all`, les appelants directs correspondent donc à la porte heuristique.