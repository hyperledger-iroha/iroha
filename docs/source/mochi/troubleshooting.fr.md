---
lang: fr
direction: ltr
source: docs/source/mochi/troubleshooting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb67b304bae01fa4a50d25dc9f086811dabfbcb24239b3ec9679338248e18be6
source_last_modified: "2026-01-03T18:07:57.000591+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Guide de dépannage MOCHI

Utilisez ce runbook lorsque les clusters MOCHI locaux refusent de démarrer, restez coincé dans un
redémarrez la boucle ou arrêtez la diffusion des mises à jour de bloc/événement/statut. Il prolonge le
élément de la feuille de route « Documentation et déploiement » en intégrant les comportements des superviseurs
`mochi-core` en étapes concrètes de récupération.

## 1. Liste de contrôle des premiers intervenants

1. Capturez la racine de données utilisée par MOCHI. La valeur par défaut suit
   `$TMPDIR/mochi/<profile-slug>` ; les chemins personnalisés apparaissent dans la barre de titre de l'interface utilisateur et
   via `cargo run -p mochi-ui-egui -- --data-root ...`.
2. Exécutez `./ci/check_mochi.sh` à partir de la racine de l'espace de travail. Cela valide le noyau,
   L'interface utilisateur et les caisses d'intégration avant de commencer à modifier les configurations.
3. Notez le préréglage (`single-peer` ou `four-peer-bft`). La topologie générée
   détermine le nombre de dossiers/journaux homologues auxquels vous devez vous attendre sous la racine des données.

## 2. Collectez les journaux et les preuves de télémétrie

`NetworkPaths::ensure` (voir `mochi/mochi-core/src/config.rs`) crée une stabilité
mise en page :

```
<data_root>/<profile>/
  peers/<alias>/...
  logs/<alias>.log
  genesis/
  snapshots/
```

Suivez ces étapes avant d'apporter des modifications :

- Utilisez l'onglet **Journaux** ou ouvrez directement `logs/<alias>.log` pour capturer le dernier
  200 lignes pour chaque pair. Le superviseur suit les canaux stdout/stderr/system
  via `PeerLogStream`, ces fichiers correspondent donc à la sortie de l'interface utilisateur.
- Exporter un instantané via **Maintenance → Exporter un instantané** (ou appeler
  `Supervisor::export_snapshot`). L'instantané regroupe le stockage, les configurations et
  se connecte à `snapshots/<timestamp>-<label>/`.
- Si le problème concerne les widgets de flux, copiez le `ManagedBlockStream`,
  Indicateurs de santé `ManagedEventStream` et `ManagedStatusStream` du
  Tableau de bord. L'interface utilisateur affiche la dernière tentative de reconnexion et la raison de l'erreur ; saisir
  une capture d'écran pour l'enregistrement de l'incident.

## 3. Résoudre les problèmes de démarrage des pairs

La plupart des échecs de lancement par les pairs se répartissent en trois catégories :

### Binaires manquants ou remplacements incorrects

`SupervisorBuilder` s'adresse à `irohad`, `kagami` et (futur) `iroha_cli`.
Si l'interface utilisateur signale « échec du processus de génération » ou « autorisation refusée », pointez MOCHI
sur les bons binaires connus :

```bash
cargo run -p mochi-ui-egui -- \
  --irohad /path/to/irohad \
  --kagami /path/to/kagami \
  --iroha-cli /path/to/iroha_cli
```

Vous pouvez définir `MOCHI_IROHAD`, `MOCHI_KAGAMI` et `MOCHI_IROHA_CLI` pour éviter
taper les drapeaux à plusieurs reprises. Lors du débogage des builds du bundle, comparez les
`BundleConfig` dans `mochi/mochi-ui-egui/src/config/` par rapport aux chemins dans
`target/mochi-bundle`.

### Collisions portuaires

`PortAllocator` sonde l'interface de bouclage avant d'écrire les configurations. Si tu vois
`failed to allocate Torii port` ou `failed to allocate P2P port`, un autre
le processus écoute déjà sur la plage par défaut (8080/1337). Relancer MOCHI
avec des bases explicites :

```bash
cargo run -p mochi-ui-egui -- --torii-start 12000 --p2p-start 19000
```

Le constructeur déploiera des ports séquentiels à partir de ces bases, alors réservez une plage
dimensionné pour votre préréglage (pairs `peer_count` → ports `peer_count` par transport).

### Genesis et corruption du stockageSi Kagami se termine avant d’émettre un manifeste, les homologues se bloqueront immédiatement. Vérifier
`genesis/*.json`/`.toml` à l’intérieur de la racine des données. Réexécuter avec
`--kagami /path/to/kagami` ou pointez la boîte de dialogue **Paramètres** vers le binaire de droite.
En cas de corruption du stockage, utilisez **Wipe & re-genesis** de la section Maintenance.
bouton (couvert ci-dessous) au lieu de supprimer les dossiers à la main ; il recrée le
répertoires homologues et racines d’instantanés avant de redémarrer les processus.

### Réglage des redémarrages automatiques

`[supervisor.restart]` dans `config/local.toml` (ou les indicateurs CLI
`--restart-mode`, `--restart-max`, `--restart-backoff-ms`) contrôlent la fréquence à laquelle le
le superviseur réessaye les pairs ayant échoué. Définissez `mode = "never"` lorsque vous avez besoin de l'interface utilisateur pour
faire apparaître immédiatement la première défaillance, ou raccourcir `max_restarts`/`backoff_ms`
pour réduire la fenêtre de nouvelle tentative pour les tâches CI qui doivent échouer rapidement.

## 4. Réinitialiser les pairs en toute sécurité

1. Arrêtez les pairs concernés depuis le tableau de bord ou quittez l'interface utilisateur. Le superviseur
   refuse d'effacer le stockage pendant qu'un homologue est en cours d'exécution (`PeerHandle::wipe_storage`
   renvoie `PeerStillRunning`).
2. Accédez à **Maintenance → Effacer et régénérer**. MOCHI va :
   - supprimer `peers/<alias>/storage` ;
   - réexécutez Kagami pour reconstruire les configurations/genèse sous `genesis/` ; et
   - redémarrer les pairs avec les remplacements CLI/environnement préservés.
3. Si vous devez le faire manuellement :
   ```bash
   cargo run -p mochi-ui-egui -- --data-root /tmp/mochi --profile four-peer-bft --help
   # Note the actual root printed above, then:
   rm -rf /tmp/mochi/four-peer-bft
   ```
   Ensuite, redémarrez MOCHI pour que `NetworkPaths::ensure` recrée l'arborescence.

Archivez toujours le dossier `snapshots/<timestamp>` avant de l'effacer, même en local
développement : ces bundles capturent les journaux et configurations `irohad` précis nécessaires
pour reproduire des bugs.

### 4.1 Restauration à partir d'instantanés

Lorsqu'une expérience corrompt le stockage ou que vous devez rejouer un état connu comme bon, utilisez l'outil de maintenance.
bouton **Restaurer l'instantané** de la boîte de dialogue (ou appeler `Supervisor::restore_snapshot`) au lieu de copier
répertoires manuellement. Fournissez soit un chemin absolu vers le bundle, soit le nom du dossier nettoyé
sous `snapshots/`. Le superviseur :

1. arrêter tous les pairs en cours d'exécution ;
2. Vérifiez que le `metadata.json` de l'instantané correspond au `chain_id` actuel et au nombre de pairs ;
3. recopiez `peers/<alias>/{storage,snapshot,config.toml,latest.log}` dans le profil actif ; et
4. restaurez `genesis/genesis.json` avant de redémarrer les pairs s'ils étaient en cours d'exécution auparavant.

Si l'instantané a été créé pour un identifiant de préréglage ou de chaîne différent, l'appel de restauration renvoie un
`SupervisorError::Config` afin que vous puissiez récupérer un ensemble correspondant au lieu de mélanger silencieusement les artefacts.
Conservez au moins un nouvel instantané par préréglage pour accélérer les exercices de récupération.

## 5. Réparation des flux de bloc/événement/statut- **Le flux est bloqué mais les pairs sont sains.** Vérifiez les panneaux **Événements**/**Blocs**.
  pour les barres d'état rouges. Cliquez sur « Stop » puis sur « Démarrer » pour forcer le flux géré à démarrer.
  réabonnez-vous ; le superviseur enregistre chaque tentative de reconnexion (avec l'alias du homologue et
  erreur) afin que vous puissiez confirmer les étapes d’attente.
- **Superposition de statut obsolète.** `ManagedStatusStream` interroge `/status` tous les
  deux secondes et marque les données obsolètes après `STATUS_POLL_INTERVAL *
  STATUS_STALE_MULTIPLIER` (six secondes par défaut). Si le badge reste rouge, vérifiez
  `torii_status_url` dans la configuration homologue et assurez-vous que la passerelle ou le VPN n'est pas
  bloquer les connexions de bouclage.
- **Échecs de décodage d'événement.** L'interface utilisateur imprime l'étape de décodage (octets bruts,
  `BlockSummary`, ou décodage Norito) et le hachage de transaction incriminé. Exporter
  l'événement via le bouton presse-papier pour pouvoir reproduire le décodage dans les tests
  (`mochi-core` expose les constructeurs auxiliaires sous
  `mochi/mochi-core/src/torii.rs`).

Lorsque les flux se bloquent à plusieurs reprises, mettez à jour le problème avec l'alias exact du homologue et
chaîne d'erreur (`ToriiErrorKind`) afin que les jalons de télémétrie de la feuille de route restent liés
à des preuves concrètes.