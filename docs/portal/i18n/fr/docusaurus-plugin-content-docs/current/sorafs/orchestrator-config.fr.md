---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : configuration de l'orchestrateur
titre : Configuration de l’orchestrateur SoraFS
sidebar_label : Configuration de l'orchestrateur
description : Configurer l’orchestrateur de fetch multi-source, interpréter les echecs et déboguer la télémétrie.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/developer/orchestrator.md`. Conservez les deux copies synchronisées jusqu’à ce que la documentation soit retirée.
:::

# Guide de l’orchestrateur de fetch multi-source

L’orchestrateur de fetch multi-source de SoraFS pilote des téléchargements
parallèles et déterministes depuis l’ensemble de fournisseurs publié dans des
annonces soutenues par la gouvernance. Ce guide explique comment configurer les commentaires
l’orchestrateur, quels signaux d’échec attendre pendant les déploiements et quels
flux de télémétrie exposant des indicateurs de santé.

## 1. Vue d'ensemble de la configuration

L’orchestrateur fusionne trois sources de configuration :

| Source | Objectif | Remarques |
|--------|----------|-------|
| `OrchestratorConfig.scoreboard` | Normaliser le poids des fournisseurs, valider la fraîcheur de la télémétrie et conserver le tableau de bord JSON utilisé pour les audits. | Adossé à `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Applique des limites d’exécution (budgets de retry, bornes de concurrence, bascules de vérification). | Mappé à `FetchOptions` dans `crates/sorafs_car::multi_fetch`. |
| Paramètres CLI / SDK | Limitez le nombre de paires, attachez des régions de télémétrie et exposez les politiques de deny/boost. | `sorafs_cli fetch` expose directement ces drapeaux ; les SDK les propagent via `OrchestratorConfig`. |

Les helpers JSON dans `crates/sorafs_orchestrator::bindings` sérialisent la
configuration complète en Norito JSON, la rendant portable entre liaisons SDK et
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
actual) afin que les déployés déterministes héritent des mêmes limites sur
les noeuds. Pour un profil de réponse direct-only aligné sur le déploiement SNNet-5a,
consulter `docs/examples/sorafs_direct_mode_policy.json` et les indications
associées dans `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Dérogations de conformité

SNNet-9 intègre la conformité pilotée par la gouvernance dans l’orchestrateur.
Un nouvel objet `compliance` dans la configuration Norito Fichiers de capture JSON
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
```- `operator_jurisdictions` déclare les codes ISO‑3166 alpha‑2 où opère cette
  instance de l’orchestrateur. Les codes sont normalisés en majuscules lors du
  analyse.
- `jurisdiction_opt_outs` reflète le registre de gouvernance. Lorsqu'une
  juridiction opérateur apparaît dans la liste, l’orchestrateur impose
  `transport_policy=direct-only` et émet la raison de repli
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` liste les digests de manifest (CIDs masqués, encodés en
  majuscule hexadécimal). Les charges utiles correspondantes forcent aussi une
  planification direct-only et exposant le fallback
  `compliance_blinded_cid_opt_out` dans la télémétrie.
- `audit_contacts` enregistre les URI que la gouvernance attend des opérateurs
  dans leurs playbooks GAR.
- `attestations` capture les paquets de conformité signés qui justifient la
  politique. Chaque entrée définit un `jurisdiction` optionnel (code ISO‑3166
  alpha‑2), un `document_uri`, le `digest_hex` canonique à 64 caractères, le
  timestamp d’émission `issued_at_ms`, et un `expires_at_ms` optionnel. Ces
  artefacts alimentent la checklist d’audit de l’orchestrateur afin que les
  les outils de gouvernance peuvent relier les dérogations aux documents signés.

Fournissez le bloc de conformité via l’empilement habituel de configuration pour
que les opérateurs reçoivent des dérogations déterministes. L'orchestrateur
applique la conformité _après_ les conseils de write-mode : même si un SDK demande
`upload-pq-only`, les opt-outs de juridiction ou de manifeste basculent toujours
vers le transport direct-only et échouent rapidement lorsqu’aucun fournisseur
conforme n’existe pas.

Les catalogues canoniques d’opt-out résident dans
`governance/compliance/soranet_opt_outs.json` ; le Conseil de gouvernance publique
les mises à jour via des releases taggées. Un exemple de configuration complet
(incluant les attestations) est disponible dans
`docs/examples/sorafs_compliance_policy.json`, et le processus opérationnel est
capturé dans le
[playbook de conformité GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Paramètres CLI & SDK| Drapeau / Champion | Effets |
|--------------|-------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Limitez le nombre de fournisseurs qui passent le filtre du scoreboard. Mettez `None` pour utiliser tous les fournisseurs éligibles. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Plafonne les tentatives par chunk. Dépasser la limite déclenchée `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Injectez des instantanés de latence/échec dans le constructeur du scoreboard. Une télémétrie périmée au-delà de `telemetry_grace_secs` rend les fournisseurs inéligibles. |
| `--scoreboard-out` | Persistez le scoreboard calculé (fournisseurs éligibles + inéligibles) pour l’inspection post-run. |
| `--scoreboard-now` | Surchage du timestamp du scoreboard (secondes Unix) pour garder les captures de luminaires déterministes. |
| `--deny-provider` / crochet de politique de score | Exclure les fournisseurs de manière déterministe sans supprimer les publicités. Utile pour un blacklisting à réponse rapide. |
| `--boost-provider=name:delta` | Ajustez les crédits de round-robin pondérés d’un fournisseur sans toucher au poids de gouvernance. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Étiquette les métriques et logs structurés pour que les tableaux de bord puissent pivoter par géographie ou vague de déploiement. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | Par défaut `soranet-first` maintenant que l’orchestrateur multi-source est la base. Utilisez `direct-only` lors d’un downgrade ou d’une directive de conformité, et réservez `soranet-strict` aux pilotes PQ-only ; les dérogations de conformité restent le plafond dur. |

SoraNet-first est désormais le défaut livré, et les rollbacks doivent citer le
bloquant le correspondant SNNet. Après la graduation de SNNet-4/5/5a/5b/6a/7/8/12/13,
la gouvernance durcira la posture requise (vers `soranet-strict`) ; d'ici là, les
seuls les overrides motivés par incident doivent privilégier `direct-only`, et ils
sont à consigner dans le journal de déploiement.

Tous les flags ci-dessus acceptent la syntaxe `--` dans `sorafs_cli fetch` et le
binaire orienté développeurs `sorafs_fetch`. Les SDK exposent les mêmes options
via des constructeurs typés.

### 1.4 Gestion du cache des gardes

La CLI câble désormais le sélecteur de gardes SoraNet pour permettre aux
opérateurs d’épingler les relais d’entrée de manière déterministe avant le
rollout complet du transport SNNet-5. Trois nouveaux drapeaux contrôlent ce flux :| Drapeau | Objectif |
|------|----------|
| `--guard-directory <PATH>` | Pointe vers un fichier JSON décrivant le consensus de relays le plus récent (sous-ensemble ci-dessous). Passer le répertoire rafraîchit le cache des gardes avant l’exécution du fetch. |
| `--guard-cache <PATH>` | Persistez le `GuardSet` encodé en Norito. Les exécutions suivantes réutilisent le cache même sans nouveau répertoire. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Overrides optionnels pour le nombre de gardes d’entrée à épingler (par défaut 3) et la fenêtre de rétention (par défaut 30 jours). |
| `--guard-cache-key <HEX>` | Clé optionnelle de 32 octets utilisée pour taguer les caches de garde avec un MAC Blake3 afin de vérifier le fichier avant réutilisation. |

Les charges utiles du répertoire de gardes utilisent un schéma compact :

Le flag `--guard-directory` attend désormais un payload `GuardDirectorySnapshotV2`
encodé en Norito. Le snapshot binaire contient :

- `version` — version du schéma (actuellement `2`).
-`directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  les métadonnées de consensus devant correspondent à chaque certificat intégré.
- `validation_phase` — gate de politique de certificats (`1` = autoriser une
  seule signature Ed25519, `2` = préférer les doubles signatures, `3` = exiger
  des doubles signatures).
- `issuers` — émetteurs de gouvernance avec `fingerprint`, `ed25519_public` et
  `mldsa65_public`. Les empreintes digitales sont calculées comme
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — une liste de bundles SRCv2 (sortie
  `RelayCertificateBundleV2::to_cbor()`). Chaque bundle inclut le descripteur du
  relais, les drapeaux de capacité, la politique ML-KEM et les signatures doubles
  Ed25519/ML-DSA-65.

La CLI vérifie chaque bundle contre les clés d’émetteurs déclarées avant de
fusionner le répertoire avec le cache de guards. Les esquisses JSON héritées ne
sont plus acceptées ; les instantanés SRCv2 sont requis.

Appelez la CLI avec `--guard-directory` pour fusionner le consensus le plus
récent avec le cache existant. Le sélecteur conserve les gardes épinglés encore
validé dans la fenêtre de rétention et éligibles dans le répertoire ; les
de nouveaux relais remplacent les entrées expirées. Après une récupération réussie, le cache
mis à jour est réécrit dans le chemin fourni via `--guard-cache`, gardant les
séances suivantes déterministes. Les SDK reproduisent le même comportement en
appelant `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
et en injectant le `GuardSet` résultant dans `SorafsGatewayFetchOptions`.`ml_kem_public_hex` permet au sélecteur de prioriser les gardes capables PQ
pendant le déploiement SNNet-5. Les bascules d'étape (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) dégradent désormais automatiquement les
relais classiques : lorsqu’un guard PQ est disponible, le sélecteur supprime les
pins classiques en trop afin que les sessions suivantes soulignent les
poignées de mains hybrides. Les résumés CLI/SDK exposent le mix résultant via
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` et les champs associés de candidats/déficit/delta de
supply, rendant explicites les baisses de tension et les replis classiques.

Les répertoires de gardes peuvent désormais embarquer un bundle SRCv2 complet via
`certificate_base64`. L’orchestrateur décode chaque bundle, revalide les
signatures Ed25519/ML-DSA et conserver le certificat analysé aux côtés du cache
des gardes. Lorsqu’un certificat est présent, il devient la source canonique pour
les clés PQ, les préférences de handshake et la pondération ; les certificats
expirés sont écartés et le sélecteur revient aux champs hérités du descripteur.
Les certificats se propagent dans la gestion du cycle de vie des circuits et
sont exposés via `telemetry::sorafs.guard` et `telemetry::sorafs.circuit`, qui
consignent la fenêtre de validité, les suites de handshake et l’observation ou
non de doubles signatures pour chaque garde.

Utilisez les helpers CLI pour garder les instantanés alignés avec les éditeurs :

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` télécharger et vérifier le snapshot SRCv2 avant de l’écrire sur disque,
tandis que `verify` rejoue le pipeline de validation pour des artefacts issus
d’autres équipes, en émettant un résumé JSON qui reflète la sortie du sélecteur
les gardes CLI/SDK.

### 1.5 Gestionnaire du cycle de vie des circuits

Lorsque le répertoire de relais et le cache de gardes sont fournis, l’orchestrateur
active le gestionnaire du cycle de vie des circuits pour pré-construire et
renouveler les circuits SoraNet avant chaque récupération. La configuration se trouve
sous `OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) via deux
nouveaux champions :

- `relay_directory` : transporte le snapshot du répertoire SNNet-3 pour que les
  sauts middle/exit soient sélectionnés de manière déterministe.
- `circuit_manager` : configuration optionnelle (activée par défaut) contrôlant le
  TTL des circuits.

Norito JSON accepte désormais un bloc `circuit_manager` :

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Les SDK transmettent les données de répertoire via
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), et la CLI le câble automatiquement dès que
`--guard-directory` est fourni (`crates/iroha_cli/src/commands/sorafs.rs:365`).Le gestionnaire renouvelle les circuits lorsque les métadonnées de garde changent
(endpoint, clé PQ ou timestamp d’épinglage) ou lorsque le TTL expire. L'assistant
`refresh_circuits` explorez avant chaque récupération (`crates/sorafs_orchestrator/src/lib.rs:1346`)
Émission des logs `CircuitEvent` pour que les opérateurs puissent tracer les décisions
liés au cycle de vie. Le test de trempage
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) démontre une latence stable
sur trois rotations de gardes ; voir le rapport associé dans
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Proxy QUIC local

L’orchestrateur peut optionnellement lancer un proxy QUIC local afin que les
extensions navigateur et les adaptateurs SDK n’étaient pas à gérer les certificats
ni les clés du cache de guards. Le proxy se trouve à une adresse loopback, termine
les connexions QUIC et renvoyer un manifest Norito décrivant le certificat et la
clé de cache de garde optionnelle au client. Les événements de transport émis par
le proxy est compté via `sorafs_orchestrator_transport_events_total`.

Activez le proxy via le nouveau bloc `local_proxy` dans le JSON de l'orchestrateur :

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
```- `bind_addr` contrôle l’adresse d’écoute du proxy (utilisez le port `0` pour
  demander un port éphémère).
- `telemetry_label` se propage aux métriques pour que les tableaux de bord distinguent
  les proxys des sessions de fetch.
- `guard_cache_key_hex` (optionnel) permet au proxy d'exposer le même cache de
  guards à clé que les CLI/SDK, gardant les extensions navigateurs alignées.
- `emit_browser_manifest` bascule le retour d'un manifeste que les extensions
  peuvent stocker et valider.
- `proxy_mode` choisit si le proxy relais le trafic localement (`bridge`) ou
  s’il n’émet que des métadonnées pour que les SDK ouvrent eux-mêmes des circuits
  SoraNet (`metadata-only`). Le proxy est par défaut en `bridge` ; choisir
  `metadata-only` quand un poste doit exposer le manifeste sans relayer les flux.
- `prewarm_circuits`, `max_streams_per_circuit` et `circuit_ttl_hint_secs`
  exposer des astuces supplémentaires au navigateur afin de budgéter les flux
  parallèles et comprendre le niveau de réutilisation des circuits.
- `car_bridge` (optionnel) pointe vers un cache local d'archives CAR. Le champion
  `extension` contrôle le suffixe ajouté lorsque la cible omet `*.car` ; défini
  `allow_zst = true` pour servir directement des payloads `*.car.zst` pré-compressés.
- `kaigi_bridge` (optionnel) expose les routes Kaigi spoolées au proxy. Le champion
  `room_policy` annonce si le pont fonctionne en mode `public` ou
  `authenticated` afin que les clients navigateur pré-sélectionnent les labels
  GAR approprié.
- `sorafs_cli fetch` expose les remplacements `--local-proxy-mode=bridge|metadata-only`
  et `--local-proxy-norito-spool=PATH`, permettant de basculer le mode d’exécution
  ou de pointer vers des spools alternatifs sans modifier la politique JSON.
- `downgrade_remediation` (optionnel) configure le hook de downgrade automatique.
  Lorsqu’il est activé, l’orchestrateur surveille la télémétrie des relais pour
  détecter des rafales de downgrade et, après dépassement du `threshold` dans la
  fenêtre `window_secs`, forcer le proxy local vers `target_mode` (par défaut
  `metadata-only`). Une fois les downgrades stoppés, le proxy revient au
  `resume_mode` après `cooldown_secs`. Utilisez le tableau `modes` pour limiteur
  le déclenchement à des rôles de relais spécifiques (par défaut les relais d’entrée).

Quand le proxy tourne en mode bridge, il sert deux services applicatifs :- **`norito`** – la cible de flux du client est résolue par rapport à
  `norito_bridge.spool_dir`. Les cibles sont aseptisées (pas de traversal, pas de
  chemins absolus), et lorsqu’un fichier n’a pas d’extension, le suffixe configuré
  est appliqué avant de streamer le payload tel quel vers le navigateur.
- **`car`** – les cibles de flux se résolvent dans `car_bridge.cache_dir`, héritent
  de l’extension par défaut configurée et rejettent les payloads compressés sauf
  si `allow_zst` est activé. Les ponts réussis répondent avec `STREAM_ACK_OK`
  avant de transférer les octets de l’archive afin que les clients puissent
  pipeline la vérification.

Dans les deux cas, le proxy fournit l’HMAC du cache-tag (quand une clé de cache
était présente lors du handshake) et enregistre les codes de raison de télémétrie
`norito_*` / `car_*` pour que les tableaux de bord distinguent d'un coup d'œil les
succès, les fichiers manquants et les échecs de sanitisation.

`Orchestrator::local_proxy().await` exposer le handle en cours pour que les
Les appelants peuvent lire le PEM du certificat, récupérer le manifeste du navigateur
ou demander un arrêt gracieux à la fermeture de l’application.

Lorsque le proxy est activé, il sert désormais des enregistrements **manifest v2**.
En plus du certificat existant et de la clé de cache de guard, la v2 ajoute :

- `alpn` (`"sorafs-proxy/1"`) et un tableau `capabilities` pour que les clients
  confirmez le protocole de flux à utiliser.
- Un `session_id` par handshake et un bloc de salage `cache_tagging` pour dériver
  des affinités de garde par session et des tags HMAC.
- Des conseils de circuit et de sélection de garde (`circuit`, `guard_selection`,
  `route_hints`) afin que les intégrations navigateur exposent une UI plus riche
  avant l’ouverture des flux.
- `telemetry_v2` avec des boutons d’échantillonnage et de confidentialité pour
  l’instrumentation locale.
- Chaque `STREAM_ACK_OK` inclut `cache_tag_hex`. Les clients retirent la valeur
  dans l'en-tête `x-sorafs-cache-tag` lors de requêtes HTTP ou TCP afin que les
  les sélections de garde en cache restent chiffrées au repos.

Ces champs s’ajoutent au manifest v2 ; les clients doivent consommer
précision les clés qu’ils soutiennent et ignorent le reste.

## 2. Sémantique des échecs

L’orchestrateur applique des vérifications strictes de capacités et de budgets
avant de transférer le moindre octet. Les échecs tombent dans trois catégories :1. **Échecs d’éligibilité (pré-vol).** Les fournisseurs sans capacité de plage,
   aux annonces expirées ou à la télémétrie périmée sont consignés dans l’artefact
   du scoreboard et omis de la planification. Les CV CLI remplissent le
   tableau `ineligible_providers` avec les raisons afin que les opérateurs
   inspectez les dérives de gouvernance sans gratter les logs.
2. **Épuisement à l’exécution.** Chaque fournisseur suit les échecs consécutifs.
   Une fois `provider_failure_threshold` atteint, le fournisseur est marqué
   `disabled` pour le reste de la session. Si tous les fournisseurs deviennent
   `disabled`, l'orchestrateur renvoie
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Arrêts déterministes.** Les limites dures remontent sous forme d’erreurs
   structurées :
   - `MultiSourceError::NoCompatibleProviders` — le manifeste requiert une durée de
     chunks ou un alignement que les fournisseurs restants ne peuvent honorer.
   - `MultiSourceError::ExhaustedRetries` — le budget de tentatives par morceau a été
     consommé.
   - `MultiSourceError::ObserverFailed` — les observateurs aval (crochets de
     streaming) ont rejeté un morceau vérifié.

Chaque erreur embarque l’index du chunk fautif et, lorsque disponible, la raison
finale d’échec du fournisseur. Traitez ces erreurs comme des bloqueurs de release
— les tentatives avec la même entrée reproduiront l’échec tant que l’annonce, la
la télémétrie ou la santé du fournisseur sous-jacent ne changent pas.

### 2.1 Persistance du tableau de bord

Quand `persist_path` est configuré, l’orchestrateur écrit le scoreboard final
après chaque course. Le document JSON contient :

- `eligibility` (`eligible` ou `ineligible::<reason>`).
- `weight` (poids normalisés assignés pour ce run).
- métadonnées du `provider` (identifiant, points finaux, budget de concurrence).

Archivez les snapshots de scoreboard avec les artefacts de release afin que les
le choix de blacklisting et de déploiement reste vérifiable.

## 3. Télémétrie et débogage

### 3.1 Métriques Prometheus

L’orchestrateur émet les métriques suivantes via `iroha_telemetry` :| Métrique | Étiquettes | Descriptif |
|---------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Jauge des récupérations orchestres en vol. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Histogramme enregistrant la latence de récupération de bout en bout. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Compteur des échecs terminaux (retries épuisées, aucun fournisseur, échec observateur). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Compteur des tentatives de nouvelle tentative par fournisseur. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Compteur des échecs de fournisseur au niveau session menant à la désactivation. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Compte des décisions de politique d’anonymat (tenue vs brownout) groupées par étape de déploiement et raison de repli. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Histogramme de la partie des relais PQ dans l'ensemble SoraNet sélectionné. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Histogramme des ratios d’offre de relais PQ dans le snapshot du scoreboard. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Histogramme du déficit de politique (écart entre l’objectif et la part PQ réelle). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Histogramme de la partie des relais classiques utilisés dans chaque session. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Histogramme des compteurs de relais classiques sélectionnés par session. |

Intégrez ces métriques dans les tableaux de bord de staging avant d’activer les boutons
en production. La disposition recommandée reflète le plan d’observabilité SF-6 :

1. **Fetches actifs** — alerte si la jauge monte sans complétions correspondantes.
2. **Ratio de retries** — avertit lorsque les compteurs `retry` dépassent les
   lignes de base historiques.
3. **Échecs fournisseurs** — déclenche les alertes pager lorsqu'un fournisseur
   passe `session_failure > 0` dans une fenêtre de 15 minutes.

### 3.2 Cibles de logs structurés

L’orchestrateur publie des événements structurés vers des cibles déterministes :

- `telemetry::sorafs.fetch.lifecycle` — marqueurs `start` et `complete` avec
  nombre de morceaux, de tentatives et durée totale.
- `telemetry::sorafs.fetch.retry` — événements de nouvelle tentative (`provider`, `reason`,
  `attempts`) pour l’analyse manuelle.
- `telemetry::sorafs.fetch.provider_failure` — fournisseurs désactivés pour
  erreurs répétées.
- `telemetry::sorafs.fetch.error` — échecs terminaux résumés avec `reason` et
  métadonnées optionnelles du fournisseur.Acheminez ces flux vers le pipeline de logs Norito existant afin que la réponse
aux incidents étaient une source unique de vérité. Les événements de cycle de vie
exposant le mix PQ/classique via `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` et leurs compteurs associés,
ce qui facilite la connexion des tableaux de bord sans gratter les métriques. Pendentif
les rollouts GA, bloquez le niveau de logs à `info` pour les événements de cycle
de vie/retry et utilisez `warn` pour les erreurs terminales.

### 3.3 Résumé JSON

`sorafs_cli fetch` et le SDK Rust renvoient un résumé structuré contenant :

- `provider_reports` avec les compteurs succès/échec et l'état de désactivation.
- `chunk_receipts` détaillant quel fournisseur a satisfait chaque morceau.
- les tableaux `retry_stats` et `ineligible_providers`.

Archivez le fichier de CV pour déboguer les fournisseurs défaillants : les
les recettes mappent directement aux métadonnées de logs ci-dessus.

## 4. Checklist opérationnelle

1. **Préparer la configuration en CI.** Lancez `sorafs_fetch` avec la
   configuration cible, passez `--scoreboard-out` pour capturer la vue
   d’éligibilité, et faites un diff avec le release précédent. Tout fournisseur
   inéligible inattendu bloque la promotion.
2. **Valider la télémétrie.** Assurez-vous que le déploiement exporte les
   métriques `sorafs.fetch.*` et les logs structurés avant d'activer les fetchs
   multi-source pour les utilisateurs. L’absence de métriques indique souvent
   que la façade de l’orchestrateur n’a pas été appelée.
3. **Documenter les overrides.** Lors d’un `--deny-provider` ou `--boost-provider`
   En cas d'urgence, consignez le JSON (ou l'invocation CLI) dans le changelog. Les
   les rollbacks doivent rappeler l’override et capturer un nouveau snapshot de
   tableau de bord.
4. **Relancer les smoke tests.** Après modification des budgets de retry ou des
   caps de fournisseurs, refetcher la luminaire canonique
   (`fixtures/sorafs_manifest/ci_sample/`) et vérifier que les reçus de chunks
   restent déterministes.

Suivre les étapes ci-dessus garde le comportement de l’orchestrateur
reproductible dans les déploiements en phases et fournit la télémétrie nécessaire à
la réponse aux incidents.

### 4.1 Dérogations de politique

Les opérateurs peuvent barrer la phase de transport/anonymat actif sans
modifier la configuration de base en définissant
`policy_override.transport_policy` et `policy_override.anonymity_policy` dans
leur JSON `orchestrator` (ou en fournissant
`--transport-policy-override=` / `--anonymity-policy-override=` à
`sorafs_cli fetch`). Lorsqu’un override est présent, l’orchestrateur saute le
baisse de tension habituelle : si le niveau PQ demandé ne peut pas être satisfait,
le fetch a échoué avec `no providers` au lieu de dégrader silencieusement. Le
retour au comportement par défaut consiste simplement à vider les champs
d'override.