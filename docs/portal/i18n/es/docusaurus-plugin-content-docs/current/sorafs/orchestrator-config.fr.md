---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: orchestrator-config
title: Configuration de l’orchestrateur SoraFS
sidebar_label: Configuration de l’orchestrateur
description: Configurer l’orchestrateur de fetch multi-source, interpreter les echecs et deboguer la telemetrie.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/developer/orchestrator.md`. Gardez les deux copies synchronisées jusqu’à ce que la documentation héritée soit retirée.
:::

# Guide de l’orchestrateur de fetch multi-source

L’orchestrateur de fetch multi-source de SoraFS pilote des téléchargements
parallèles et déterministes depuis l’ensemble de fournisseurs publié dans des
adverts soutenus par la gouvernance. Ce guide explique comment configurer
l’orchestrateur, quels signaux d’échec attendre pendant les rollouts et quels
flux de télémétrie exposent des indicateurs de santé.

## 1. Vue d’ensemble de la configuration

L’orchestrateur fusionne trois sources de configuration :

| Source | Objectif | Notes |
|--------|----------|-------|
| `OrchestratorConfig.scoreboard` | Normalise les poids des fournisseurs, valide la fraîcheur de la télémétrie et persiste le scoreboard JSON utilisé pour les audits. | Adossé à `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Applique des limites d’exécution (budgets de retry, bornes de concurrence, bascules de vérification). | Mappé à `FetchOptions` dans `crates/sorafs_car::multi_fetch`. |
| Paramètres CLI / SDK | Limite le nombre de pairs, attache des régions de télémétrie et expose les politiques de deny/boost. | `sorafs_cli fetch` expose ces flags directement ; les SDK les propagent via `OrchestratorConfig`. |

Les helpers JSON dans `crates/sorafs_orchestrator::bindings` sérialisent la
configuration complète en Norito JSON, la rendant portable entre bindings SDK et
automatisation.

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

Persistez le fichier via l’empilement habituel `iroha_config` (`defaults/`, user,
actual) afin que les déploiements déterministes héritent des mêmes limites sur
les nœuds. Pour un profil de repli direct-only aligné sur le rollout SNNet-5a,
consultez `docs/examples/sorafs_direct_mode_policy.json` et les indications
associées dans `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Overrides de conformité

SNNet-9 intègre la conformité pilotée par la gouvernance dans l’orchestrateur.
Un nouvel objet `compliance` dans la configuration Norito JSON capture les
carve-outs qui forcent le pipeline de fetch en mode direct-only :

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` déclare les codes ISO‑3166 alpha‑2 où opère cette
  instance de l’orchestrateur. Les codes sont normalisés en majuscules lors du
  parsing.
- `jurisdiction_opt_outs` reflète le registre de gouvernance. Lorsqu’une
  juridiction opérateur apparaît dans la liste, l’orchestrateur impose
  `transport_policy=direct-only` et émet la raison de fallback
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` liste les digests de manifest (CIDs masqués, encodés en
  hexadécimal majuscule). Les payloads correspondants forcent aussi une
  planification direct-only et exposent le fallback
  `compliance_blinded_cid_opt_out` dans la télémétrie.
- `audit_contacts` enregistre les URI que la gouvernance attend des opérateurs
  dans leurs playbooks GAR.
- `attestations` capture les paquets de conformité signés qui justifient la
  politique. Chaque entrée définit un `jurisdiction` optionnel (code ISO‑3166
  alpha‑2), un `document_uri`, le `digest_hex` canonique à 64 caractères, le
  timestamp d’émission `issued_at_ms`, et un `expires_at_ms` optionnel. Ces
  artefacts alimentent la checklist d’audit de l’orchestrateur afin que les
  outils de gouvernance puissent relier les overrides aux documents signés.

Fournissez le bloc de conformité via l’empilement habituel de configuration pour
que les opérateurs reçoivent des overrides déterministes. L’orchestrateur
applique la conformité _après_ les hints de write-mode : même si un SDK demande
`upload-pq-only`, les opt-outs de juridiction ou de manifest basculent toujours
vers le transport direct-only et échouent rapidement lorsqu’aucun fournisseur
conforme n’existe.

Les catalogues canonique d’opt-out résident dans
`governance/compliance/soranet_opt_outs.json` ; le Conseil de gouvernance publie
les mises à jour via des releases taggées. Un exemple de configuration complet
(incluant les attestations) est disponible dans
`docs/examples/sorafs_compliance_policy.json`, et le processus opérationnel est
capturé dans le
[playbook de conformité GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Paramètres CLI & SDK

| Flag / Champ | Effet |
|--------------|-------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Limite le nombre de fournisseurs qui passent le filtre du scoreboard. Mettez `None` pour utiliser tous les fournisseurs éligibles. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Plafonne les retries par chunk. Dépasser la limite déclenche `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Injecte des snapshots de latence/échec dans le constructeur du scoreboard. Une télémétrie périmée au-delà de `telemetry_grace_secs` rend les fournisseurs inéligibles. |
| `--scoreboard-out` | Persiste le scoreboard calculé (fournisseurs éligibles + inéligibles) pour inspection post-run. |
| `--scoreboard-now` | Surchage le timestamp du scoreboard (secondes Unix) pour garder les captures de fixtures déterministes. |
| `--deny-provider` / hook de politique de score | Exclut des fournisseurs de manière déterministe sans supprimer les adverts. Utile pour un blacklisting à réponse rapide. |
| `--boost-provider=name:delta` | Ajuste les crédits de round-robin pondéré d’un fournisseur sans toucher aux poids de gouvernance. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Étiquette les métriques et logs structurés pour que les dashboards puissent pivoter par géographie ou vague de rollout. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | Par défaut `soranet-first` maintenant que l’orchestrateur multi-source est la base. Utilisez `direct-only` lors d’un downgrade ou d’une directive de conformité, et réservez `soranet-strict` aux pilotes PQ-only ; les overrides de conformité restent le plafond dur. |

SoraNet-first est désormais le défaut livré, et les rollbacks doivent citer le
bloquant SNNet correspondant. Après la graduation de SNNet-4/5/5a/5b/6a/7/8/12/13,
la gouvernance durcira la posture requise (vers `soranet-strict`) ; d’ici là, les
seuls overrides motivés par incident doivent privilégier `direct-only`, et ils
sont à consigner dans le log de rollout.

Tous les flags ci-dessus acceptent la syntaxe `--` dans `sorafs_cli fetch` et le
binaire orienté développeurs `sorafs_fetch`. Les SDK exposent les mêmes options
via des builders typés.

### 1.4 Gestion du cache des guards

La CLI câble désormais le sélecteur de guards SoraNet pour permettre aux
opérateurs d’épingler les relays d’entrée de manière déterministe avant le
rollout complet du transport SNNet-5. Trois nouveaux flags contrôlent ce flux :

| Flag | Objectif |
|------|----------|
| `--guard-directory <PATH>` | Pointe vers un fichier JSON décrivant le consensus de relays le plus récent (sous-ensemble ci-dessous). Passer le directory rafraîchit le cache des guards avant l’exécution du fetch. |
| `--guard-cache <PATH>` | Persiste le `GuardSet` encodé en Norito. Les exécutions suivantes réutilisent le cache même sans nouveau directory. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Overrides optionnels pour le nombre de guards d’entrée à épingler (par défaut 3) et la fenêtre de rétention (par défaut 30 jours). |
| `--guard-cache-key <HEX>` | Clé optionnelle de 32 octets utilisée pour taguer les caches de guard avec un MAC Blake3 afin de vérifier le fichier avant réutilisation. |

Les payloads du directory de guards utilisent un schéma compact :

Le flag `--guard-directory` attend désormais un payload `GuardDirectorySnapshotV2`
encodé en Norito. Le snapshot binaire contient :

- `version` — version du schéma (actuellement `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  métadonnées de consensus devant correspondre à chaque certificat intégré.
- `validation_phase` — gate de politique de certificats (`1` = autoriser une
  seule signature Ed25519, `2` = préférer les doubles signatures, `3` = exiger
  des doubles signatures).
- `issuers` — émetteurs de gouvernance avec `fingerprint`, `ed25519_public` et
  `mldsa65_public`. Les fingerprints sont calculés comme
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — une liste de bundles SRCv2 (sortie
  `RelayCertificateBundleV2::to_cbor()`). Chaque bundle inclut le descripteur du
  relay, les flags de capacité, la politique ML-KEM et les signatures doubles
  Ed25519/ML-DSA-65.

La CLI vérifie chaque bundle contre les clés d’émetteurs déclarées avant de
fusionner le directory avec le cache de guards. Les esquisses JSON héritées ne
sont plus acceptées ; les snapshots SRCv2 sont requis.

Appelez la CLI avec `--guard-directory` pour fusionner le consensus le plus
récent avec le cache existant. Le sélecteur conserve les guards épinglés encore
valides dans la fenêtre de rétention et éligibles dans le directory ; les
nouveaux relays remplacent les entrées expirées. Après un fetch réussi, le cache
mis à jour est réécrit dans le chemin fourni via `--guard-cache`, gardant les
sessions suivantes déterministes. Les SDK reproduisent le même comportement en
appelant `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
et en injectant le `GuardSet` résultant dans `SorafsGatewayFetchOptions`.

`ml_kem_public_hex` permet au sélecteur de prioriser les guards capables PQ
pendant le rollout SNNet-5. Les toggles d’étape (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) dégradent désormais automatiquement les
relays classiques : lorsqu’un guard PQ est disponible, le sélecteur supprime les
pins classiques en trop afin que les sessions suivantes favorisent les
handshakes hybrides. Les résumés CLI/SDK exposent le mix résultant via
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` et les champs associés de candidats/déficit/delta de
supply, rendant explicites les brownouts et fallbacks classiques.

Les directories de guards peuvent désormais embarquer un bundle SRCv2 complet via
`certificate_base64`. L’orchestrateur décode chaque bundle, revalide les
signatures Ed25519/ML-DSA et conserve le certificat analysé aux côtés du cache
de guards. Lorsqu’un certificat est présent, il devient la source canonique pour
les clés PQ, les préférences de handshake et la pondération ; les certificats
expirés sont écartés et le sélecteur revient aux champs hérités du descripteur.
Les certificats se propagent dans la gestion du cycle de vie des circuits et
sont exposés via `telemetry::sorafs.guard` et `telemetry::sorafs.circuit`, qui
consignent la fenêtre de validité, les suites de handshake et l’observation ou
non de doubles signatures pour chaque guard.

Utilisez les helpers CLI pour garder les snapshots alignés avec les éditeurs :

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` télécharge et vérifie le snapshot SRCv2 avant de l’écrire sur disque,
tandis que `verify` rejoue le pipeline de validation pour des artefacts issus
d’autres équipes, en émettant un résumé JSON qui reflète la sortie du sélecteur
de guards CLI/SDK.

### 1.5 Gestionnaire du cycle de vie des circuits

Lorsque le directory de relays et le cache de guards sont fournis, l’orchestrateur
active le gestionnaire du cycle de vie des circuits pour pré-construire et
renouveler des circuits SoraNet avant chaque fetch. La configuration se trouve
sous `OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) via deux
nouveaux champs :

- `relay_directory`: transporte le snapshot du directory SNNet-3 pour que les
  sauts middle/exit soient sélectionnés de manière déterministe.
- `circuit_manager`: configuration optionnelle (activée par défaut) contrôlant le
  TTL des circuits.

Norito JSON accepte désormais un bloc `circuit_manager` :

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Les SDK transmettent les données de directory via
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), et la CLI le câble automatiquement dès que
`--guard-directory` est fourni (`crates/iroha_cli/src/commands/sorafs.rs:365`).

Le gestionnaire renouvelle les circuits lorsque les métadonnées de guard changent
(endpoint, clé PQ ou timestamp d’épinglage) ou lorsque le TTL expire. Le helper
`refresh_circuits` invoqué avant chaque fetch (`crates/sorafs_orchestrator/src/lib.rs:1346`)
émet des logs `CircuitEvent` pour que les opérateurs puissent tracer les décisions
liées au cycle de vie. Le soak test
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) démontre une latence stable
sur trois rotations de guards ; voir le rapport associé dans
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Proxy QUIC local

L’orchestrateur peut optionnellement lancer un proxy QUIC local afin que les
extensions navigateur et les adaptateurs SDK n’aient pas à gérer les certificats
ni les clés du cache de guards. Le proxy se lie à une adresse loopback, termine
les connexions QUIC et renvoie un manifest Norito décrivant le certificat et la
clé de cache de guard optionnelle au client. Les événements de transport émis par
le proxy sont comptés via `sorafs_orchestrator_transport_events_total`.

Activez le proxy via le nouveau bloc `local_proxy` dans le JSON de l’orchestrateur :

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
```

- `bind_addr` contrôle l’adresse d’écoute du proxy (utilisez le port `0` pour
  demander un port éphémère).
- `telemetry_label` se propage aux métriques pour que les dashboards distinguent
  les proxies des sessions de fetch.
- `guard_cache_key_hex` (optionnel) permet au proxy d’exposer le même cache de
  guards à clé que les CLI/SDK, gardant les extensions navigateur alignées.
- `emit_browser_manifest` bascule le retour d’un manifest que les extensions
  peuvent stocker et valider.
- `proxy_mode` choisit si le proxy relaie le trafic localement (`bridge`) ou
  s’il n’émet que des métadonnées pour que les SDK ouvrent eux-mêmes des circuits
  SoraNet (`metadata-only`). Le proxy est par défaut en `bridge` ; choisissez
  `metadata-only` quand un poste doit exposer le manifest sans relayer les flux.
- `prewarm_circuits`, `max_streams_per_circuit` et `circuit_ttl_hint_secs`
  exposent des hints supplémentaires au navigateur afin de budgéter les flux
  parallèles et comprendre le niveau de réutilisation des circuits.
- `car_bridge` (optionnel) pointe vers un cache local d’archives CAR. Le champ
  `extension` contrôle le suffixe ajouté lorsque la cible omet `*.car` ; définissez
  `allow_zst = true` pour servir directement des payloads `*.car.zst` pré-compressés.
- `kaigi_bridge` (optionnel) expose les routes Kaigi spoolées au proxy. Le champ
  `room_policy` annonce si le bridge fonctionne en mode `public` ou
  `authenticated` afin que les clients navigateur pré-sélectionnent les labels
  GAR appropriés.
- `sorafs_cli fetch` expose les overrides `--local-proxy-mode=bridge|metadata-only`
  et `--local-proxy-norito-spool=PATH`, permettant de basculer le mode d’exécution
  ou de pointer vers des spools alternatifs sans modifier la politique JSON.
- `downgrade_remediation` (optionnel) configure le hook de downgrade automatique.
  Lorsqu’il est activé, l’orchestrateur surveille la télémétrie des relays pour
  détecter des rafales de downgrade et, après dépassement du `threshold` dans la
  fenêtre `window_secs`, force le proxy local vers `target_mode` (par défaut
  `metadata-only`). Une fois les downgrades stoppés, le proxy revient au
  `resume_mode` après `cooldown_secs`. Utilisez le tableau `modes` pour limiter
  le déclencheur à des rôles de relay spécifiques (par défaut les relays d’entrée).

Quand le proxy tourne en mode bridge, il sert deux services applicatifs :

- **`norito`** – la cible de flux du client est résolue par rapport à
  `norito_bridge.spool_dir`. Les cibles sont sanitizées (pas de traversal, pas de
  chemins absolus), et lorsqu’un fichier n’a pas d’extension, le suffixe configuré
  est appliqué avant de streamer le payload tel quel vers le navigateur.
- **`car`** – les cibles de flux se résolvent dans `car_bridge.cache_dir`, héritent
  de l’extension par défaut configurée et rejettent les payloads compressés sauf
  si `allow_zst` est activé. Les bridges réussis répondent avec `STREAM_ACK_OK`
  avant de transférer les octets de l’archive afin que les clients puissent
  pipeline la vérification.

Dans les deux cas, le proxy fournit l’HMAC du cache-tag (quand une clé de cache
était présente lors du handshake) et enregistre les codes de raison de télémétrie
`norito_*` / `car_*` pour que les dashboards distinguent d’un coup d’œil les
succès, les fichiers manquants et les échecs de sanitisation.

`Orchestrator::local_proxy().await` expose le handle en cours pour que les
appelants puissent lire le PEM du certificat, récupérer le manifest navigateur
ou demander un arrêt gracieux à la fermeture de l’application.

Lorsque le proxy est activé, il sert désormais des enregistrements **manifest v2**.
En plus du certificat existant et de la clé de cache de guard, la v2 ajoute :

- `alpn` (`"sorafs-proxy/1"`) et un tableau `capabilities` pour que les clients
  confirment le protocole de flux à utiliser.
- Un `session_id` par handshake et un bloc de salage `cache_tagging` pour dériver
  des affinités de guard par session et des tags HMAC.
- Des hints de circuit et de sélection de guard (`circuit`, `guard_selection`,
  `route_hints`) afin que les intégrations navigateur exposent une UI plus riche
  avant l’ouverture des flux.
- `telemetry_v2` avec des knobs d’échantillonnage et de confidentialité pour
  l’instrumentation locale.
- Chaque `STREAM_ACK_OK` inclut `cache_tag_hex`. Les clients reflètent la valeur
  dans l’en-tête `x-sorafs-cache-tag` lors de requêtes HTTP ou TCP afin que les
  sélections de guard en cache restent chiffrées au repos.

Ces champs s’ajoutent au manifest v2 ; les clients doivent consommer
explicitement les clés qu’ils supportent et ignorer le reste.

## 2. Sémantique des échecs

L’orchestrateur applique des vérifications strictes de capacités et de budgets
avant de transférer le moindre octet. Les échecs tombent dans trois catégories :

1. **Échecs d’éligibilité (pré-vol).** Les fournisseurs sans capacité de plage,
   aux adverts expirés ou à la télémétrie périmée sont consignés dans l’artefact
   du scoreboard et omis de la planification. Les résumés CLI remplissent le
   tableau `ineligible_providers` avec les raisons afin que les opérateurs
   inspectent les dérives de gouvernance sans scraper les logs.
2. **Épuisement à l’exécution.** Chaque fournisseur suit les échecs consécutifs.
   Une fois `provider_failure_threshold` atteint, le fournisseur est marqué
   `disabled` pour le reste de la session. Si tous les fournisseurs deviennent
   `disabled`, l’orchestrateur renvoie
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Arrêts déterministes.** Les limites dures remontent sous forme d’erreurs
   structurées :
   - `MultiSourceError::NoCompatibleProviders` — le manifest requiert un span de
     chunks ou un alignement que les fournisseurs restants ne peuvent honorer.
   - `MultiSourceError::ExhaustedRetries` — le budget de retries par chunk a été
     consommé.
   - `MultiSourceError::ObserverFailed` — les observateurs downstream (hooks de
     streaming) ont rejeté un chunk vérifié.

Chaque erreur embarque l’index du chunk fautif et, lorsque disponible, la raison
finale d’échec du fournisseur. Traitez ces erreurs comme des bloqueurs de release
— les retries avec la même entrée reproduiront l’échec tant que l’advert, la
telemetrie ou la santé du fournisseur sous-jacent ne changent pas.

### 2.1 Persistance du scoreboard

Quand `persist_path` est configuré, l’orchestrateur écrit le scoreboard final
après chaque run. Le document JSON contient :

- `eligibility` (`eligible` ou `ineligible::<reason>`).
- `weight` (poids normalisé assigné pour ce run).
- métadonnées du `provider` (identifiant, endpoints, budget de concurrence).

Archivez les snapshots de scoreboard avec les artefacts de release afin que les
choix de blacklisting et de rollout restent auditables.

## 3. Télémétrie et débogage

### 3.1 Métriques Prometheus

L’orchestrateur émet les métriques suivantes via `iroha_telemetry` :

| Métrique | Labels | Description |
|---------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Gauge des fetches orchestrés en vol. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Histogramme enregistrant la latence de fetch de bout en bout. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Compteur des échecs terminaux (retries épuisés, aucun fournisseur, échec observateur). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Compteur des tentatives de retry par fournisseur. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Compteur des échecs de fournisseur au niveau session menant à la désactivation. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Compte des décisions de politique d’anonymat (tenue vs brownout) groupées par étape de rollout et raison de fallback. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Histogramme de la part de relays PQ dans le set SoraNet sélectionné. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Histogramme des ratios d’offre de relays PQ dans le snapshot du scoreboard. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Histogramme du déficit de politique (écart entre l’objectif et la part PQ réelle). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Histogramme de la part de relays classiques utilisée dans chaque session. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Histogramme des compteurs de relays classiques sélectionnés par session. |

Intégrez ces métriques dans les dashboards de staging avant d’activer les knobs
en production. La disposition recommandée reflète le plan d’observabilité SF-6 :

1. **Fetches actifs** — alerte si le gauge monte sans completions correspondantes.
2. **Ratio de retries** — avertit lorsque les compteurs `retry` dépassent les
   baselines historiques.
3. **Échecs fournisseurs** — déclenche les alertes pager quand un fournisseur
   passe `session_failure > 0` dans une fenêtre de 15 minutes.

### 3.2 Cibles de logs structurés

L’orchestrateur publie des événements structurés vers des cibles déterministes :

- `telemetry::sorafs.fetch.lifecycle` — marqueurs `start` et `complete` avec
  nombre de chunks, retries et durée totale.
- `telemetry::sorafs.fetch.retry` — événements de retry (`provider`, `reason`,
  `attempts`) pour l’analyse manuelle.
- `telemetry::sorafs.fetch.provider_failure` — fournisseurs désactivés pour
  erreurs répétées.
- `telemetry::sorafs.fetch.error` — échecs terminaux résumés avec `reason` et
  métadonnées optionnelles du fournisseur.

Acheminez ces flux vers le pipeline de logs Norito existant afin que la réponse
aux incidents ait une source unique de vérité. Les événements de cycle de vie
exposent le mix PQ/classique via `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` et leurs compteurs associés,
ce qui facilite le câblage des dashboards sans scraper les métriques. Pendant
les rollouts GA, bloquez le niveau de logs à `info` pour les événements de cycle
de vie/retry et utilisez `warn` pour les erreurs terminales.

### 3.3 Résumés JSON

`sorafs_cli fetch` et le SDK Rust renvoient un résumé structuré contenant :

- `provider_reports` avec les compteurs succès/échec et l’état de désactivation.
- `chunk_receipts` détaillant quel fournisseur a satisfait chaque chunk.
- les tableaux `retry_stats` et `ineligible_providers`.

Archivez le fichier de résumé pour déboguer les fournisseurs défaillants : les
receipts mappent directement aux métadonnées de logs ci-dessus.

## 4. Checklist opérationnelle

1. **Préparer la configuration en CI.** Lancez `sorafs_fetch` avec la
   configuration cible, passez `--scoreboard-out` pour capturer la vue
   d’éligibilité, et faites un diff avec le release précédent. Tout fournisseur
   inéligible inattendu bloque la promotion.
2. **Valider la télémétrie.** Assurez-vous que le déploiement exporte les
   métriques `sorafs.fetch.*` et les logs structurés avant d’activer les fetches
   multi-source pour les utilisateurs. L’absence de métriques indique souvent
   que la façade de l’orchestrateur n’a pas été appelée.
3. **Documenter les overrides.** Lors d’un `--deny-provider` ou `--boost-provider`
   d’urgence, consignez le JSON (ou l’invocation CLI) dans le changelog. Les
   rollbacks doivent révoquer l’override et capturer un nouveau snapshot de
   scoreboard.
4. **Relancer les smoke tests.** Après modification des budgets de retry ou des
   caps de fournisseurs, refetcher la fixture canonique
   (`fixtures/sorafs_manifest/ci_sample/`) et vérifiez que les receipts de chunks
   restent déterministes.

Suivre les étapes ci-dessus garde le comportement de l’orchestrateur
reproductible dans les rollouts en phases et fournit la télémétrie nécessaire à
la réponse aux incidents.

### 4.1 Overrides de politique

Les opérateurs peuvent épingler la phase de transport/anonymat active sans
modifier la configuration de base en définissant
`policy_override.transport_policy` et `policy_override.anonymity_policy` dans
leur JSON `orchestrator` (ou en fournissant
`--transport-policy-override=` / `--anonymity-policy-override=` à
`sorafs_cli fetch`). Lorsqu’un override est présent, l’orchestrateur saute le
fallback brownout habituel : si le niveau PQ demandé ne peut pas être satisfait,
le fetch échoue avec `no providers` au lieu de dégrader silencieusement. Le
retour au comportement par défaut consiste simplement à vider les champs
d’override.
