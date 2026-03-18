---
lang: fr
direction: ltr
source: docs/source/mochi/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44faf6c98d141959cf8cf40b1df7d3d82c3448e6f2b1bc4fa54cdeceb97994b0
source_last_modified: "2026-01-03T18:07:56.999063+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Démarrage rapide MOCHI

**MOCHI** est le superviseur de bureau pour les réseaux locaux Hyperledger Iroha. Ce guide parcourt
installer les prérequis, créer l'application, lancer le shell egui et utiliser le
outils d’exécution (paramètres, instantanés, wipes) pour le développement quotidien.

## Prérequis

- Chaîne d'outils Rust : `rustup default stable` (Workspace Targets édition 2024 / Rust 1.82+).
- Chaîne d'outils de la plateforme :
  - macOS : outils de ligne de commande Xcode (`xcode-select --install`).
  - Linux : GCC, pkg-config, en-têtes OpenSSL (`sudo apt install build-essential pkg-config libssl-dev`).
- Dépendances de l'espace de travail Iroha :
  - `cargo xtask mochi-bundle` nécessite `irohad`, `kagami` et `iroha_cli` construits. Construisez-les une fois via
    `cargo build -p irohad -p kagami -p iroha_cli`.
- En option : `direnv` ou `cargo binstall` pour la gestion des binaires de fret locaux.

MOCHI utilise les binaires CLI. Assurez-vous qu'ils sont détectables via les variables d'environnement
ci-dessous ou disponible sur le PATH :

| Binaire | Remplacement de l'environnement | Remarques |
|--------------|----------------------|-------------------------------------------------------|
| `irohad` | `MOCHI_IROHAD` | Supervise ses pairs |
| `kagami` | `MOCHI_KAGAMI` | Génère des manifestes/instantanés de genèse |
| `iroha_cli` | `MOCHI_IROHA_CLI` | Facultatif pour les fonctionnalités d'assistance à venir |

## Construire MOCHI

Depuis la racine du référentiel :

```bash
cargo build -p mochi-ui-egui
```

Cette commande crée à la fois `mochi-core` et l'interface egui. Pour produire un bundle distribuable, exécutez :

```bash
cargo xtask mochi-bundle
```

La tâche bundle assemble les fichiers binaires, le manifeste et les stubs de configuration sous `target/mochi-bundle`.

## Lancement du shell egui

Exécutez l'interface utilisateur directement depuis cargo :

```bash
cargo run -p mochi-ui-egui
```

Par défaut, MOCHI crée un préréglage unique dans un répertoire de données temporaire :

- Racine des données : `$TMPDIR/mochi`.
- Port de base Torii : `8080`.
- Port de base P2P : `1337`.

Utilisez les indicateurs CLI pour remplacer les valeurs par défaut lors du lancement :

```bash
cargo run -p mochi-ui-egui -- \
  --data-root /path/to/workspace \
  --profile four-peer-bft \
  --torii-start 12000 \
  --p2p-start 13000 \
  --kagami /path/to/kagami \
  --irohad /path/to/irohad
```

Les variables d'environnement reflètent les mêmes remplacements lorsque les indicateurs CLI sont omis : définissez `MOCHI_DATA_ROOT`,
`MOCHI_PROFILE`, `MOCHI_CHAIN_ID`, `MOCHI_TORII_START`, `MOCHI_P2P_START`, `MOCHI_RESTART_MODE`,
`MOCHI_RESTART_MAX` ou `MOCHI_RESTART_BACKOFF_MS` pour préconfigurer le générateur de superviseur ; chemins binaires
continuer à respecter les points `MOCHI_IROHAD`/`MOCHI_KAGAMI`/`MOCHI_IROHA_CLI`, et `MOCHI_CONFIG` à un
explicite `config/local.toml`.

## Paramètres et persistance

Ouvrez la boîte de dialogue **Paramètres** depuis la barre d'outils du tableau de bord pour ajuster la configuration du superviseur :

- **Data root** : répertoire de base pour les configurations homologues, le stockage, les journaux et les instantanés.
- **Ports de base Torii / P2P** — ports de démarrage pour une allocation déterministe.
- **Visibilité du journal** — basculez les canaux stdout/stderr/system dans la visionneuse de journaux.

Des boutons avancés tels que la politique de redémarrage du superviseur sont présents
`config/local.toml`. Définissez `[supervisor.restart] mode = "never"` pour désactiver
redémarrages automatiques lors du débogage d'incident, ou ajustement
`max_restarts`/`backoff_ms` (via le fichier de configuration ou les indicateurs CLI
`--restart-mode`, `--restart-max`, `--restart-backoff-ms`) pour contrôler les nouvelles tentatives
comportement.L'application des modifications reconstruit le superviseur, redémarre tous les pairs en cours d'exécution et écrit les remplacements dans
`config/local.toml`. La fusion de configuration préserve les clés non liées afin que les utilisateurs avancés puissent conserver
ajustements manuels aux côtés des valeurs gérées par MOCHI.

## Instantanés et effacement/regenèse

La boîte de dialogue **Maintenance** expose deux opérations de sécurité :

- **Exporter un instantané** : copie le stockage/config/logs homologues et le manifeste de genèse actuel dans
  `snapshots/<label>` sous la racine de données active. Les étiquettes sont automatiquement désinfectées.
- **Restaurer l'instantané** : réhydrate le stockage homologue, les racines des instantanés, les configurations, les journaux et la genèse
  manifeste à partir d’un bundle existant. `Supervisor::restore_snapshot` accepte soit un chemin absolu, soit
  le nom du dossier `snapshots/<label>` nettoyé ; l'interface utilisateur reflète ce flux donc Maintenance → Restaurer
  peut relire des lots de preuves sans toucher manuellement aux fichiers.
- **Wipe & re-genesis** — arrête l'exécution des pairs, supprime les répertoires de stockage, régénère Genesis via
  Kagami et redémarre les homologues une fois l'effacement terminé.

Les deux flux sont couverts par des tests de régression (`export_snapshot_captures_storage_and_metadata`,
`wipe_and_regenerate_resets_storage_and_genesis`) pour garantir des sorties déterministes.

## Journaux et flux

Le tableau de bord expose les données/métriques en un coup d'œil :

- **Journaux** — suit les messages `irohad` stdout/stderr/system lifecycle. Changer de chaîne dans Paramètres.
- **Blocs/Événements** — les flux gérés se reconnectent automatiquement avec un intervalle exponentiel et annotent les trames
  avec des résumés décodés Norito.
- **Status** : interroge `/status` et affiche des sparklines pour la profondeur, le débit et la latence de la file d'attente.
- **Préparation au démarrage** — après avoir appuyé sur **Démarrer** (un seul homologue ou tous les homologues), sondes MOCHI
  `/status` avec espacement limité ; la bannière signale quand chaque pair est prêt (avec le
  profondeur de la file d'attente) ou fait apparaître l'erreur Torii si le délai de préparation expire.

Les onglets pour l'explorateur d'état et le compositeur fournissent un accès rapide aux comptes, aux actifs, aux pairs et aux éléments communs.
instructions sans quitter l’interface utilisateur. La vue Peers reflète la requête `FindPeers` afin que vous puissiez confirmer
quelles clés publiques sont actuellement enregistrées dans l'ensemble de validateurs avant d'exécuter les tests d'intégration.

Utilisez le bouton **Gérer le coffre-fort de signature** de la barre d'outils Composer pour importer ou modifier les autorités de signature. Le
La boîte de dialogue écrit les entrées dans la racine du réseau actif (`<data_root>/<profile>/signers.json`) et les enregistre
les clés du coffre-fort sont immédiatement disponibles pour les aperçus et les soumissions de transactions. Lorsque le coffre-fort est
vide, le compositeur revient aux clés de développement fournies afin que les flux de travail locaux continuent de fonctionner.
Les formulaires couvrent désormais la création/gravure/transfert (y compris la réception implicite), la définition de domaine/compte/actif
inscription, politiques d'admission de compte, propositions multisig, manifestes Space Directory (AXT/AMX),
Manifestes de broches SoraFS et actions de gouvernance telles que l'octroi ou la révocation de rôles si courants
Les tâches de création de feuille de route peuvent être répétées sans écrire manuellement les charges utiles Norito.

## Nettoyage et dépannage- Arrêtez l'application pour mettre fin aux pairs supervisés.
- Supprimez la racine des données (`rm -rf <data_root>`) pour réinitialiser tous les états.
- Si les emplacements Kagami ou irohad changent, mettez à jour les variables d'environnement ou réexécutez MOCHI avec le
  indicateurs CLI appropriés ; la boîte de dialogue Paramètres conservera les nouveaux chemins lors de la prochaine application.

Pour une automatisation supplémentaire, vérifiez `mochi/mochi-core/tests` (tests du cycle de vie du superviseur) et
`mochi/mochi-integration` pour les scénarios Torii simulés. Pour expédier des lots ou câbler le
desktop dans les pipelines CI, reportez-vous au guide {doc}`mochi/packaging`.

## Porte de test locale

Exécutez `ci/check_mochi.sh` avant d'envoyer les correctifs afin que la porte CI partagée exerce les trois MOCHI
caisses :

```bash
./ci/check_mochi.sh
```

L'assistant exécute `cargo check`/`cargo test` pour `mochi-core`, `mochi-ui-egui` et
`mochi-integration`, qui capture la dérive du luminaire (captures de blocs canoniques/d'événements) et le faisceau egui
régressions d’un seul coup. Si le script signale des appareils obsolètes, réexécutez les tests de régénération ignorés,
par exemple :

```bash
cargo test -p mochi-core regenerate_block_wire_fixture -- --ignored
```

La réexécution de la porte après la régénération garantit que les octets mis à jour restent cohérents avant de pousser.