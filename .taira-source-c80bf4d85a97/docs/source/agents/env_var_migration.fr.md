---
lang: fr
direction: ltr
source: docs/source/agents/env_var_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9ce6010594e495116c1397b984000d1ee5d45d064294eca046f8dc762fa73b6
source_last_modified: "2026-01-04T10:50:53.607349+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Env → Suivi des migrations de configuration

Ce tracker résume les bascules de variables d'environnement liées à la production qui ont fait surface
par `docs/source/agents/env_var_inventory.{json,md}` et la migration prévue
chemin vers `iroha_config` (ou portée explicite de développement/test uniquement).


Remarque : `ci/check_env_config_surface.sh` échoue désormais lorsqu'un nouvel environnement de **production**
les cales apparaissent par rapport à `AGENTS_BASE_REF` sauf si `ENV_CONFIG_GUARD_ALLOW=1` est
ensemble; documentez ici les ajouts intentionnels avant d’utiliser le remplacement.

## Migrations terminées- **IVM Désactivation ABI** — `IVM_ALLOW_NON_V1_ABI` supprimé ; le compilateur rejette maintenant
  ABI non v1 sans condition avec un test unitaire gardant le chemin d'erreur.
- **Calme d'environnement de bannière de débogage IVM** — Suppression de la désactivation de l'environnement `IVM_SUPPRESS_BANNER` ;
  la suppression des bannières reste disponible via le programme de configuration programmatique.
- **Cache/dimensionnement IVM** — Dimensionnement du cache/prover/GPU threadé via
  `iroha_config` (`pipeline.{cache_size,ivm_cache_max_decoded_ops,ivm_cache_max_bytes,ivm_prover_threads}`,
  `accel.max_gpus`) et suppression des cales d'environnement d'exécution. Les hôtes appellent maintenant
  `ivm::ivm_cache::configure_limits` et `ivm::zk::set_prover_threads`, utilisation des tests
  `CacheLimitsGuard` au lieu des remplacements d'environnement.
- **Connecter la racine de la file d'attente** — Ajout de `connect.queue.root` (par défaut :
  `~/.iroha/connect`) à la configuration client et l'a transmis via la CLI et
  Diagnostic JS. Les assistants JS résolvent la configuration (ou un `rootDir` explicite) et
  Honorez uniquement `IROHA_CONNECT_QUEUE_ROOT` en développement/test via `allowEnvOverride` ;
  les modèles documentent le bouton afin que les opérateurs n'aient plus besoin de remplacements d'environnement.
- **Izanami opt-in réseau** — Ajout d'un indicateur CLI/config `allow_net` explicite pour
  l'outil de chaos Izanami ; les exécutions nécessitent désormais `allow_net=true`/`--allow-net` et
- **Bip de bannière IVM** — Remplacement de la cale d'environnement `IROHA_BEEP` par une configuration pilotée
  `ivm.banner.{show,beep}` bascule (par défaut : vrai/vrai). Bannière/bip de démarrage
  le câblage lit désormais la configuration uniquement en production ; les versions de développement/test honorent toujours
  le remplacement d'environnement pour les bascules manuelles.
- **Remplacement du spool DA (tests uniquement)** — Le remplacement `IROHA_DA_SPOOL_DIR` est désormais
  clôturé derrière les aides `cfg(test)` ; le code de production s'approvisionne toujours en bobine
  chemin de la configuration.
- **Intrinsèques de la crypto** — Remplacement de `IROHA_DISABLE_SM_INTRINSICS` /
  `IROHA_ENABLE_SM_INTRINSICS` avec la configuration
  Politique `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`) et
  retiré la garde `IROHA_SM_OPENSSL_PREVIEW`. Les hôtes appliquent la politique à
  démarrage, les bancs/tests peuvent s'inscrire via `CRYPTO_SM_INTRINSICS` et OpenSSL
  l'aperçu ne respecte désormais que l'indicateur de configuration.
  Izanami nécessite déjà `--allow-net`/configuration persistante, et les tests reposent désormais sur
  ce bouton plutôt que les bascules d'environnement ambiant.
- **Réglage GPU FastPQ** — Ajout de `fastpq.metal.{max_in_flight,threadgroup_width,metal_trace,metal_debug_enum,metal_debug_fused}`
  boutons de configuration (par défaut : `None`/`None`/`false`/`false`/`false`) et les transmettre via l'analyse CLI
  Les cales `FASTPQ_METAL_*` / `FASTPQ_DEBUG_*` se comportent désormais comme des solutions de secours de développement/test et
  sont ignorés une fois la configuration chargée (même lorsque la configuration les laisse non définis) ; les documents/inventaires étaient
  actualisé pour signaler la migration.【crates/irohad/src/main.rs:2609】【crates/iroha_core/src/fastpq/lane.rs:109】【crates/fastpq_prover/src/overrides.rs:11】
  (`IVM_DECODE_TRACE`, `IVM_DEBUG_WSV`, `IVM_DEBUG_COMPACT`, `IVM_DEBUG_INVALID`,
  `IVM_DEBUG_REGALLOC`, `IVM_DEBUG_METAL_ENUM`, `IVM_DEBUG_METAL_SELFTEST`,
  `IVM_FORCE_METAL_ENUM`, `IVM_FORCE_METAL_SELFTEST_FAIL`, `IVM_FORCE_CUDA_SELFTEST_FAIL`,
  `IVM_DISABLE_METAL`, `IVM_DISABLE_CUDA`) sont désormais protégés derrière les versions de débogage/test via un partage
  helper afin que les binaires de production les ignorent tout en préservant les boutons pour les diagnostics locaux. Env.
  l'inventaire a été régénéré pour refléter la portée de développement/test uniquement.- **Mises à jour des appareils FASTPQ** — `FASTPQ_UPDATE_FIXTURES` apparaît désormais uniquement dans l'intégration FASTPQ
  tests; les sources de production ne lisent plus la bascule d'environnement et l'inventaire reflète uniquement le test
  portée.
- **Actualisation de l'inventaire + détection de la portée** — Les outils d'inventaire env marquent désormais les fichiers `build.rs` comme
  construire la portée et suivre les modules de faisceau `#[cfg(test)]`/intégration afin de basculer uniquement en test (par exemple,
  `IROHA_TEST_*`, `IROHA_RUN_IGNORED`) et les indicateurs de build CUDA apparaissent en dehors du décompte de production.
  Inventaire régénéré le 7 décembre 2025 (518 références / 144 variables) pour garder le diff de garde env-config vert.
- **Protection de libération de cale d'environnement de topologie P2P** — `IROHA_P2P_TOPOLOGY_UPDATE_MS` déclenche désormais un
  erreur de démarrage dans les versions de version (avertissement uniquement dans le débogage/test) afin que les nœuds de production s'appuient uniquement sur
  `network.peer_gossip_period_ms`. L'inventaire env a été régénéré pour refléter le garde et le
  le classificateur mis à jour étend désormais les bascules protégées par `cfg!` en tant que débogage/test.

## Migrations hautement prioritaires (chemins de production)

- _Aucun (inventaire actualisé avec détection cfg!/debug ; vert de garde env-config après le durcissement de la cale P2P)._

## Dév/test uniquement bascule vers fence

- Balayage actuel (7 décembre 2025) : les indicateurs CUDA de construction uniquement (`IVM_CUDA_*`) sont définis comme `build` et le
  les bascules de harnais (`IROHA_TEST_*`, `IROHA_RUN_IGNORED`, `IROHA_SKIP_BIND_CHECKS`) s'enregistrent désormais en tant que
  `test`/`debug` en inventaire (y compris cales protégées `cfg!`). Aucune clôture supplémentaire n'est requise ;
  conservez les ajouts futurs derrière les aides `cfg(test)`/banc uniquement avec les marqueurs TODO lorsque les cales sont temporaires.

## Environnements au moment de la construction (laisser tels quels)

- Environnements cargo/fonctionnalité (`CARGO_*`, `OUT_DIR`, `DOCS_RS`, `PROFILE`, `CUDA_HOME`,
  `CUDA_PATH`, `JSONSTAGE1_CUDA_ARCH`, `FASTPQ_SKIP_GPU_BUILD`, etc.) restent
  problèmes de script de construction et sont hors de portée pour la migration de la configuration d'exécution.

## Actions suivantes

1) Exécutez `make check-env-config-surface` après les mises à jour de la surface de configuration pour détecter les nouvelles cales d'environnement de production
   tôt et attribuer des propriétaires de sous-systèmes/ETA.  
2) Actualisez l'inventaire (`make check-env-config-surface`) après chaque balayage afin
   le tracker reste aligné avec les nouveaux garde-corps et le diff de garde env-config reste sans bruit.