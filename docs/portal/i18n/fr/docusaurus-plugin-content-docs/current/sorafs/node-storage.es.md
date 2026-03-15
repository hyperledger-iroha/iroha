---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-storage.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : stockage de nœud
titre : Conception de l'aménagement du nœud SoraFS
sidebar_label : Conception de l'emplacement du nœud
description: Arquitectura de almacenamiento, cuotas y hooks del ciclo de vida para nodos Torii que alojan datos de SoraFS.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/sorafs_node_storage.md`. Assurez-vous que les copies sont synchronisées jusqu'à ce que le ensemble de documents Sphinx hérité soit retiré.
:::

## Conception de stockage du nœud SoraFS (Borrador)

Cette note précise comment un nœud Iroha (Torii) peut opter pour la tête de
disponibilité des données de SoraFS et consacrer une partie de la discothèque locale pour
préparer et servir les morceaux. Complémente la spécification de découverte
`sorafs_node_client_protocol.md` et le travail des luminaires SF-1b en détail
architecture du lieu de stockage, contrôle des ressources et plomberie de
configuration qui doit être connectée au nœud et aux routes de la passerelle.
Les pratiques opérationnelles vivent dans le
[Runbook des opérations de nœud] (./node-operations).

### Objets- Permettre que tout validateur ou processus auxiliaire de Iroha exponga disco
  Nous sommes le fournisseur SoraFS sans affecter les responsabilités du grand livre.
- Maintenir le module de stockage déterministe et guidé par Norito :
  manifestes, plans de chunk, preuves de récupérabilité (PoR) et annonces de
  provenor son la fuente de verdad.
- Impone cuotas definidas por el operador para qu'un nodo no agote sus propios
  recursos al acceptar demasiadas sollicitudes de pin o fetch.
- Exponer salud/telemetría (muestreo PoR, latencia de fetch de chunks, presión
  de disco) hacia gobernanza y clients.

### Architecture de haut niveau

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

Modules clés :- **Gateway** : expose les points de terminaison HTTP Norito pour les propriétés du code PIN et les sollicitudes
  de récupérer des morceaux, de muestreo PoR et de télémétrie. Charges utiles Valida Norito et
  enruta las sollicitudes hacia el chunk store. Utiliser la pile HTTP de Torii
  pour éviter un nouveau démon.
- **Pin Registry** : état du pin de manifeste rastreado en `iroha_data_model::sorafs`
  et `iroha_core`. Lorsque vous acceptez un manifeste, le registre enregistre le résumé
  del manifest, digest del plan de chunk, raíz PoR y flags de capacidad del
  fournisseur.
- **Chunk Storage** : implémentation `ChunkStore` résolue par le disco que l'ingénieur
  manifeste firmados, matérialise les plans de chunk en utilisant `ChunkProfile::DEFAULT`
  et conserver les morceaux sous une mise en page déterminée. Chaque morceau est associé à un
  empreintes digitales du contenu et des métadonnées PoR pour que le musée puisse être revalidé
  sans publier l'archive complète.
- **Quota/Scheduler** : impose des limites configurées par l'opérateur (octets maximum
  de disco, pins pendientes máximos, récupère paralelos máximos, TTL de chunk)
  et coordonnez les IO pour que les comptes du grand livre ne soient pas exécutés sans recursos. El
  Le planificateur effectue également des tests PoR et des demandes de rendez-vous avec CPU acotada.

### Configuration

Ajoutez une nouvelle section à `iroha_config` :

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # etiqueta opcional legible
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```- `enabled` : bascule de participation. Quand la passerelle est fausse, elle répond
  503 pour les points de terminaison de stockage et le noeud n'est pas annoncé dans la découverte.
- `data_dir` : répertoire racine pour les données de chunk, les arbres PoR et la télémétrie de
  aller chercher. La valeur par défaut est `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes` : limite dure pour les données de morceaux fixés. Une tâche de
  fondo rechaza nuevos pins cuando se alcanza el límite.
- `max_parallel_fetches` : tope de simultanéité impuesto por el planningr para
  équilibrer l'ancre de bande/IO de disco avec la charge du validateur.
- `max_pins` : nombre maximum de broches du manifeste qui acceptent le nœud avant de
  appliquer l'expulsion/contre-pression.
- `por_sample_interval_secs` : cadence pour les travaux automatiques de musique
  PoR. Chaque travail montre `N` hojas (configurable par manifeste) et émet
  événements de télémétrie. La gouvernance peut escalader `N` de forme déterministe
  établir la clé de métadonnées `profile.sample_multiplier` (entrer `1-4`).
  La valeur peut être un numéro/chaîne unique ou un objet avec remplacement par profil,
  par exemple `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts` : structure utilisée par le générateur d'annonces pour compléter les champs
  `ProviderAdvertV1` (pointeur d'enjeu, conseils de QoS, sujets). Si vous omettez le
  nodo usa defaults del registro de gobernanza.

Plomberie de configuration :- `[sorafs.storage]` se définit en `iroha_config` comme `SorafsStorage` et se charge
  à partir du fichier de configuration du nœud.
- `iroha_core` et `iroha_torii` pasan la configuration de stockage ici
  constructeur de la passerelle et du magasin de morceaux dans l'arranque.
- Existence de remplacements pour dev/test (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), mais
  Les programmes de production doivent être basés sur le fichier de configuration.

### Utilisations de CLI

Mettre la surface HTTP de Torii à la fin du câble, la caisse
`sorafs_node` inclut une CLI liviana pour que les opérateurs puissent
Automatiser les exercices d'ingestion/exportation contre le backend persistant.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` attend un manifeste `.to` codifié en Norito et les octets de charge utile
  correspondants. Reconstruire le plan de morceau à partir du profil de
  chunking del manifest, impone paridad de digests, persiste archivos de chunk,
  et éventuellement émettre un blob JSON `chunk_fetch_specs` pour les outils
  validation du tracé en aval.
- `export` accepter un ID de manifeste et décrire le manifeste/charge utile enregistré en
  discothèque (avec plan JSON facultatif) pour que les appareils soient connectés aux reproductibles
  entre entornos.Les commandes permettent d'imprimer un CV Norito JSON sur la sortie standard, facilitant ainsi l'utilisation du
scripts. La CLI est couverte par une vérification d'intégration pour garantir que
les manifestes et les charges utiles se reconstituent correctement avant de les lire
API de Torii.【crates/sorafs_node/tests/cli.rs:1】

> Parité HTTP
>
> El gateway Torii expose désormais les assistants de lecture solo en réponse au mismo
> `NodeHandle` :
>
> - `GET /v2/sorafs/storage/manifest/{manifest_id_hex}` — développer le manifeste
> Norito sauvegardé (base64) avec résumé/métadonnées.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v2/sorafs/storage/plan/{manifest_id_hex}` — développer le plan de chunk
> Déterministe JSON (`chunk_fetch_specs`) pour l'outillage en aval.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> Ces points de terminaison reflètent la sortie de la CLI pour que les pipelines puissent être utilisés
> passer des scripts locaux et sondes HTTP sans modifier les analyseurs.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Cycle de vie du nœud1. **Arranque**:
   - Si le stockage est activé, le nœud initialise le magasin de morceaux avec
     le répertoire et la capacité configurés. Ceci inclut vérifier ou créer le
     base de données de manifestes PoR et reproduction de manifestes fijados para calentar
     caches.
   - Routes du registraire de la passerelle SoraFS (points de terminaison Norito JSON POST/GET pour la broche,
     chercher, muestreo PoR, télémétrie).
   - Lanzar le travailleur de muestreo PoR et le moniteur de cuotas.
2. **Découverte / Annonces** :
   - Générer les documents `ProviderAdvertV1` en utilisant la capacité/salud actuelle,
     firmarlos con la clave aprobada por el consejo et publicarlos via Discovery.
     Utilisez la nouvelle liste `profile_aliases` pour les poignées canoniques et
3. **Flujo de pin**:
   - La passerelle reçoit un manifeste ferme (incluyendo plan de chunk, raíz PoR,
     sociétés du conseil). Valider la liste des alias (`sorafs.sf1@1.0.0` requis)
     et assurez-vous que le plan de chunk coïncide avec les métadonnées du manifeste.
   - Vérifiez les comptes. Si la capacité/les limites des broches sont dépassées, elles répondent à un
     erreur de politique (Norito structuré).
   - Streamea datos de chunk hacia `ChunkStore`, vérification des résumés au cours de la
     ingestion. Actualiser les arbres PoR et stocker les métadonnées du manifeste dans le
     registre.
4. **Flux de récupération** :
   - Sirve sollicitudes de rango de chunk desde disco. Le planificateur impose`max_parallel_fetches` et `429` lorsque celui-ci est saturé.
   - Émettre une télémétrie structurée (Norito JSON) avec latence, octets servis et
     conteos de error para monitoreo en aval.
5. **Mustreo PoR** :
   - La sélection du travailleur se manifeste proportionnellement au peso (par exemple, octets
     almacenados) et ejecuta muestreo determinista en utilisant l'arbol PoR del chunk store.
   - Persistez les résultats pour les auditoires de gouvernement et incluez les résultats en
     annonces du fournisseur / points de terminaison de télémétrie.
6. **Expulsión / cumplimiento de cuotas** :
   - Lorsque la capacité du nœud retrace de nouvelles broches par défaut est atteinte.
     Optionnellement, les opérateurs peuvent configurer la politique d'expulsion
     (par exemple, TTL, LRU) une fois que le modèle de gouvernance est adopté ; por
     maintenant le design assume les restrictions et les opérations de désépinglage initiées par
     l'opérateur.

### Déclaration de capacité et intégration de planification- Torii retransmet maintenant les mises à jour de `CapacityDeclarationRecord` depuis
  `/v2/sorafs/capacity/declare` hacia el `CapacityManager` intégré, de modo que
  chaque fois, construisez une vue en mémoire de vos attributions compromises
  chunker et voie. Le manager expose des instantanés de lecture solo pour la télémétrie
  (`GET /v2/sorafs/capacity/state`) et applications réservées au profil ou à la voie avant
  accepter de nouvelles commandes.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- Le point final `/v2/sorafs/capacity/schedule` accepte les charges utiles `ReplicationOrderV1`
  émis par la gouvernance. Lorsque la commande est adressée au fournisseur local et au responsable
  réviser la planification dupliquée, vérifier la capacité du chunker/lane, réserver la
  Franja et développez un `ReplicationPlan` décrivant la capacité restante pour
  que les outils d’orquestación continuent à ingérer. Les ordonnances pour
  D'autres fournisseurs se reconnaissent avec une réponse `ignored` pour faciliter les flux
  multi-opérateur.【crates/iroha_torii/src/routing.rs:4845】
- Hooks de complétude (par exemple, disparados tras el éxito de la ingestión)
  Appelez le `POST /v2/sorafs/capacity/complete` pour libérer des réserves via
  `CapacityManager::complete_order`. La réponse inclut un instantané
  `ReplicationRelease` (totales restantes, résiduelles de chunker/lane) pour que
  les outils d'orquestación peuvent être insérés dans l'ordre suivant sans sondage.
  Travailler à l'avenir câblera ce pipeline du magasin de morceaux une fois que lelogique d'ingestion de l'aire.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- Le `TelemetryAccumulator` intégré peut être intermédiaire
  `NodeHandle::update_telemetry`, permettant aux travailleurs en deuxième plan
  Enregistrez les données PoR/uptime et éventuellement les charges utiles dérivées canoniques
  `CapacityTelemetryV1` ne touche pas l'intérieur du planificateur.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Intégrations et travail futur

- **Gouvernance** : extension `sorafs_pin_registry_tracker.md` avec télémétrie de
  stockage (tasa de éxito PoR, utilización de disco). La politique d'admission
  Vous pouvez avoir besoin d'une capacité minimale ou d'une capacité minimale d'exécution avant d'accepter
  publicités.
- **SDK client** : expose la nouvelle configuration de stockage (limites de
  disco, alias) pour que les outils de gestion puissent amorcer des nœuds
  par programmation.
- **Télémétrie** : intégrer la pile de données existantes (Prometheus /
  OpenTelemetry) pour que les paramètres de stockage puissent apparaître dans les tableaux de bord de
  observabilité.
- **Sécurité** : exécuter le module de stockage sur une piscine dédiée aux tâches
  async avec contre-pression et prise en compte du sandboxing des lectures de morceaux via
  io_uring o pools acotados de tokio para evitar que clientses maliciosos agoten
  récursifs.Cette conception maintient le module de stockage facultatif et déterminant à la
vez que otorga a los operadores les boutons nécessaires pour participer à la capa
SoraFS de disponibilité des données. Implémenter les changements dans `iroha_config`,
`iroha_core`, `iroha_torii` et la passerelle Norito, en plus de l'outillage publicitaire
du fournisseur.