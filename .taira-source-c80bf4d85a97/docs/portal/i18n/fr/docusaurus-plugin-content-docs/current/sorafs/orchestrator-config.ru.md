---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : configuration de l'orchestrateur
titre : Administrateur de configuration SoraFS
sidebar_label : Configuration de l'opérateur
description: Настройка мульти-источникового fetch-оркестратора, интерпретация сбоев и отладка телеметрии.
---

:::note Канонический источник
Cette page correspond à `docs/source/sorafs/developer/orchestrator.md`. Il est possible de copier des copies de synchronisation si l'installation de documents ne concerne pas l'exploitation.
:::

# Руководство по мульти-источниковому fetch-оркестратору

L'opérateur de récupération multi-instruments SoraFS utilise des moyens de détermination
Des mises à jour parallèles pour les fournisseurs, publiées dans les annonces publicitaires
contrôler la gouvernance. Dans cette optique, comment recruter un entrepreneur,
Certains signaux doivent s'afficher avant le déploiement et certains paramètres télémétriques sont disponibles.
les indicateurs s'allument.

## 1. Configuration de la configuration

L'opérateur s'occupe de trois configurations historiques :

| Источник | Назначение | Première |
|--------------|------------|------------|
| `OrchestratorConfig.scoreboard` | Normalisez vos fournisseurs, vérifiez la qualité de la télémétrie et optimisez le tableau de bord JSON pour les auditeurs. | Appelé `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Principales configurations d'exécution (récupérations, limites parallèles, vérifications précises). | Carte pour `FetchOptions` et `crates/sorafs_car::multi_fetch`. |
| Paramètres CLI / SDK | Il s'agit de créer des problèmes, d'ajouter des télémétries aux régions et de créer des politiques de refus/renforcement. | `sorafs_cli fetch` раскрывает эти флаги напрямую; Le SDK est disponible à partir de `OrchestratorConfig`. |

Les fichiers JSON dans `crates/sorafs_orchestrator::bindings` sont disponibles en série
La configuration dans Norito JSON permet d'utiliser le SDK et l'automatisation.

### 1.1 Exemple de configuration JSON

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

Сохраняйте файл через стандартное наслаивание `iroha_config` (`defaults/`, utilisateur,
réel), les mesures à prendre en fonction des limites définies pour chacun d'entre eux
nodas. Pour le déploiement direct uniquement du profil SNNet-5a, sélectionnez
`docs/examples/sorafs_direct_mode_policy.json` et recommandations informatiques pour
`docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Remplace les solutions

SNNet-9 assure la conformité axée sur la gouvernance pour l'opérateur. Nouvel objet
`compliance` dans la configuration Norito JSON fixe les exclusions, selon les paramètres
récupération du pipeline en direct uniquement :

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` obtient les codes ISO‑3166 alpha‑2, pour que ce soit le cas
  инстанция оркестратора. Les codes sont normalisés dans le registre de l'analyse.
- `jurisdiction_opt_outs` зеркалирует реестр gouvernance. Когда любая юрисдикция
  opérateur присутствует в списке, оркестратор применяет
  `transport_policy=direct-only` et utiliser la solution de repli
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` перечисляет digest manifesta (aveugle CID, в верхнем
  hexadécimal). Les charges utiles compatibles permettent de planifier et de publier directement uniquement
  repli `compliance_blinded_cid_opt_out` en télémétrie.
- `audit_contacts` indique l'URI, la gouvernance apparaît dans GAR
  playbooks операторов.
- `attestations` fournit des paquets de conformité pour les produits de base
  politique. Vous pouvez acheter le produit optionnel `jurisdiction` (ISO‑3166 alpha‑2),
  `document_uri`, canonique `digest_hex` (64 symboles), horodatage
  `issued_at_ms` et `expires_at_ms` en option. Ces œuvres d'art sont publiées dans
  L'auditeur-chef d'orchestre, qui s'occupe des outils de gouvernance, remplace les remplacements par
  подписанными документами.

Assurez-vous que le bloc soit conforme aux normes de configuration standard.
les opérateurs получали детерминированные remplacements. Оркестратор применяет
conseils de conformité _после_ en mode écriture : даже если SDK запрашивает `upload-pq-only`,
opt-out en cas de demande ou de manifestation en cas de refus de transport en direct uniquement
et votre entreprise s'en occupe, sinon vous n'aurez pas recours à des fournisseurs soviétiques.

Les catalogues canoniques de désinscription sont disponibles dans
`governance/compliance/soranet_opt_outs.json` ; Совет по gouvernance публикует
обновления через versions étiquetées. Les principales configurations (avec
attestations) déposé dans `docs/examples/sorafs_compliance_policy.json`, a
description du processus de fonctionnement
[playbook соответствия GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Utiliser la CLI et le SDK| Drapeau/Pôle | Effet |
|-------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Ограничивает, сколько провайдеров пройдут фильтр scoreboard. Installez `None` pour faire appel à tous les fournisseurs éligibles. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Ограничивает число ретраев на chunk. La limite précédente est `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Вкалывает instantanés латентности/сбоев в построитель scoreboard. L'installation de la télévision pour les pré-déclarations `telemetry_grace_secs` ne permet pas aux fournisseurs d'être éligibles. |
| `--scoreboard-out` | Сохраняет вычисленный tableau de bord (fournisseurs éligibles + non éligibles) pour l'analyse post-analyse. |
| `--scoreboard-now` | En préparant le tableau de bord d'horodatage (secondes Unix), vous capturez les appareils en définissant les paramètres. |
| `--deny-provider` / crochet score politique | Il est possible d'exclure les fournisseurs de planification en dehors des annonces publicitaires. Idéal pour la liste noire des entreprises. |
| `--boost-provider=name:delta` | Il s'agit d'un correctif de crédit à tour de rôle pondéré, qui ne fait pas partie de la gouvernance. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Les mesures et les logs structurels sont indiqués pour que les frontières puissent être exploitées en fonction de la géographie ou du déploiement global. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | Pour le numéro `soranet-first`, c'est un opérateur multi-histoire — bas. Utilisez `direct-only` en cas de rétrogradation ou de conformité directe, et `soranet-strict` est destiné aux pilotes PQ uniquement ; la conformité remplace остаются жестким потолком. |

SoraNet-first est le premier défectuosité, et les rollbacks sont facilement disponibles pour la solution
Bloqueur SNNet. Après la gouvernance SNNet-4/5/5a/5b/6a/7/8/12/13
требуемую позу (dans la boutique `soranet-strict`); до этого только override по
L'incident permet de définir `direct-only` et de le modifier dans le journal.
déploiement.

Vous avez des drapeaux sur la syntaxe `--` ainsi que sur `sorafs_cli fetch`, ainsi que dans
разработческом бинаре `sorafs_fetch`. Le SDK permet de prévisualiser les options que vous tapez
constructeurs.

### 1.4 Mise à jour du cache de garde

CLI permet de sélectionner les gardes du sélecteur SoraNet, que les opérateurs peuvent utiliser
детерминированно закреплять les relais d'entrée до полноценного déploiement SNNet-5.
Le processus de contrôle du robot contrôle trois nouveaux drapeaux :

| Drapeau | Назначение |
|------|-----------|
| `--guard-directory <PATH>` | Téléchargez le fichier JSON avec le consensus de relais (en fonction de votre date). Le répertoire précédent ouvre le cache de garde avant la récupération. |
| `--guard-cache <PATH>` | Сохраняет Norito-codé `GuardSet`. Les prochains programmes utilisent le cache, car tout nouveau répertoire n'est pas disponible. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Les dérogations spéciales pour chaque garde d'entrée (pour une durée de 3) et une autre pour une garde d'entrée (pour une durée de 30 jours). |
| `--guard-cache-key <HEX>` | Le clavier spécial 32 bits pour votre Guard Cache sur Blake3 MAC, vous pouvez le vérifier avant de l'utiliser. |

Le répertoire Payloads Guard utilise un schéma compact :

L'indicateur `--guard-directory` doit afficher la charge utile codée en Norito.
`GuardDirectorySnapshotV2`. L'instantané binaire fonctionne :- `version` — version схемы (сейчас `2`).
-`directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  Il s'agit d'un consensus métadonnée qui doit être soumis au certificat international.
- `validation_phase` — certificats politiques de porte (`1` = разрешить одну Ed25519
  подпись, `2` = предпочесть двойные подписи, `3` = требовать двойные подписи).
- `issuers` — gouvernance éminente avec `fingerprint`, `ed25519_public` et
  `mldsa65_public`. Empreinte digitale вычисляется как
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — bundles SRCv2 inclus (pour `RelayCertificateBundleV2::to_cbor()`).
  Ce bundle comprend le relais de descripteur, les indicateurs de capacité, la politique ML-KEM et
  Voici la version Ed25519/ML-DSA-65.

CLI vérifie chaque paquet de produits détectés par l'émetteur avant la vérification
instantanés.

Consultez la CLI avec `--guard-directory` pour obtenir un consensus réel sur
существующим cache. Sélecteur de gardes de sécurité, qui sont à l'intérieur
удержания и допустимы в répertoire; De nouveaux relais заменяют просроченные записи.
Une fois que vous avez récupéré le cache actuel, vous avez téléchargé `--guard-cache`,
обеспечивая детерминированность следующих сессий. Le SDK permet de développer,
вызывая `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
et avant `GuardSet` à `SorafsGatewayFetchOptions`.

`ml_kem_public_hex` permet de sélectionner le sélecteur de priorité des protections PQ à l'heure actuelle
déploiement SNNet-5. Basculements de scène (`anon-guard-pq`, `anon-majority-pq`,
`anon-strict-pq`) Les relais automatiques sont compatibles avec les relais classiques :
доступен PQ guard, le sélecteur сбрасывает лишние broches classiques, чтобы последующие
сессии предпочитали гибридные poignées de main. Les résumés CLI/SDK показывают итоговый
микс через `anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` et la fourniture de candidats/défisites/delts,
делая baisses de tension et replis classiques явными.

Les répertoires de garde peuvent être utilisés pour le bundle SRCv2 complet ici
`certificate_base64`. L'opérateur fournit un bundle de décodage, disponible en version officielle
Vous pouvez ajouter Ed25519/ML-DSA et obtenir un certificat de sécurité dans votre cache de garde.
Si vous souhaitez obtenir un certificat, sur la base des clés PQ,
настроек poignées de main et весов; obtention de certificats et sélecteur
Voici un circuit de cycle et des installations à partir de `telemetry::sorafs.guard` et
`telemetry::sorafs.circuit`, les correctifs sont bien valides, les suites de prise de contact et
наличие двойных подписей для каждого garde.

Utilisez les assistants CLI pour synchroniser les instantanés avec
publicistes :

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` permet de télécharger et de valider un instantané SRCv2 avant de l'installer sur le disque, sur `verify`.
la validation du pipeline pour les articles d'art provenant de deux commandes, en utilisant JSON
résumé, vous pouvez sélectionner le sélecteur de garde de sortie CLI/SDK.

### 1.5 Circuit de cycle inférieurКогда доступны и relay directory, и guard cache, оркестратор активирует circuit
gestionnaire de cycle de vie pour la publication et la maintenance préliminaires des circuits SoraNet
перед каждым chercher. Configuration configurée pour `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) pour deux nouveaux personnes :

- `relay_directory` : capture instantanée du répertoire SNNet-3, sauts intermédiaires/sorties
  выбираLISь детерминированно.
- `circuit_manager` : configuration optionnelle (ajoutée à la configuration),
  контролирующая TTL цепей.

Norito JSON utilise le bloc `circuit_manager` :

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Le SDK permet d'accéder au répertoire suivant
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), une CLI ajoute des fonctionnalités automatiques, ainsi
par `--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`).

Le gestionnaire met en service les circuits, ainsi que la protection des métadonnées (point final, clé PQ
ou un horodatage épinglé) ou un TTL. Aide `refresh_circuits`, disponible
avant la récupération (`crates/sorafs_orchestrator/src/lib.rs:1346`), émettez les journaux
`CircuitEvent`, l'opérateur peut résoudre le problème de la taille du cycle. Tremper
Test `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) démontrant la stabilité
латентность на трех rotations gardes; смотрите отчет в
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Procédures rapides locales

L'opérateur peut effectuer de manière optionnelle des procédures rapides locales, qui fonctionnent
La mise à jour et les adaptateurs SDK n'ont pas de certificats ni de clés de cache de garde. Proximité
слушает bouclage-adresse, завершает QUIC соединения et возвращает Norito manifest,
Certificat de certification et clé de cache de garde opérationnelle. Transports,
Le processus émis s'est produit dans `sorafs_orchestrator_transport_events_total`.

Sélectionnez le nouveau bloc `local_proxy` dans l'opérateur JSON :

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```- `bind_addr` indiquez l'adresse de votre choix (utilisez le port `0` pour l'événement
  port).
- `telemetry_label` s'étend aux mesures des frontières du pays
  и récupérer-sessions.
- `guard_cache_key_hex` (officiellement) permet de désactiver la saisie de votre clé
  guard cache, que vous utilisez CLI/SDK, que vous devez installer
  синхронизированными.
- `emit_browser_manifest` vous permet de consulter le manifeste, ce qui signifie que vous pouvez le faire.
  сохранять и проверять.
- `proxy_mode` s'ouvre, vous devez rechercher le trafic local (`bridge`) ou
  Il suffit d'extraire les métadonnées du SDK pour ouvrir les circuits SoraNet.
  (`metadata-only`). Pour la référence `bridge` ; utilisez `metadata-only`, sinon
  рабочая станция должна выдавать manifeste без ретрансляции потоков.
- `prewarm_circuits`, `max_streams_per_circuit` et `circuit_ttl_hint_secs`
  Avant d'ajouter des conseils supplémentaires au clavier, vous pouvez également ajouter des conseils en parallèle.
  потоки и понимать агрессивность circuits de réutilisation.
- `car_bridge` (opционально) указывает на локальный cache CAR-архивов. Pôle
  `extension` задает суфикс, добавляемый когда target ne содержит `*.car` ; задайте
  `allow_zst = true` pour votre première fois `*.car.zst`.
- `kaigi_bridge` (opционально) экспонирует Kaigi routes из spool в прокси. Pôle
  `room_policy` utilise le modèle `public` ou `authenticated`, qui est activé
  Les clients ont trouvé les étiquettes GAR correctes.
- `sorafs_cli fetch` remplace `--local-proxy-mode=bridge|metadata-only`
  et `--local-proxy-norito-spool=PATH`, vous pouvez utiliser la fonction d'exécution ou la sélection
  D'autres bobines sont compatibles avec les politiques JSON.
- `downgrade_remediation` (officiellement) permet de déclasser automatiquement le crochet.
  Aujourd'hui, l'opérateur s'occupe des relais de télémétrie pour la rétrogradation de votre entreprise et,
  Après avoir testé `threshold` sur `window_secs`, vous devez effectuer le processus de démarrage
  dans `target_mode` (pour remplacer `metadata-only`). Когда rétrograde прекращаются,
  Le processus s'effectue avec `resume_mode` après `cooldown_secs`. Utiliser massivement
  `modes`, pour gérer les relais à rouleaux de déclenchement (pour activer les relais d'entrée).

Lors du processus de mise en service du pont, en fonction de l'application :

- **`norito`** — le client cible du flux est clairement affiché
  `norito_bridge.spool_dir`. Cibles санитизируются (sans traversée, sans
  абсолютных путей), и если файл без расширения, применяется настроенный суффикс
  до отправки payload в браузер.
- **`car`** — Les cibles de flux sont définies par `car_bridge.cache_dir`, ensuite
  Il est possible de réparer et d'ouvrir les charges utiles, si `allow_zst` n'est pas activé.
  Le pont prévu s'ouvrira `STREAM_ACK_OK` avant le début de l'arche, les gens
  Les clients peuvent vérifier le pipeline.

Dans le cadre de ces processus, la balise de cache HMAC est créée (la clé de cache de garde étant celle de
lors de la poignée de main) et indiquez les codes de raison `norito_*` / `car_*`, à votre bord.
Vous avez des questions sur la désinfection des aliments et des boissons.`Orchestrator::local_proxy().await` poignée de verrouillage pour support PEM
certificat, mise à jour du manifeste du navigateur ou mise à jour correcte pour votre choix
приложения.

Lorsque le processus s'est terminé, vous avez maintenant accès au **manifeste v2**. Pomimo
Le certificat et la clé de cache de garde, v2 sont fournis :

- `alpn` (`"sorafs-proxy/1"`) et `capabilities`, pour les clients.
  протокол потока.
- `session_id` pour la poignée de main et `cache_tagging` bloc de sel pour la dérivation
  affinité de garde de session et balises HMAC.
- Conseils pour la sélection du circuit et de la garde (`circuit`, `guard_selection`,
  `route_hints`) pour l'ensemble de l'interface utilisateur pour l'ouverture des portes.
- `telemetry_v2` avec boutons d'amélioration et de confidentialité pour les instruments locaux.
- Le boîtier `STREAM_ACK_OK` correspond à `cache_tag_hex`. Les clients apprécient cette offre
  Dans l'application `x-sorafs-cache-tag` pour les applications HTTP/TCP, il y a une garde en cache
  Les sélections s'installent sur le disque.

Il s'agit d'une entreprise entièrement commerciale — les clients peuvent découvrir de nouveaux clés et
продолжать использовать v1 sous-ensemble.

## 2. Семантика отказов

L'opérateur fournit des produits de qualité et de préparation pour les personnes âgées
первого байта. Offres spéciales pour trois catégories :

1. **Отказы по éligible (pré-vol).** Провайдеры без capacité de portée,
   publier des annonces ou utiliser des paramètres télémétriques dans le tableau de bord
   L'artéfact n'est pas ajouté à la planification. Les résumés CLI заполняют массив
   `ineligible_providers`, les opérateurs qui peuvent visualiser la dérive
   gouvernance без парсинга логов.
2. **Épuisement de l'exécution.** Каждый провайдер отслеживает последовательные ошибки.
   Lorsque vous utilisez `provider_failure_threshold`, le fournisseur s'engage à le faire
   `disabled` pour les séances de concert. Si votre fournisseur est `disabled`, opérateur
   возвращает `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Детерминированные прерывания.** Les limites de la structure correspondent
   ошибки:
   - `MultiSourceError::NoCompatibleProviders` — portée/alignement du manifeste,
     Votre fournisseur d'accès n'est pas à votre disposition.
   - `MultiSourceError::ExhaustedRetries` — le budget est récupéré par morceau.
   - `MultiSourceError::ObserverFailed` — observateurs en aval (hameçons de streaming)
     отклонили проверенный morceau.

Alors que l'index contient un morceau de problème et, comme prévu, la finale est terminée.
причину отказа провайдера. Считайте эти ошибки release blockers — повторные
les messages contextuels relatifs à la saisie de votre message, à la publicité, à la télémétrie ou à l'affichage
провайдера не изменятся.

### 2.1 Tableau de bord de la sécurité

Lors de la création du tableau de bord final, l'opérateur `persist_path`
каждого прогона. Le document JSON correspond :

- `eligibility` (`eligible` ou `ineligible::<reason>`).
- `weight` (normalisé pour ce programme).
- métadonnée `provider` (identifiant, points de terminaison, mise en parallèle).

Archivez le tableau de bord des instantanés lors de la publication des éléments d'art, ce qui permet de résoudre le problème
la liste noire et le déploiement permettent d'effectuer des vérifications.

## 3. Télémétrie et surveillance

### 3.1 Paramètres Prometheus

L'opérateur émet des mesures selon `iroha_telemetry` :| Métrique | Étiquettes | Description |
|---------|--------|--------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Jauge active la récupération. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Гистограмма полной латентности chercher. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Счетчик финальных отказов (исчерпаны ретраи, нет провайдеров, ошибка observer). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Les documents sont disponibles auprès du fournisseur. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Les programmes d'ouverture des sessions de votre session sont liés à l'ouverture. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Il existe des paramètres politiques anonymes (en cas de baisse de tension ou de baisse de tension) lors du déploiement du stade et de la solution de secours. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | L'historique des relais PQ se trouve sur SoraNet. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | L'historique des relais PQ dans le tableau de bord instantané. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | L'histogramme définit la politique politique (разница между целью и фактической долей PQ). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | L'historique des relais classiques dans chaque séance. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | L'histogramme contient des relais classiques pendant les séances. |

Intégrez les mesures dans les tableaux de bord de préparation en cliquant sur les boutons de production.
Le plan de sortie recommandé pour le SF-6 :

1. **Récupérations actives** — alerte, si la jauge est en dehors des achèvements corrects.
2. **Taux de nouvelle tentative** — Préparé pour les lignes de base historiques précédentes sur `retry`.
3. **Échecs du fournisseur** — déclenchement des alertes de téléavertisseur, ainsi que l'annonce du fournisseur
   `session_failure > 0` dans 15 minutes.

### 3.2 Cibles de journaux structurés

L'opérateur publie des projets structurés pour déterminer les cibles :

- `telemetry::sorafs.fetch.lifecycle` — marqueurs `start` et `complete` avec le corps
  morceaux, ретраев и общей длительностью.
- `telemetry::sorafs.fetch.retry` — события ретраев (`provider`, `reason`,
  `attempts`) pour le triage rapide.
- `telemetry::sorafs.fetch.provider_failure` — fournisseur, ouvert ici
  повторяющихся ошибок.
- `telemetry::sorafs.fetch.error` — Offres finales avec `reason` et options
  метаданными провайдера.

Indiquez ces points dans le pipeline de journaux Norito en cas d'incident.
réponse был единый источник истины. Cycle de vie события показывают PQ/classique
mélangez `anonymity_effective_policy`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` et les cartes françaises qui améliorent la configuration
дашбORDов без парсинга метрик. Lors des déploiements GA d'aujourd'hui, vous aurez accès à vos logos
`info` pour le cycle de vie/nouvelle tentative est utilisé et utilise `warn` pour les erreurs de terminal.

### 3.3 Résumés JSON

`sorafs_cli fetch` et Rust SDK proposent un résumé structuré, par exemple :- `provider_reports` pour les utilisateurs/utilisateurs et le statut du fournisseur d'ouverture.
- `chunk_receipts`, le fournisseur de café a trouvé un morceau.
- les masses `retry_stats` et `ineligible_providers`.

Архивируйте summary при отладке проблемных провайдеров — reçus напрямую
соотносятся с лог-метаданными выше.

## 4. Операционный чекlist

1. **Déployez la configuration dans CI.** Déployez `sorafs_fetch` avec ce bouton.
   configuration, passez par `--scoreboard-out` pour la configuration de l'éligibilité et
   сравните с предыдущим release. Fournisseur inéligible de Lubboï
   bloquer la promotion.
2. **Proverter le téléphone.** Assurez-vous de déployer les mesures d'exportation
   `sorafs.fetch.*` et la structure des journaux avant la récupération multi-source
   для пользователей. La mesure la plus efficace est celle de l'opérateur de la façade
   не был вызван.
3. **Документируйте overrides.** En cas d'urgence `--deny-provider` ou
   `--boost-provider` indique JSON (ou CLI) dans le journal des modifications. Annulations
   Vous pouvez supprimer le remplacement et créer un nouvel instantané du tableau de bord.
4. ** Effectuer des tests de fumée. ** Après avoir révisé les budgets ou les plafonds
   провайдеров заново выполните chercher канонического luminaire
   (`fixtures/sorafs_manifest/ci_sample/`) et indiquez quels reçus sont en morceaux
   остаются детерминированными.

Vous devez retarder la livraison de l'orchestrateur lors de la mise en scène
déploiements et préparation de la télémétrie pour la réponse aux incidents.

### 4.1 Remplace les politiques

Les opérateurs peuvent autoriser les transports actifs/l'anonymat en dehors des activités de base
configurations, pour `policy_override.transport_policy` et
`policy_override.anonymity_policy` dans JSON `orchestrator` (ou avant
`--transport-policy-override=` / `--anonymity-policy-override=`
`sorafs_cli fetch`). Si vous remplacez le paramètre, l'opérateur est disponible
baisse de tension de secours : si vous essayez de supprimer le niveau PQ, récupérez la valeur de repli
`no providers` вместо тихого downgrade. Возврат к поведению по умолчанию —
простое очищение override полей.