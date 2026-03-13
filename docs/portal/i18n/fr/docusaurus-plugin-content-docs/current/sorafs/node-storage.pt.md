---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-storage.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : stockage de nœud
titre : Conception de stockage do nodo SoraFS
sidebar_label : Conception du stockage à faire
description : Architecture de stockage, quotas et crochets de cycle de vie pour les nœuds Torii que hospedam dados SoraFS.
---

:::note Fonte canonica
Cette page espelha `docs/source/sorafs/sorafs_node_storage.md`. Mantenha ambas comme copies synchronisées.
:::

## Conception de stockage do nodo SoraFS (Draft)

Cette note est raffinée comme un nœud Iroha (Torii) pour choisir la chambre de
disponibilité des données du SoraFS et dédiées à un pédagogue de la discothèque locale pour
armazenar et servir des morceaux. Ela complémenta a spécificacao de découverte
`sorafs_node_client_protocol.md` pour le travail des luminaires SF-1b et les détails
arquitetura do lado do stockage, controles de recursos e plomberie de
configuration qui doit être connectée à un nœud et à nos chemins vers la passerelle.
Os exercices operacionais ficam non
[Runbook des opéracoes à nodo](./node-operations).

### Objets- Permettre que tout validateur ou processus auxiliaire du Iroha expose la discothèque
  ocioso como provenor SoraFS sem afetar as responsabilidades do ledger.
- Manter o modulo de storage déterministico e guado por Norito : manifestes,
  planos de chunk, raizes Proof-of-Retrievability (PoR) et publicités du fournisseur sao
  une fonte de verdade.
- Importer des quotas définis par l'opérateur pour qu'un nœud nao esgote sois
  recursos ao aceitar demasiadas requisicos de pin or fetch.
- Expor saude/telemetria (amostragem PoR, latencia de fetch de chunk, pressao de
  discothèque) de volta para gouvernenca e clientes.

### Architecture de haut niveau

```
+--------------------------------------------------------------------+
|                         Iroha/Torii Node                           |
|                                                                    |
|  +----------+      +----------------------+                        |
|  | Torii APIs|<---->|    SoraFS Gateway   |<---------------+       |
|  +----------+      |  (Norito endpoints)  |                |       |
|                    +----------+-----------+                |       |
|                               |                            |       |
|                    +----------v-----------+                |       |
|                    |     Pin Registry     |<---- manifests |       |
|                    |     (State / DB)     |                |       |
|                    +----------+-----------+                |       |
|                               |                            |       |
|                    +----------v-----------+                |       |
|                    |     Chunk Storage    |<---- chunk plans|       |
|                    |      (ChunkStore)    |                |       |
|                    +----------+-----------+                |       |
|                               |                            |       |
|                    +----------v-----------+                |       |
|                    |    Disk Quota/IO     |--pin/serve----->| Fetch |
|                    |      Scheduler       |                | Clients|
|                    +----------------------+                |       |
|                                                                    |
+--------------------------------------------------------------------+
```

Les modules incluent :- **Gateway** : expose les points de terminaison HTTP Norito pour les propositions de code PIN et les exigences de
  récupérer les morceaux, amostragem PoR et télémétrie. Charges utiles Valida Norito e
  encaminha requisicos para o chunk store. Réutiliser la pile HTTP avec Torii pour
  éviter un nouveau démon.
- **Pin Registry** : état de la broche du manifeste rastré sur `iroha_data_model::sorafs`
  et `iroha_core`. Quand un manifeste et un enregistrement du registre ou un résumé sont effectués
  manifest, digest do plano de chunk, Raiz PoR et flags de capacité do provenor.
- **Chunk Storage** : implémentation de `ChunkStore` dans la discothèque qui contient des manifestes
  assassinés, matérialisez les plans de chunk en utilisant `ChunkProfile::DEFAULT`, et
  conserver les morceaux dans la disposition déterministe. Cada chunk et associé à euh
  empreinte digitale du contenu et des métadonnées PoR pour qu'un amostragem puisse être revalidé
  sem reler o arquivo inteiro.
- **Quota/Scheduler** : impoe limites configurées par l'opérateur (octets max de
  disco, broches pendantes max, récupère parallèles max, TTL de chunk) et coordination IO
  pour que les tarefas du grand livre ne soient pas sans recursos. O planificateur aussi
  Responsavel por servir provas PoR e requisicoes de amostragem com CPU limitada.

### Configuration

Ajouter une nouvelle secao à `iroha_config` :

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # tag opcional legivel
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```- `enabled` : bascule de participation. Quando false o gateway retorna 503 para
  les points de terminaison de stockage et le noeud n'annoncent pas la découverte.
- `data_dir` : diretorio raid para dados de chunk, arvores PoR et telemetria de
  aller chercher. Par défaut `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes` : limite rigide pour dados de chunk pinados. Uma tarefa de
  fundo rejeita novos pins quando o limite e atingido.
- `max_parallel_fetches` : limite de correspondance pour le planificateur pour
  équilibrer la bande/IO de disco contre la charge du validateur.
- `max_pins` : maximo de pins de manifest que o nodo aceita avant d'appliquer
  évacuation/contre-pression.
- `por_sample_interval_secs` : cadence pour les travaux automatiques d'amostagem PoR.
  Chaque tâche comprend `N` folhas (configuravel por manifest) et émet des événements de
  télémétrie. La gouvernance peut être escalar `N` de forme déterministe définissant un
  j'ai les métadonnées `profile.sample_multiplier` (inteiro `1-4`). O pode de valeur
  être un numéro/une chaîne unique ou un objet avec des remplacements par profil, par exemple
  `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts` : structure utilisée pour le générateur de publicité pour les enfants en bas âge
  `ProviderAdvertV1` (pointeur d'enjeu, conseils de QoS, sujets). Se omitido o nodo
  Les valeurs par défaut des États-Unis sont le registre de gouvernance.

Configuration de plomberie :- `[sorafs.storage]` et défini sur `iroha_config` comme `SorafsStorage` et e
  chargé de l'archive de configuration du nœud.
- `iroha_core` et `iroha_torii` passent par la configuration du stockage pour le constructeur
  faire la passerelle et le magasin de morceaux sans démarrage.
- Remplace l'existence de dev/test (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), mais
  les déploiements de production doivent confier aucun archivage de configuration.

### Utilitaires de CLI

En ce qui concerne la surface HTTP du Torii, cet envoi est effectué dans la caisse
`sorafs_node` envoie un niveau CLI pour que les opérateurs puissent automatiser les forets de
ingérer/exporter contre le backend persistant. [crates/sorafs_node/src/bin/sorafs-node.rs:1]

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` attend le manifeste `.to` codifié dans Norito avec plus d'octets de charge utile
  correspondants. Ele reconstruit le plan de morceau à partir du profil de
  le chunking se manifeste, impose la parité de digest, persiste les archives de chunk e
  Émettre éventuellement un blob JSON `chunk_fetch_specs` pour l'outillage en aval
  valider la mise en page.
- `export` prend un ID de manifeste et grave le manifeste/charge utile armazenado em
  disco (avec plan JSON facultatif) pour que les appareils continuent à être reproduits.Les commandes doivent être imprimées dans un CV Norito JSON sur la sortie standard, ce qui facilite leur utilisation
scripts. Une CLI est assurée par un test d'intégration pour garantir que les manifestes sont
les charges utiles fazem aller-retour corretamente avant les API Torii entrent. [crates/sorafs_node/tests/cli.rs:1]

> Parité HTTP
>
> O gateway Torii agora expoe helpers lecture seule apoiados pelo mesmo
> `NodeHandle` :
>
> - `GET /v2/sorafs/storage/manifest/{manifest_id_hex}` - retour ou manifeste
> Norito armazenado (base64) avec résumé/métadonnées. [crates/iroha_torii/src/sorafs/api.rs:1207]
> - `GET /v2/sorafs/storage/plan/{manifest_id_hex}` - retour ou plan de morceau
> Déterministe JSON (`chunk_fetch_specs`) pour l'outillage en aval. [crates/iroha_torii/src/sorafs/api.rs:1259]
>
> Esses endpoints espelham a saida do CLI para que pipelines possam trocar
> scripts locaux pour les sondes HTTP et les analyseurs syntaxiques. [crates/iroha_torii/src/sorafs/api.rs:1207] [crates/iroha_torii/src/sorafs/api.rs:1259]

### Cycle de vie du nodo1. **Démarrage** :
   - Le stockage est activé ou nodo initializa o chunk store com o
     répertoire et capacité configurés. Est-ce inclus vérifier ou créer une base
     de dados de manifest PoR et fazer replay de manifests pinados para aquecer
     caches.
   - Bureau d'enregistrement en tant que rotation de la passerelle SoraFS (points de terminaison Norito JSON POST/GET para pin,
     récupérer, amostragem PoR, télémétrie).
   - Iniciar o Worker de Amostragem PoR e o Monitor de quotas.
2. **Découverte / Annonces** :
   - Gerar documentos `ProviderAdvertV1` usando capacidade/saude atual, assinar
     com a chave aprovada pelo conselho e publicar via Discovery. Utilisez une liste
3. **Flux de broche** :
   - O gateway recebe um manifest assinado (incluindo plano de chunk, raiz PoR,
     assinaturas do conselho). Valider la liste des alias (`sorafs.sf1@1.0.0`
     requis) et garantit que le plan du chunk correspond aux métadonnées manifestées.
   - Vérifier les quotas. Si la capacité/les limites de broche dépassent, répondez à l'erreur
     de politique (Norito structuré).
   - Diffusez des données de chunk pour le `ChunkStore`, vérifiez les résumés pendante
     un ingéré. Les métadonnées actuelles de PoR et armazena ne manifestent aucun registre.
4. **Flux de récupération** :
   - Servir les accessoires de la gamme de morceaux à partir de la discothèque. O planificateur impoe
     `max_parallel_fetches` et retourner `429` lorsque celui-ci est saturé.- Émettre une télémétrie structurée (Norito JSON) avec latence, octets de service et
     contadores de erro para monitoramento en aval.
5. **PoR d'Amostragem** :
   - La sélection des travailleurs se manifeste proportionnellement au peso (par exemple, octets
     armazenados) et roda amostragem deterministica en utilisant un arvore PoR do chunk store.
   - Conserver les résultats pour les auditoires de gouvernance et inclure des CV dans les publicités
     du fournisseur / points finaux de télémétrie.
6. **Eviccao / cumprimento de quotas** :
   - Quando a capacidade e atingida o nodo rejeita novos pins por padrao. En option,
     Les opérateurs peuvent configurer la politique d'expulsion (ex : TTL, LRU) quand le modèle
     de gouvernance estiver definido; por ora o design assumer les quotas estritas e
     opérations de désépinglage lancées par l'opérateur.

### Déclaration de capacité et intégration de planification- Torii après avoir repassé les mises à jour de `CapacityDeclarationRecord` de `/v2/sorafs/capacity/declare`
  pour le `CapacityManager` embutido, de modo que chaque nodo construit un visa dans la mémoire
  de vos alocacoes compromis de chunker e lane. O manager expose les instantanés en lecture seule
  pour la télémétrie (`GET /v2/sorafs/capacity/state`) et impoe réservations par profil ou voie
  antes de novas ordens serem aceitas. [crates/sorafs_node/src/capacity.rs:1] [crates/sorafs_node/src/lib.rs:60]
- O endpoint `/v2/sorafs/capacity/schedule` aceita payloads `ReplicationOrderV1`
  émis par la gouvernance. Quando a ordem mira o provenor local o manager verifica
  planification dupliquée, validation de la capacité du chunker/lane, réservation ou tranche et retour
  et `ReplicationPlan` décrit la capacité restante pour les ferraments de
  orquestração possam seguir com a ingestao. Ordres pour d'autres fournisseurs sao
  reçu avec la réponse `ignored` pour faciliter les flux de travail multi-opérateurs. [crates/iroha_torii/src/routing.rs:4845]
- Hooks de conclusion (par exemple, disparados apos ingestao bem sucedida) chamam
  `POST /v2/sorafs/capacity/complete` pour libérer les réserves via
  `CapacityManager::complete_order`. Une réponse incluant un instantané
  `ReplicationRelease` (total restantes, résidus de chunker/lane) pour l'outillage
  de orquestracao possa enfileirar a proxima ordem sem polling. Trabalho futuro
  Je vais le connecter au pipeline du magasin de morceaux ainsi que la logique d'absorption.[crates/iroha_torii/src/routing.rs:4885] [crates/sorafs_node/src/capacity.rs:90]
- O `TelemetryAccumulator` embutido peut être modifié via
  `NodeHandle::update_telemetry`, permettant aux travailleurs de s'enregistrer en arrière-plan
  amostras de PoR/uptime et éventuellement dériver des charges utiles canoniques
  `CapacityTelemetryV1` se trouve à l'intérieur de notre planificateur. [crates/sorafs_node/src/lib.rs:142] [crates/sorafs_node/src/telemetry.rs:1]

### Intégrations et travail futur

- **Gouvernance** : estender `sorafs_pin_registry_tracker.md` avec télémétrie de
  stockage (taxa de sucesso PoR, utilizacao de disco). Podem de politique d'admission
  Exigir capacité minima ou taxes minima de succès PoR antes de aceitar adverts.
- **SDK client** : explorer une nouvelle configuration de stockage (limites de discothèque, alias)
  pour que les ferramentas de gestao puissent amorcer les nœuds par programmation.
- **Télémétrie** : intégrer la pile de métriques existantes (Prometheus /
  OpenTelemetry) pour que les mesures de stockage apparaissent dans les tableaux de bord d'observation.
- **Sécurité** : installer le module de stockage dans un pool dédié aux tarifications asynchrones avec
  contre-pression et prise en compte du sandboxing des lectures de chunk via io_uring ou pools
  Tokyo est limité pour éviter que les clients malicieux soient récursifs.Cette conception de manteau ou de module de stockage optionnel et déterminant quant à la force
les boutons nécessaires pour que les opérateurs participent à la camada de disponibilité des données
SoraFS. Mettre en œuvre l'exigira mudancas em `iroha_config`, `iroha_core`, `iroha_torii`
Il n'y a pas de passerelle Norito, mais l'outillage est annoncé par le fournisseur.