---
lang: fr
direction: ltr
source: docs/source/iroha_monitor.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05149d624d680d04433be41a4525538c97bd103ae7f80dda2613a6adb181a93d
source_last_modified: "2026-01-03T18:07:57.206662+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha Moniteur

Le moniteur Iroha refactorisé associe une interface utilisateur de terminal légère à des animations.
festival d'art ASCII et le thème traditionnel Etenraku.  Il se concentre sur deux
flux de travail simples :

- **Mode Spawn-lite** – démarrez des stubs de statut/métriques éphémères qui imitent les pairs.
- **Mode Attachement** – pointez le moniteur vers les points de terminaison HTTP Torii existants.

L'interface utilisateur affiche trois régions à chaque actualisation :

1. **En-tête d'horizon Torii** – porte torii animée, mont Fuji, vagues de koi et étoile
   champ qui défile en synchronisation avec la cadence de rafraîchissement.
2. **Bande récapitulative** – blocs/transactions/gaz agrégés plus calendrier d'actualisation.
3. **Table des pairs et murmures du festival** – rangées de pairs à gauche, événement tournant
   connectez-vous à droite qui capture les avertissements (timeouts, charges utiles surdimensionnées, etc.).
4. **Tendance de gaz facultative** – activez `--show-gas-trend` pour ajouter une ligne d'étincelles
   résumant la consommation totale de gaz de tous les pairs.

Nouveau dans ce refactor :

- Scène ASCII animée de style japonais avec koi, torii et lanternes.
- Surface de commande simplifiée (`--spawn-lite`, `--attach`, `--interval`).
- Bannière d'introduction avec lecture audio optionnelle du thème gagaku (MIDI externe
  lecteur ou le synthétiseur logiciel intégré lorsque la plateforme/pile audio le prend en charge).
- Drapeaux `--no-theme` / `--no-audio` pour CI ou fumées rapides.
- Colonne « humeur » par homologue affichant le dernier avertissement, l'heure de validation ou la disponibilité.

## Démarrage rapide

Créez le moniteur et exécutez-le sur les pairs tronqués :

```bash
cargo run -p iroha_monitor -- --spawn-lite --peers 3
```

Attachez-vous aux points de terminaison Torii existants :

```bash
cargo run -p iroha_monitor -- \
  --attach http://127.0.0.1:8080 http://127.0.0.1:8081 \
  --interval 500
```

Invocation compatible CI (ignorer l'animation d'introduction et l'audio) :

```bash
cargo run -p iroha_monitor -- --spawn-lite --no-theme --no-audio
```

### Indicateurs CLI

```
--spawn-lite         start local status/metrics stubs (default if no --attach)
--attach <URL...>    attach to existing Torii endpoints
--interval <ms>      refresh interval (default 800ms)
--peers <N>          stub count when spawn-lite is active (default 4)
--no-theme           skip the animated intro splash
--no-audio           mute theme playback (still prints the intro frames)
--midi-player <cmd>  external MIDI player for the built-in Etenraku .mid
--midi-file <path>   custom MIDI file for --midi-player
--show-gas-trend     render the aggregate gas sparkline panel
--art-speed <1-8>    multiply the animation step rate (1 = default)
--art-theme <name>   choose between night, dawn, or sakura palettes
--headless-max-frames <N>
                     cap headless fallback to N frames (0 = unlimited)
```

## Introduction au thème

Par défaut, le démarrage lit une courte animation ASCII tandis que l'Etenraku marque
commence.  Ordre de sélection audio :

1. Si `--midi-player` est fourni, générez la démo MIDI (ou utilisez `--midi-file`)
   et lancez la commande.
2. Sinon, sous macOS/Windows (ou Linux avec `--features iroha_monitor/linux-builtin-synth`)
   rendre la partition avec le synthétiseur logiciel gagaku intégré (pas d'audio externe
   actifs requis).
3. Si l'audio est désactivé ou si l'initialisation échoue, l'intro imprime toujours le
   animation et entre immédiatement dans le TUI.

Le synthétiseur alimenté par CPAL s'active automatiquement sur macOS et Windows. Sous Linux, c'est
optez pour éviter de manquer les en-têtes ALSA/Pulse lors de la création de l'espace de travail ; l'activer
avec `--features iroha_monitor/linux-builtin-synth` si votre système fournit un
pile audio fonctionnelle.

Utilisez `--no-theme` ou `--no-audio` lors de l'exécution dans des shells CI ou sans tête.

Le synthétiseur logiciel suit désormais l'arrangement capturé dans la conception du synthétiseur *MIDI dans
Rust.pdf* : hichiriki et ryūteki partagent une mélodie hétérophonique tandis que le shō
fournit les tampons Aitake décrits dans le document.  Les données de note chronométrées sont conservées
dans `etenraku.rs` ; il alimente à la fois le rappel CPAL et la démo MIDI générée.
Lorsque la sortie audio n'est pas disponible, le moniteur saute la lecture mais restitue toujours
l'animation ASCII.

## Présentation de l'interface utilisateur- **Art d'en-tête** – généré chaque image par `AsciiAnimator` ; koi, lanternes torii,
  et les vagues dérivent pour donner un mouvement continu.
- **Bande récapitulative** : affiche les pairs en ligne, le nombre de pairs signalé, les totaux des blocs,
  totaux de blocs non vides, approbations/rejets d'émission, consommation de gaz et taux de rafraîchissement.
- **Table des pairs** – colonnes pour l'alias/point de terminaison, les blocs, les transactions, la taille de la file d'attente,
  consommation de gaz, latence et un indice « d'humeur » (avertissements, temps de validation, temps de disponibilité).
- **Festival murmure** – journal continu des avertissements (erreurs de connexion, charge utile)
  dépassements de limites, points finaux lents).  Les messages sont inversés (le dernier en haut).

Raccourcis clavier :

- `n` / Droite / Bas – déplace le focus vers le homologue suivant.
- `p` / Gauche / Haut – déplace le focus sur le homologue précédent.
- `q` / Esc / Ctrl-C – quittez et restaurez le terminal.

Le moniteur utilise crossterm + ratatui avec un tampon d'écran alternatif ; en le quittant
restaure le curseur et efface l'écran.

## Tests de fumée

La caisse contient des tests d'intégration qui exercent les deux modes et les limites HTTP :

-`spawn_lite_smoke_renders_frames`
-`attach_mode_with_stubs_runs_cleanly`
-`invalid_endpoint_surfaces_warning`
-`status_limit_warning_is_rendered`
-`attach_mode_with_slow_peer_renders_multiple_frames`

Exécutez uniquement les tests du moniteur :

```bash
cargo test -p iroha_monitor -- --nocapture
```

L'espace de travail comporte des tests d'intégration plus lourds (`cargo test --workspace`). Courir
les tests du moniteur séparément sont toujours utiles pour une validation rapide lorsque vous le faites
pas besoin de la suite complète.

## Mise à jour des captures d'écran

La démo de la documentation se concentre désormais sur l'horizon torii et la table des pairs.  Pour rafraîchir le
actifs, exécutez :

```bash
make monitor-screenshots
```

Cela enveloppe `scripts/iroha_monitor_demo.sh` (mode spawn-lite, graine/fenêtre fixe,
pas d'intro/audio, palette dawn, art-speed 1, capuchon sans tête 24) et écrit le
Cadres SVG/ANSI plus `manifest.json` et `checksums.json` dans
`docs/source/images/iroha_monitor_demo/`. `make check-iroha-monitor-docs`
enveloppe les deux gardes CI (`ci/check_iroha_monitor_assets.sh` et
`ci/check_iroha_monitor_screenshots.sh`) donc les hachages du générateur, les champs manifestes,
et les sommes de contrôle restent synchronisées ; la vérification de la capture d'écran est également livrée sous forme
`python3 scripts/check_iroha_monitor_screenshots.py`. Transmettez `--no-fallback` à
le script de démonstration si vous souhaitez que la capture échoue au lieu de revenir au
images cuites lorsque la sortie du moniteur est vide ; lorsque le repli est utilisé, le brut
Les fichiers `.ans` sont réécrits avec les images cuites afin que les manifestes/sommes de contrôle restent
déterministe.

## Captures d'écran déterministes

Les instantanés expédiés se trouvent dans `docs/source/images/iroha_monitor_demo/` :

![présentation du moniteur](images/iroha_monitor_demo/iroha_monitor_demo_overview.svg)
![surveiller le pipeline](images/iroha_monitor_demo/iroha_monitor_demo_pipeline.svg)

Reproduisez-les avec une fenêtre/graine fixe :

```bash
scripts/iroha_monitor_demo.sh \
  --cols 120 --rows 48 \
  --interval 500 \
  --seed iroha-monitor-demo
```

L'assistant de capture corrige `LANG`/`LC_ALL`/`TERM`, transmet
`IROHA_MONITOR_DEMO_SEED`, coupe le son et épingle le thème/vitesse artistique afin que le
les images s’affichent de manière identique sur toutes les plates-formes. Il écrit `manifest.json` (générateur
hachages + tailles) et `checksums.json` (condensés SHA-256) sous
`docs/source/images/iroha_monitor_demo/` ; Exécutions CI
`ci/check_iroha_monitor_assets.sh` et `ci/check_iroha_monitor_screenshots.sh`
échouer lorsque les actifs dérivent des manifestes enregistrés.

## Dépannage- **Aucune sortie audio** – le moniteur revient en lecture silencieuse et continue.
- **Le repli sans tête se termine plus tôt** – le moniteur limite les exécutions sans tête à quelques
  douzaine d'images (environ 12 secondes à l'intervalle par défaut) lorsqu'il ne peut pas changer
  le terminal en mode brut ; passez `--headless-max-frames 0` pour le faire fonctionner
  indéfiniment.
- **Charges utiles de statut surdimensionnées** – la colonne d'humeur des pairs et le journal du festival
  afficher `body exceeds …` avec la limite configurée (`128 KiB`).
- **Pairs lents** – le journal des événements enregistre les avertissements d'expiration ; concentrer ce pair sur
  mettez la ligne en surbrillance.

Profitez de l'horizon du festival !  Contributions pour des motifs ASCII supplémentaires ou
les panneaux de métriques sont les bienvenus : gardez-les déterministes pour que les clusters restituent la même chose
image par image, quel que soit le terminal.