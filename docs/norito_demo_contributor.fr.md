---
lang: fr
direction: ltr
source: docs/norito_demo_contributor.md
status: complete
translator: manual
source_hash: b11d23ecafbc158e0c83cdb6351085fde02f362cfc73a1a1a33555e90cc556ef
source_last_modified: "2025-11-09T09:04:55.207331+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traduction française de docs/norito_demo_contributor.md (Norito SwiftUI Demo Contributor Guide) -->

# Guide de contribution pour la démo SwiftUI Norito

Ce document décrit les étapes de configuration manuelle nécessaires pour exécuter la démo
SwiftUI contre un nœud Torii local et un ledger simulé. Il complète
`docs/norito_bridge_release.md` en se concentrant sur les tâches de développement
quotidiennes. Pour un tutoriel plus approfondi sur l’intégration du bridge Norito/stack
Connect dans des projets Xcode, voir `docs/connect_swift_integration.md`.

## Configuration de l’environnement

1. Installer le toolchain Rust défini dans `rust-toolchain.toml`.
2. Installer Swift 5.7+ et les Xcode Command Line Tools sur macOS.
3. (Optionnel) Installer [SwiftLint](https://github.com/realm/SwiftLint) pour le
   linting.
4. Exécuter `cargo build -p irohad` pour vérifier que le nœud compile sur votre machine.
5. Copier `examples/ios/NoritoDemoXcode/Configs/demo.env.example` vers `.env` et ajuster
   les valeurs à votre environnement. L’application lit les variables suivantes au
   démarrage :
   - `TORII_NODE_URL` — URL REST de base (les URLs WebSocket en sont dérivées).
   - `CONNECT_SESSION_ID` — identifiant de session 32 octets (base64/base64url).
   - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` — tokens renvoyés par
     `/v1/connect/session`.
   - `CONNECT_CHAIN_ID` — identifiant de chaîne annoncé lors du handshake de contrôle.
   - `CONNECT_ROLE` — rôle par défaut pré‑sélectionné dans l’UI (`app` ou `wallet`).
   - Helpers optionnels pour tests manuels : `CONNECT_PEER_PUB_B64`,
     `CONNECT_SHARED_KEY_B64`, `CONNECT_APPROVE_ACCOUNT_ID`,
     `CONNECT_APPROVE_PRIVATE_KEY_B64`, `CONNECT_APPROVE_SIGNATURE_B64`.

## Démarrer Torii + ledger simulé

Le repository fournit des scripts d’aide qui démarrent un nœud Torii avec un ledger en
mémoire pré‑chargé avec des comptes de démo :

```bash
./scripts/ios_demo/start.sh --config examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json
```

Le script produit :

- Les logs du nœud Torii dans `artifacts/torii.log`.
- Les métriques du ledger (format Prometheus) dans `artifacts/metrics.prom`.
- Des tokens d’accès client dans `artifacts/torii.jwt`.

`start.sh` garde le peer de démonstration en cours d’exécution jusqu’à ce que vous
fassiez `Ctrl+C`. Il écrit un snapshot d’état « prêt » dans `artifacts/ios_demo_state.json`
(source de vérité pour les autres artefacts), copie le log stdout actif de Torii, interroge
`/metrics` jusqu’à ce qu’un scrape Prometheus soit disponible et rend les comptes
configurés dans `torii.jwt` (en incluant les clés privées lorsque la configuration les
fournit). Le script accepte `--artifacts` pour surcharger le répertoire de sortie,
`--telemetry-profile` pour s’aligner sur des configurations Torii personnalisées et
`--exit-after-ready` pour les jobs CI non interactifs.

Chaque entrée dans `SampleAccounts.json` supporte les champs suivants :

- `name` (string, optionnel) — stocké comme metadata `alias` du compte.
- `public_key` (string multihash, obligatoire) — utilisé comme signataire du compte.
- `private_key` (optionnel) — inclus dans `torii.jwt` pour la génération de
  credentials client.
- `domain` (optionnel) — par défaut, le domaine de l’asset si omis.
- `asset_id` (string, obligatoire) — définition d’asset à minter pour le compte.
- `initial_balance` (string, obligatoire) — montant numérique minté sur le compte.

## Exécuter la démo SwiftUI

1. Construire le XCFramework comme décrit dans `docs/norito_bridge_release.md` et l’ajouter
   au projet de démo (les références attendent `NoritoBridge.xcframework` à la racine du
   projet).
2. Ouvrir le projet `NoritoDemoXcode` dans Xcode.
3. Sélectionner le schéma `NoritoDemo` et cibler un simulateur ou un device iOS.
4. Vérifier que le fichier `.env` est pris en compte via les variables d’environnement du
   schéma. Renseigner les valeurs `CONNECT_*` renvoyées par `/v1/connect/session` pour que
   l’UI soit pré‑remplie au lancement.
5. Vérifier les toggles d’accélération matérielle : `App.swift` appelle
   `DemoAccelerationConfig.load().apply()` pour que la démo consomme soit l’override
   d’environnement `NORITO_ACCEL_CONFIG_PATH`, soit un fichier bundlé
   `acceleration.{json,toml}`/`client.{json,toml}`. Supprimez/ajustez ces entrées si vous
   souhaitez forcer un fallback CPU avant d’exécuter.
6. Compiler et lancer l’application. L’écran d’accueil demande l’URL/token Torii s’ils ne
   sont pas déjà fournis via `.env`.
7. Initier une session « Connect » pour s’abonner aux mises à jour de compte ou approuver
   des requêtes.
8. Soumettre un transfert IRH et inspecter les logs affichés dans l’app ainsi que les logs
   de Torii.

### Toggles d’accélération matérielle (Metal / NEON)

`DemoAccelerationConfig` reflète la configuration du nœud Rust, de sorte que les
développeurs puissent exercer les chemins Metal/NEON sans coder en dur les seuils. Le
chargeur recherche les paramètres au démarrage dans cet ordre :

1. `NORITO_ACCEL_CONFIG_PATH` (définie dans `.env`/les arguments de schéma) — chemin
   absolu ou chemin `~`‑expansé vers un fichier `iroha_config` JSON/TOML.
2. Fichiers de configuration bundlés nommés `acceleration.{json,toml}` ou
   `client.{json,toml}`.
3. Si aucune source n’est disponible, les paramètres par défaut (`AccelerationSettings()`)
   restent en vigueur.

Exemple de snippet `acceleration.toml` :

```toml
[accel]
enable_metal = true
merkle_min_leaves_metal = 256
prefer_cpu_sha2_max_leaves_aarch64 = 128
```

Les champs laissés à `nil` héritent des valeurs par défaut du workspace. Les nombres
négatifs sont ignorés et l’absence de section `[accel]` entraîne un comportement
déterministe côté CPU. Lorsqu’on exécute sur un simulateur sans support Metal, le bridge
conserve silencieusement le chemin scalaire même si la configuration demande Metal.

## Tests d’intégration

- Les tests d’intégration résideront dans `Tests/NoritoDemoTests` (à ajouter une fois le CI
  macOS disponible).
- Les tests démarrent Torii via les scripts ci‑dessus et exercent les abonnements
  WebSocket, les soldes de tokens et les flux de transfert via le package Swift.
- Les logs des exécutions de tests sont stockés sous `artifacts/tests/<timestamp>/` avec
  les métriques et dumps de ledger d’exemple.

## Vérifications de parité CI

- Exécuter `make swift-ci` avant de pousser un PR qui touche à la démo ou aux fixtures
  partagés. La target exécute des vérifications de parité des fixtures, valide les flux
  des dashboards et génère des résumés localement. En CI, le même workflow dépend du
  metadata Buildkite (`ci/xcframework-smoke:<lane>:device_tag`) pour attribuer les
  résultats au simulateur ou à la lane StrongBox correcte ; après modification du pipeline
  ou des tags d’agents, vérifiez que ce metadata est toujours présent.
- En cas d’échec de `make swift-ci`, suivez `docs/source/swift_parity_triage.md` et
  examinez la sortie `mobile_ci` pour identifier la lane à régénérer ou l’incident à
  traiter.

## Dépannage

- Si la démo ne parvient pas à se connecter à Torii, vérifiez l’URL du nœud et la
  configuration TLS.
- Assurez‑vous que le token JWT (si utilisé) est valide et non expiré.
- Consultez `artifacts/torii.log` pour les erreurs côté serveur.
- Pour les problèmes WebSocket, inspectez la fenêtre de logs côté client ou la console
  Xcode.

