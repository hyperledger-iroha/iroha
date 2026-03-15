---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/node-storage.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: node-storage
כותרת: Conception du stockage du nœud SoraFS
sidebar_label: Conception du stockage du nœud
תיאור: Architecture de stockage, quotas et hooks de cycle de vie pour les nœuds Torii hébergeant des données SoraFS.
---

:::הערה מקור קנוניק
Cette page reflète `docs/source/sorafs/sorafs_node_storage.md`. Gardez les deux copies Syncées jusqu'au retrait de la documentation Sphinx historique.
:::

## Conception du stockage du nœud SoraFS (ברויון)

ציין הערה מדויקת un nœud Iroha (Torii) peut s'incrire dans la couche
זמינות de données SoraFS et réserver une partie du disque local pour
stocker et servir des chunks. Elle complète la specification de discovery
`sorafs_node_client_protocol.md` et le travail de fixtures SF-1b en détaillant
l'architecture côté stockage, les contrôles de ressources et la plomberie de
תצורה qui doivent arriver dans les chemins de code du nœud et du gateway.
Les drills opérateurs se trouvent dans le
[Runbook d'operations du nœud](./node-operations).

### אובייקטים

- Permettre à tout validateur ou processus Iroha auxiliaire d'exposer du disque
  זמין עבור ספק SoraFS ללא משפיע על אחריותו של ספר החשבונות.
- Garder le module de stockage déterministe et piloté par Norito : מניפסטים,
  תוכניות של נתח, racines Proof-of-trievability (PoR) ופרסומות של ספק
  sont la source de vérité.
- Appliquer des quotas définis par l'opérateur afin qu'un nœud n'épuise pas ses
  propres ressources en acceptant trop de demandes de pin ou de fetch.
- Exposer la santé/la télémétrie (échantillonnage PoR, latence de fetch de chunk,
  pression disque) à la gouvernance et aux clients.

### אדריכלות ברמה גבוהה

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

מודולים כוללים:- **שער** : חשוף את נקודות הקצה של HTTP Norito pour les propositions de pin,
  les requêtes de fetch de chunks, l'échantillonnage PoR et la télémétrie. Il
  valide les payloads Norito et achemine les requêtes vers le chunk store. השתמש מחדש
  la pile HTTP Torii existante pour éviter un nouveau daemon.
- **רישום סיכות** : l'état des pins de manifest suivi dans `iroha_data_model::sorafs`
  et `iroha_core`. Lorsqu'un manifest est accepté, le registre enregistre le digest
  du manifest, le digest du plan de chunk, la racine PoR et les flags de capacité du
  ספק.
- **אחסון נתחים**: יישום `ChunkStore` sur disque qui ingère des manifests
  signnés, matérialise les plans de chunk דרך `ChunkProfile::DEFAULT`, et persiste
  les chunks selon un layout déterministe. צ'אק chunk est associé à un טביעת אצבע
  de contenu et à des métadonnées PoR afin que l'échantillonnage puisse revalider
  sans relire le fichier complet.
- **מכסה/מתזמן**: להטיל תצורות מוגבלות להפעלה (בייט דיסק מקסימום,
  pins en attente max, fetches parallèles max, TTL de chunk) et coordonne l'IO pour que
  les tâches Ledger ne soient pas affamées. Le scheduler est également responsable du
  service des preuves PoR et des requêtes d'échantillonnage med CPU borné.

### תצורה

Ajoutez une nouvelle section à `iroha_config` :

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # tag optionnel lisible
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled` : החלפת השתתפות. Quand false, le gateway renvoie 503 pour les
  נקודות קצה de storage et le nœud ne s'annonce pas en Discovery.
- `data_dir` : répertoire racine des données de chunk, arbres PoR et télémétrie de
  להביא. Défaut `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes` : מוגבל לתקופה של כוסות פינצ'ס. Une tâche de fond
  rejette les nouveaux pins quand la limite est atteinte.
- `max_parallel_fetches` : plafond de concurrence imposé par le scheduler pour
  équilibrer bande passante/IO disk avec la charge du validateur.
- `max_pins` : nombre maximum de pins de manifest que le nœud accepte avant d'appliquer
  פינוי/לחץ לאחור.
- `por_sample_interval_secs` : cadence des jobs d'échantillonnage PoR automatiques. עבודת צ'אק
  échantillonne `N` feuilles (ניתן להגדרה par Manifest) et émet des événements de télémétrie.
  La gouvernance peut scaler `N` de manière déterministe via la clé de métadonnées
  `profile.sample_multiplier` (Entier `1-4`). La valeur peut être un nombre/une chaîne ייחודי
  ou un objet avec מבטל את הפרופיל, למשל `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts` : מבנה שימושי לפרסומות ג'נראטורים לשחזר את האלופים
  `ProviderAdvertV1` (מצביע הימור, רמזים ל-QoS, נושאים). אם אין, לא להשתמש ב-les
  ברירות מחדל du registre de governance.

Plomberie de Configuration:- `[sorafs.storage]` est défini dans `iroha_config` comme `SorafsStorage` et charge
  depuis le fichier de config du nœud.
- `iroha_core` et `iroha_torii` transmettent la config de storage au Builder Gateway
  et au chunk store au démarrage.
- זה עוקף מפתח/בדיקה קיים (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), מאי
  les déploiements de production doivent s'appuyer sur le fichier de config.

### Utilitaires CLI

Alors que la surface HTTP Torii הוא הדרן en cours de câblage, le crate
`sorafs_node` embarque une CLI fine pour que les opérateurs puissent scripter des
תרגילים לבליעה/ייצוא לעומת קצה אחורי מתמיד.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` להשתתף במניפסט `.to` קידוד Norito ו-les bytes de pay associés.
  איל לבנות מחדש את תוכנית הנתח, להטיל את פרופיל החתיכה
  la parité des digests, persiste les fichiers de chunk, et émet en option un blob
  JSON `chunk_fetch_specs` יאפשר לך למצוא את הפריסה במורד הזרם.
- `export` קבל un ID de manifest et écrit le manifest/loadload stocké sur disk
  (avec plan JSON optionnel) pour que les fixtures reproductibles.

Les deux commandes impriment un résumé Norito JSON sur stdout, ce qui facilite le
piping dans des scripts. La CLI est couverte par un test d'intégration pour garantir
que manifests et payloads se תיקון הלוך ושוב avant l'arrivée des APIs Torii.【crates/sorafs_node/tests/cli.rs:1】

> Parité HTTP
>
> Le gateway Torii לחשוף את désormais des helpers en lecture seule basés sur le même
> `NodeHandle` :
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — renvoie le manifest
> Norito stocké (base64) avec digest/métadonnées.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — renvoie le plan de chunk
> déterministe JSON (`chunk_fetch_specs`) pour les outils במורד הזרם.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> נקודות קצה Ces reflètent la sortie CLI afin que les צינורות puissent passer
> des scripts locaux aux probes HTTP sans changer de parseurs.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Cycle de vie du nœud1. **נישואין**:
   - Si le stockage est activé, le nœud initialise le chunk store avec le répertoire
     et la capacité configurés. Cela inclut la vérification ou la creation de la base
     de données de manifest PoR et le replay des manifests pinés pour chauffer les caches.
   - רושם את המסלולים של השער SoraFS (נקודות קצה Norito JSON POST/GET צור סיכה,
     להביא, échantillonnage PoR, télémétrie).
   - Lancer le worker d'échantillonnage PoR et le moniteur de quotas.
2. **גילוי / פרסומות**:
   - Générer des documents `ProviderAdvertV1` avec la capacité/santé courante, les signer
     avec la clé approuvée par le conseil, et publier via le canal de discovery.
     Utiliser la nouvelle list `profile_aliases` pour garder les handles canoniques
3. **זרימת עבודה דה pin**:
   - Le gateway reçoit un manifest signné (כולל תוכנית נתח, racine PoR, חתימות
     du conseil). Valider la list d'alias (`sorafs.sf1@1.0.0` דרוש) et s'assurer que
     le plan de chunk correspond aux métadonnées du manifest.
   - Verifier les quotas. Si les limites de capacité/pins seraient dépassées, répondre
     par une erreur de politique (Norito structuré).
   - Streamer les données de chunk dans `ChunkStore` en vérifiant les digests à l'ingestion.
     Mettre à jour les arbres PoR et stocker les métadonnées du manifest dans le registre.
4. **שליפה של זרימת עבודה**:
   - Servir les requêtes de range de chunk depuis le disque. להטיל מתזמן
     `max_parallel_fetches` et renvoie `429` en cas de saturation.
   - Émettre une télémétrie structurée (Norito JSON) עם השהיה, שירות בתים ו
     מתחברים לניטור במורד הזרם.
5. **Échantillonnage PoR**:
   - Le worker sélectionne les manifests proportionnelle au poids (לדוגמה bytes stockés)
     et exécute un échantillonnage déterministe דרך l'arbre PoR du chunk store.
   - Persister les résultats pour les audits de governance et inclure des résumés dans
     les ספק פרסומות / נקודות קצה de télémétrie.
6. **Eviction / Application des quotas** :
   - Quand la capacité est atteinte, le nœud rejette les nouveaux pins par défaut. En
     אופציה, מפעילי התפעול של מתכנת הפוליטיקה (לדוגמה TTL, LRU)
     une fois le modèle de governance défini; pour l'instant, le design suppose des
     quotas stricts et des operations d'unpin initiées par l'operateur.

### Declaration de capacité et intégration du scheduling- Torii relaie désormais les mises à jour `CapacityDeclarationRecord` depuis `/v1/sorafs/capacity/declare`
  vers le `CapacityManager` embarqué, de sorte que chaque nœud construit une vue en mémoire de ses
  הקצאות chunker/lane engagées. המנהל חושף את התמונות לקריאה בלבד pour la télémétrie
  (`GET /v1/sorafs/capacity/state`) et applique des réservations par profil ou par lane avant que de
  nouvelles commandes ne soient acceptées.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- L'endpoint `/v1/sorafs/capacity/schedule` accepte des payloads `ReplicationOrderV1` émis par la governance.
  Lorsque l'ordre cible le provider local, le manager vérifie la planification en doublon, valide la
  נתיב/נתיב קיבולת, רזרב לה נתון, et renvoie un `ReplicationPlan` décrivant la capacité restante
  afin que les outils d'orchestration puissent poursuivre l'ingestion. Les ordres pour d'autres ספקים
  sont acquités avec une réponse `ignored` pour faciliter les workflows multi-operateurs.【crates/iroha_torii/src/routing.rs:4845】
- מערער Des hooks de complétion (par ex. déclenchés après succès d'ingestion)
  `POST /v1/sorafs/capacity/complete` pour liberer les réservations via `CapacityManager::complete_order`.
  התגובה כוללת תמונת מצב `ReplicationRelease` (מנוחה מוחלטת, נתיב/נתיב רזידואלים)
  les outils d'orchestration puissent queue la commande suivante sans polling. Un travail futur reliera
  cela au pipeline de chunk store lorsque la logique d'ingestion sera prête.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- Le `TelemetryAccumulator` embarqué peut être muté via `NodeHandle::update_telemetry`, permettant aux
  workers de fond d'enregistrer des échantillons PoR/uptime et de dériver ensuite des payloads canoniques
  `CapacityTelemetryV1` sans toucher aux internes du scheduler.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### אינטגרציות ועבודות עתידיות

- **ממשל** : étendre `sorafs_pin_registry_tracker.md` avec la télémétrie de stockage
  (taux de succès PoR, דיסק ניצול). Les politiques d'admission peuvent exiger une capacité
  minimale ou un taux de succès PoR פרסומות מינימליות.
- **לקוחות SDK** : חשיפת תצורת אחסון חדשה (מגביל את הדיסק, כינוי) pour que
  les outils de gestion puissent bootstrapper les nœuds par program.
- **Télémétrie** : intégrer avec la stack de métriques existante (Prometheus /
  OpenTelemetry) ניתן להבחין במכשירי אחסון בלוחות המחוונים.
- **Sécurité**: מנהל מודול אחסון בבריכה אסינכרון עם לחץ אחורי
  et envisager le sandboxing des lectures de chunks via io_uring ou des pools tokio bornés pour empêcher
  des clients malveillants d'épuiser les ressources.Ce design maintient le module de storage optionnel et déterministe tout en donnant aux opérateurs les
boutons nécessaires pour participer à la couche de disponibilité des données SoraFS. יישום בן
impliquera des changements dans `iroha_config`, `iroha_core`, `iroha_torii` et le gateway Norito, ainsi
que les outils d'advert de provider.