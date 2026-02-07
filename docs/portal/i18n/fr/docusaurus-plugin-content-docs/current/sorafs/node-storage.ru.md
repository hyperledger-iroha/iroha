---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-storage.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : stockage de nœud
titre : Conception de la maison SoraFS
sidebar_label : conception de votre entreprise
description : Structure d'architecture, crochets et crochets de cycle de vie pour les utilisateurs Torii, hébergés par SoraFS.
---

:::note Канонический источник
:::

## Conception du modèle pour l'utilisateur SoraFS (Черновик)

Ceci est une erreur, car l'utilisation de Iroha (Torii) peut être effectuée à partir du moment où
disponibilité dès SoraFS et sélectionnez le disque local pour la configuration et
обслуживания чанков. Она дополняет découverte-spécifications
`sorafs_node_client_protocol.md` et appareil pour luminaires SF-1b, description de l'architecture
stockage, organisation des ressources et configuration des produits, des pièces détachées
появиться в узле и gateway-code. Description des procédures d'utilisation pratiques dans
[Utilisation du Runbook] (./node-operations).

### Celi- Déverrouillez le validateur ou le numéro Iroha lors de la publication
  Vous devez contacter le fournisseur SoraFS sans consulter le grand livre.
- Держать модуль хранения детерминированным и управляемым Norito: manifestes,
  Des projets, des preuves de récupérabilité (PoR) et des fournisseurs de publicité — c'est ça
  источник истины.
- Применять операторские квоты, чтобы узел не исчерпал свои ресурсы из-за
  слишком большого количества запросов pin ou aller chercher.
- Отдавать здоровье/телеметрию (PoR sampling, latency fetch чанков, давление на
  disque) обратно в gouvernance et client.

### L'architecture de votre ville

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

Modules clés :- **Gateway** : ouvrez l'adresse HTTP Norito pour la broche précédente, puis récupérez-la.
  чанков, PoR sampling и телеметрии. Valider les charges utiles Norito et les activer
  запросы в chunk store. Utilisez le lien HTTP Torii, mais ce n'est pas le cas.
  вводить новый демон.
- **Pin Registry** : enregistrement des fichiers PIN dans `iroha_data_model::sorafs` et
  `iroha_core`. При принятии манифеста регистр хранит digest manifesta, digest
  Plana чанков, корень PoR и флаги возможностей провайдера.
- **Chunk Storage** : réalisation de disque `ChunkStore`, qui correspond à la version finale
  Manifestations, plans matériels pour les personnes `ChunkProfile::DEFAULT`, et сохраняет
  чанки в детерминированном disposition. Le bouton concerne le contenu des empreintes digitales et
  En ce qui concerne les métadonnées, l'échantillonnage peut être validé automatiquement sans perquisition
  всего файла.
- **Quota/Scheduler** : les limites maximales de l'opérateur (max. disque, max. broches)
  en очереди, макс. (parallèlement fetch, TTL чанков) et координирует IO, чтобы задачи
  Le grand livre n'est pas disponible. Le planificateur s'occupe de la preuve PoR et des prévisions
  échantillonnage с ограниченным CPU.

### Configuration

Ajoutez une nouvelle sexualité à `iroha_config` :

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # опциональный человекочитаемый тег
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```- `enabled` : prise en charge immédiate. Alors false, la passerelle est 503 pour le stockage
  Les utilisateurs et les utilisateurs ne sont pas impliqués dans la découverte.
- `data_dir` : répertoire téléphonique pour les chaînes, PoR деревьев и телеметрии chercher.
  Utilisez `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes` : limite maximale des broches. Le téléphone est prêt à ouvrir
  De nouvelles épingles dans les limites d'expédition.
- `max_parallel_fetches` : Limites de concurrence, planificateur spécifié, tâches
  Équilibrez le disque set/IO avec le validateur approprié.
- `max_pins` : maximum de broches de manifeste, qui sont utilisées pour la présentation
  expulsion/contre-pression.
- `por_sample_interval_secs` : cadencement automatique des tâches d'échantillonnage PoR. Каждый
  job выбирает `N` listьев (настраивается par manifeste) et эмитит телеметрию.
  La gouvernance peut être déterminée par `N`, pour le bouton
  `profile.sample_multiplier` (целое `1-4`). Значение может быть числом/строкой
  ou un problème de remplacement du profil, par exemple `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts` : structure, utilisation d'un générateur publicitaire pour la mise en service du bâtiment
  `ProviderAdvertV1` (pointeur d'enjeu, conseils QoS, sujets). Si vous l'utilisez, utilisez-le
  valeurs par défaut dans le registre de gouvernance.

Configuration de la configuration :- `[sorafs.storage]` s'applique à `iroha_config` et `SorafsStorage` et est installé
  из конфигурационного файла узла.
- `iroha_core` et `iroha_torii` permettent la configuration du stockage dans le générateur de passerelle et le bloc
  magasin avant de démarrer.
- Supprimer les remplacements de développement/test (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), non
  Dans la production, vous devez utiliser un système de configuration.

### CLI utilitaires

Lorsque HTTP est activé Torii, la caisse `sorafs_node` est envoyée.
TONкий CLI, les opérateurs peuvent écrire des exercices d'ingestion/exportation pour les tâches
backend persistant.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` ajoute le manifeste codé Norito `.to` ainsi que les octets de charge utile.
  Sur la base du plan de changement de profil du manifestant, il est prouvé que les participants sont présents
  digest, associe les fichiers de fichiers et émet automatiquement JSON `chunk_fetch_specs`, les fichiers
  Les instruments en aval peuvent vérifier la mise en page.
- `export` définit l'ID du manifeste et met en évidence le manifeste/la charge utile sur le disque
  (avec le plan JSON optionnel), les appareils sont installés par vous.

Les commandes indiquent le résumé JSON Norito sur la sortie standard, qui est destiné aux scripts. CLI
Après avoir effectué le test d'intégration, vous pourrez obtenir les manifestes aller-retour corrects
et les charges utiles pour l'API Torii.【crates/sorafs_node/tests/cli.rs:1】> Pari HTTP
>
> La passerelle Torii propose des assistants en lecture seule pour vous aider
> `NodeHandle` :
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — возвращает сохраненный
> Le manifeste Norito (base64) contient digest/metadata.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — Déterminateur
> Pièces de plan JSON (`chunk_fetch_specs`) pour l'outillage en aval.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> Cette entreprise est en ligne CLI et vous permet de vous connecter à une page locale.
> scripts pour les sondes HTTP sans les analyseurs.

### Жизненный цикл узла1. **Démarrage** :
   - Pour activer le stockage, vous pouvez créer un magasin de morceaux dans le répertoire local
     et емкостью. Cela permet de fournir/réparer les bases de PoR et la broche de relecture
     MANIFESTOS POUR LE PROGRAMME DE CUISINE.
   - Enregistrez la passerelle SoraFS (Norito JSON POST/GET entrées pour la broche,
     récupération, échantillonnage PoR, télémétrie).
   - Запустить PoR échantillonnage travailleur et монитор квот.
2. **Découverte / Annonces** :
   - Formez `ProviderAdvertV1` pour votre propre matériel/pièces, afin de le faire
     ключом, одобренным советом, и опубликовать через découverte canal.
     оставались доступными.
3. **Flux de travail d'épinglage** :
   - Gateway получает подписанный manifest (с планом чанков, корнем PoR и подписями
     совета). Валидирует список alias (`sorafs.sf1@1.0.0` обязателен) и убеждается,
     что план чанков соответствует метаданным манифеста.
   - Проверяет квоты. Lors des précédentes limitations de capacité/broches, les paramètres politiques sont affichés
     (структурированный Norito).
   - Le morceau ajouté dans `ChunkStore`, prouve les résumés de l'ingestion. Afficher PoR
     деревья и хранит le manifeste de métadonnées dans le registre.
4. **Récupérer le flux de travail** :
   - Отдает запросы range чанков с диска. Le planificateur utilise `max_parallel_fetches`
     et utilisez `429` pour la recherche.
   - Émet la structure du téléphone (Norito JSON) avec la latence, les octets servis etсчетчиками ошибок для aval мониторинга.
5. **Échantillonnage PoR** :
   - Le travailleur выбирает манифесты пропорционально весу (например, байтам хранения) и
     запускает детерминированный sampling через PoR дерево chunk store.
   - Obtenez des résultats pour l'audit de gouvernance et obtenez des annonces de fournisseurs
     / point de terminaison télém.
6. **Expulsion / квоты** :
   - Lorsque vous installez des objets, utilisez-les pour ouvrir de nouvelles broches. Опционально
     Les opérateurs veulent organiser l'expulsion politique (par exemple, TTL, LRU) après
     согласования модели gouvernance; пока дизайн предполагает строгие квоты и
     désépingler les opérations, les opérateurs d'ouverture.

### Intégration des déclarations d'hébergement et de planification- Torii doit être remplacé par `CapacityDeclarationRecord` depuis `/v1/sorafs/capacity/declare`
  Pour le `CapacityManager`, c'est pourquoi vous pouvez utiliser la fonction de prévisualisation en mémoire
  зафиксированных chunker/voie аллокаций. Le fournisseur publie des instantanés en lecture seule pour la télémétrie
  (`GET /v1/sorafs/capacity/state`) et des réserves par profil/par voie pour les nouvelles entreprises
  заказов.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- Эндпоинт `/v1/sorafs/capacity/schedule` принимает gouvernance-émis `ReplicationOrderV1`
  charges utiles. Lorsque vous commandez un fournisseur local, vous pouvez assurer la duplication
  Régler, valider l'emplacement du chunker/lane, réserver l'emplacement et utiliser `ReplicationPlan`
  Selon l'article sur les aliments, l'organisation peut permettre l'ingestion. Commandes pour
  Les fournisseurs proposent la solution `ignored`, permettant des flux de travail multi-opérateurs.【crates/iroha_torii/src/routing.rs:4845】
- Les crochets de complétion (par exemple, après l'ingestion) sont activés
  `POST /v1/sorafs/capacity/complete` pour le réservoir d'eau disponible ici
  `CapacityManager::complete_order`. Ответ включает instantané `ReplicationRelease`
  (totaux d'état, chunker/voie), ce que les outils d'orchestration sont plus
  ставить следующий заказ без sondage. Un autre projet comprend ce pipeline
  chunk store, comme la logique d'ingestion est disponible.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】- Le `TelemetryAccumulator` peut être utilisé à partir du `NodeHandle::update_telemetry`,
  comment permettre aux travailleurs d'arrière-plan de créer des échantillons de PoR/de disponibilité et de les créer
  выводить канонические `CapacityTelemetryV1` payloads sans modification des charges utiles
  ici planificateur.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Intégrations et travail de bureau

- **Gouvernance** : расширить `sorafs_pin_registry_tracker.md` телеметрией хранения
  (taux de réussite PoR, utilisation du disque). La double politique peut travailler au minimum
  Prenez en compte le taux de réussite PoR minime pour les publicités publiées.
- **SDK client** : téléchargez une nouvelle configuration de stockage (disque, alias), des choses
  outils de gestion могло bootstrapper узлы программно.
- **Télémétrie** : intégrer la pile de métriques correspondante (Prometheus /
  OpenTelemetry), ces mesures sont visibles dans les tableaux de bord d'observabilité.
- **Sécurité** : activez la gestion du module dans un pool de tâches asynchrone externe avec contre-pression
  et рассмотреть sandboxing чтения чанков через io_uring или ограниченные tokio pools,
  Les employés ne disposent pas de ressources.

Ceci est conçu pour le module de gestion opérationnelle et de détection, donc
Les boutons de l'opérateur sont destinés à l'utilisation dans les conditions de livraison SoraFS. Réalisation
Vous pouvez utiliser la passerelle `iroha_config`, `iroha_core`, `iroha_torii` et Norito,
un outillage pour l'annonce du fournisseur.