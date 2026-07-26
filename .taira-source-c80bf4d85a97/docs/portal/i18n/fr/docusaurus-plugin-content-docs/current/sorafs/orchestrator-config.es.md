---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : configuration de l'orchestrateur
titre : Configuration de l'orchestre de SoraFS
sidebar_label : Configuration de l'orchestre
description : Configurez l'orquestador pour récupérer plusieurs origines, interpréter les erreurs et supprimer la sortie de télémétrie.
---

:::note Fuente canonica
Cette page reflète `docs/source/sorafs/developer/orchestrator.md`. Assurez-vous d'avoir des copies synchronisées jusqu'à ce que les documents hérités soient retirés.
:::

# Guia del orquestador de fetch multi-origen

L'explorateur de récupération multi-origine de SoraFS impulsa descargas déterministas y
parallèles depuis le groupe de fournisseurs publiés dans des publicités respaldados por
la gobernanza. Esta guia explica como configurar el orquestador, que senales de
J'attends pendant les déploiements et les flux de télémétrie affichent des indicateurs
de santé.

## 1. Résumé de la configuration

L'explorateur combine trois sources de configuration :

| Source | Proposé | Notes |
|--------|-----------|-------|
| `OrchestratorConfig.scoreboard` | Normalisez les pesos des fournisseurs, validez la fréquence de la télémétrie et conservez le tableau de bord JSON utilisé pour les auditoires. | Répondu par `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Appliquer des limites au temps d'exécution (présupposés de réintention, limites de concurrence, bascules de vérification). | Se mappe a `FetchOptions` et `crates/sorafs_car::multi_fetch`. |
| Paramètres de CLI / SDK | Limiter le nombre de pairs, ajouter des régions de télémétrie et exposer des politiques de refus/boost. | `sorafs_cli fetch` expose directement ces drapeaux ; Le SDK est passé via `OrchestratorConfig`. |

Les helpers JSON et `crates/sorafs_orchestrator::bindings` sérialisent la
configuration complète en Norito JSON, où la rendre portable entre les liaisons de
SDK et automatisation.

### 1.1 Configuration JSON par exemple

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

Conserver l'archive au milieu de l'apilado habituel de `iroha_config` (`defaults/`,
user, actual) para que los despliegues deterministas hereden los mismos limites
entre les nœuds. Pour un profil de secours direct uniquement aligné avec le déploiement
SNNet-5a, consultez `docs/examples/sorafs_direct_mode_policy.json` et le guide
complémentaria en `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Anulations de cumul

SNNet-9 intègre l'ensemble dirigé par le gouverneur de l'orchestre. Un
nouvel objet `compliance` dans la configuration Norito JSON capture les carve-outs
qui permet au pipeline de récupérer de manière directe uniquement :

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` déclare les codes ISO-3166 alpha-2 ici
  instance de l'orchestre. Les codes se normalisent pour les mayusculas durant le
  parséo.
- `jurisdiction_opt_outs` reflète le registre de gouvernement. Cuando alguna
  la juridiction de l'opérateur apparaît sur la liste, l'orchestre applicable
  `transport_policy=direct-only` et émet la raison de repli
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` lista digests de manifest (CIDs cegados, codificados en
  mayusculas hexagonales). Les chargements coïncident avec la planification
  direct uniquement et exposant le repli `compliance_blinded_cid_opt_out` en la
  télémétrie.
- `audit_contacts` enregistre l'URI que la gouvernance espère que les opérateurs
  publiquen en sus playbooks GAR.
- `attestations` capture les paquets de colis fournis par l'entreprise qui répondent au
  politique. Chaque entrée définit un `jurisdiction` facultatif (codigo ISO-3166
  alpha-2), un `document_uri`, le `digest_hex` canonique de 64 caractères, le
  horodatage de l'émission `issued_at_ms` et un `expires_at_ms` facultatif. Estos
  artefacts alimentant la liste de contrôle de la salle de l'orchestre pour que le
  outillage de gouvernance peut mettre en évidence les anulaciones con la documentacion
  firmada.

Proporciona el bloque de conformité mediante el apilado habituel de
configuration pour que les opérateurs reçoivent des annonces déterministes. El
ordonnateur de conformité des applications _despues_ des astuces de mode d'écriture : y compris si
Un SDK demande `upload-pq-only`, les exclusions par juridiction ou manifeste
siguen forzando transporte direct-only et fallan rapido cuando no existen
fournisseurs aptos.

Les catalogues canoniques de désinscription vivent en
`governance/compliance/soranet_opt_outs.json` ; le Consejo de Gobernanza publica
actualizaciones mediante publie les étiquettes. Un exemple complet de
la configuration (y compris les attestations) est disponible en
`docs/examples/sorafs_compliance_policy.json`, et le processus opérationnel est
capturer en el
[playbook de cumplimiento GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Ajustements de CLI et SDK| Drapeau / Champ | Effet |
|--------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Limita cuantos proedores sobreviven al filtre del scoreboard. Utilisez le `None` pour utiliser tous les fournisseurs éligibles. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Limiter les réintentions par morceau. Superar el limite genres `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Injectez des instantanés de latence/chute dans le constructeur du tableau de bord. La télémétrie est obsolète mais à la marque `telemetry_grace_secs`, elle est prouvée comme inéligible. |
| `--scoreboard-out` | Persistez le tableau de bord calculé (proveedores elegibles + ineligibles) pour l'inspection postérieure. |
| `--scoreboard-now` | Remplacez l'horodatage du tableau de bord (secondaire Unix) pour que les captures de luminaires soient si déterministes. |
| `--deny-provider` / crochet de politique de score | Exclure de manière déterminante les fournisseurs de la planification sans emprunter de publicités. Utilisé pour les listes noires de réponse rapide. |
| `--boost-provider=name:delta` | Ajustez les crédits du tournoi à la ronde pour qu'un fournisseur conserve intacts les pesos de l'État. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Étiquettes métriques émises et journaux structurés pour que les tableaux de bord puissent filtrer la géographie lors du déploiement. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | Par défaut, c'est `soranet-first` maintenant que l'orchestre multi-origine est la base. Utilisez `direct-only` pour préparer un déclassement ou suivre une directive d'achat, et réservez `soranet-strict` pour les pilotes PQ uniquement ; les anulaciones de conformité siguen siendo el techo duro. |

SoraNet-first est désormais la valeur par défaut, et les restaurations doivent être citar el
bloqueur SNNet correspondant. Tras graduarse SNNet-4/5/5a/5b/6a/7/8/12/13,
la gobernanza endurecera la postura requerida (hacia `soranet-strict`); hasta
donc, seules les annonces motivées par des incidents doivent être prioritaires
`direct-only`, et vous devez vous inscrire dans le journal de déploiement.

Tous les drapeaux antérieurs acceptent le style de syntaxe `--` tanto fr
`sorafs_cli fetch` comme le binaire `sorafs_fetch` orienté vers
desarrolladores. Le SDK expose les différentes options proposées par les constructeurs.

### 1.4 Gestion de cache de gardes

La CLI intègre désormais le sélecteur de gardes de SoraNet pour les opérateurs
peut fijar relais d'entrée de forme déterministe avant le déploiement complet
SNNet-5 de transport. Trois nouveaux drapeaux contrôlent le flux :| Drapeau | Proposé |
|------|-----------|
| `--guard-directory <PATH>` | Ajoutez un fichier JSON qui décrit le consentement des relais le plus récent (se montre un sous-ensemble plus bas). Passer le répertoire rafraîchir la cache des gardes avant d'exécuter la récupération. |
| `--guard-cache <PATH>` | Conservez le code `GuardSet` en Norito. Les exécutions postérieures réutilisent également le cache lorsqu'elles ne désignent pas un nouveau répertoire. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Remplace les options pour le nombre de gardes d'entrée au fijar (par défaut 3) et la fenêtre de rétention (par défaut 30 jours). |
| `--guard-cache-key <HEX>` | Clé facultative de 32 octets utilisée pour étiquetter les caches de garde avec un MAC Blake3 afin que l'archive puisse être vérifiée avant de être réutilisée. |

Les charges du directeur des gardes utilisent une esquema compacto :

Le drapeau `--guard-directory` attend maintenant une charge utile `GuardDirectorySnapshotV2`
codifié en Norito. Le binaire instantané contient :

- `version` — version du schéma (actuellement `2`).
-`directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  les métadonnées de consensus qui doivent coïncider avec chaque certificat intégré.
- `validation_phase` — ordinateur politique de certificados (`1` = permis
  una sola firma Ed25519, `2` = préférer les entreprises doubles, `3` = exiger les entreprises
  double).
- `issuers` — émetteurs de gouvernement avec `fingerprint`, `ed25519_public` et
  `mldsa65_public`. Les empreintes digitales sont calculées comme
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — une liste de bundles SRCv2 (sortie de
  `RelayCertificateBundleV2::to_cbor()`). Chaque bundle inclut le descripteur du
  relais, drapeaux de capacité, politique ML-KEM et firmas doubles Ed25519/ML-DSA-65.

La CLI vérifie chaque paquet contre les clés d'émetteur déclarées avant de
fusionner le répertoire avec la cache de gardes. Les fichiers JSON sont hérités
je n'accepte pas; si vous avez besoin d'instantanés SRCv2.

Appelez la CLI avec `--guard-directory` pour fusionner le consensus le plus récent avec
la cache existante. Le sélecteur conserve les gardes fixées qui sont à l'intérieur
la ventana de retencion y son elegibles en el Directorio ; les relais nouveaux
réemplazan entrées expiradas. Après une récupération, le cache s'actualise
écrivez de nouveau sur la route indiquée par `--guard-cache`, en maintenant les sessions
subsecuentes déterministes. Le SDK peut reproduire le même comportement
appel `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
et passer le `GuardSet` résultant en `SorafsGatewayFetchOptions`.`ml_kem_public_hex` permet que le sélecteur garde la priorité avec la capacité PQ
lorsque vous désactivez SNNet-5. Les bascules de l'étape (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) maintenant, les relais sont automatiquement dégradés
Clasicos : quand il y a un gardien PQ disponible dans le sélecteur Elimina Los Pines
clasicos sobrantes para que las sessionses posteriores favorisent les poignées de main
hybrides. Les résumés de CLI/SDK exposent la méthode résultante via
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` et les champs complémentaires de candidats/déficit/
variation de l'approvisionnement, rendant explicites les baisses de tension et les classiques de repli.

Les directeurs de gardes peuvent désormais intégrer un bundle SRCv2 complet via
`certificate_base64`. El orquestador décodifica cada bundle, revalida las firmas
Ed25519/ML-DSA et conserve le certificat analysé conjointement avec la cache des gardes.
Cuando hay un certificado presente se converti en la fuente canonica para
clés PQ, préférences de poignée de main et pondération ; les certificats expirés
se descartan et le sélecteur vuelve a campos heredados del descriptor. Los
certificados se propagan a la gestion del cycle de vida de circuitos y se
exponen via `telemetry::sorafs.guard` et `telemetry::sorafs.circuit`, qui s'enregistre
la ventana de validez, les suites de handshake y si se observaron firmas dobles
garde para cada.

Utilisez les assistants de la CLI pour maintenir les instantanés synchronisés avec les
publicadores:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` téléchargez et vérifiez l'instantané SRCv2 avant de l'écrire en discothèque,
pendant que `verify` répète le pipeline de validation pour les artefacts obtenus
par d'autres équipements, émet un CV JSON qui reflète la sortie du sélecteur
de gardes en CLI/SDK.

### 1.5 Gestionnaire de cycle de vie de circuits

Lorsque le directeur des relais se charge de la cache des gardes, le
instructeur activa le gestionnaire de cycle de vie de circuits pour la préconstruction et
rénover les circuits de SoraNet avant chaque récupération. La configuration vive en
`OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) via dos campos
nouveaux:

- `relay_directory` : prend l'instantané du répertoire SNNet-3 pour les sauts
  milieu/sortie peut être sélectionné de manière déterminée.
- `circuit_manager` : configuration facultative (habilitée par défaut) que
  contrôlez le TTL du circuit.

Norito JSON accepte maintenant un blocage `circuit_manager` :

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Le SDK réinitialise les données du répertoire via
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), et la CLI se connecte automatiquement toujours
que se sumministre `--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`).Le gestionnaire renouvelle les circuits en changeant les métadonnées de la garde (point final,
(clave PQ ou timestamp fijado) ou lorsque le TTL est activé. L'assistant `refresh_circuits`
appelé avant chaque récupération (`crates/sorafs_orchestrator/src/lib.rs:1346`)
émettre des journaux de `CircuitEvent` pour que les opérateurs rastreen prennent les décisions du
cycle de vie. El test de trempage
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) la latence est établie
voyages de trois rotations de gardes; consulter le rapport fr
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Proxy QUIC local

L'orquesteur peut activer optionnellement un proxy local QUIC pour que les
extensions du navigateur et des adaptateurs SDK ne peuvent pas être gérés
certificados ou claves de cache de guards. Le proxy se dirige vers une direction
loopback, terminez les connexions QUIC et développez un manifeste Norito qui décrit le
certificado y la clave de cache de guard facultative al cliente. Les événements de
transporte émis par le proxy se contabilizan via
`sorafs_orchestrator_transport_events_total`.

Habiliter le proxy à traverser le nouveau bloc `local_proxy` dans le JSON du
orchestre:

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
```- `bind_addr` controla donde escucha el proxy (usa puerto `0` para solicitar
  un puerto efimero).
- `telemetry_label` se propage aux mesures pour que les tableaux de bord puissent être utilisés
  distinguer les proxys des sessions de récupération.
- `guard_cache_key_hex` (facultatif) permet au proxy d'étendre le même cache
  des gardes avec la clé qui utilise CLI/SDK, en gardant les extensions alignées
  du navigateur.
- `emit_browser_manifest` alterna si la poignée de main développe un manifeste que las
  les extensions peuvent être stockées et validées.
- `proxy_mode` sélectionnant le proxy permettant un trafic local (`bridge`) ou
  émettre uniquement des métadonnées pour que le SDK ouvre les circuits SoraNet par votre compte
  (`metadata-only`). Le proxy par défaut est `bridge` ; établissement `metadata-only`
  lorsqu'un poste de travail doit exposer le manifeste sans réafficher les flux.
- `prewarm_circuits`, `max_streams_per_circuit` et `circuit_ttl_hint_secs`
  des conseils exponentiels supplémentaires sur le navigateur pour que vous puissiez présupposer des flux
  Paralelos et entendre que tan agresivamente el proxy reutiliza circuitsos.
- `car_bridge` (facultatif) ajoute un cache local aux archives CAR. Le camp
  `extension` contrôle le soufijo agrégé lorsque l'objet de flux est omite
  `*.car` ; establece `allow_zst = true` pour servir les charges utiles `*.car.zst`
  précomprimés directement.
- `kaigi_bridge` (facultatif) expose les routes Kaigi et le spool al proxy. Le camp
  `room_policy` annonce si le pont fonctionne en mode `public` ou `authenticated`
  pour que les clients du navigateur présélectionnent les étiquettes GAR
  correctas.
- `sorafs_cli fetch` expone remplace `--local-proxy-mode=bridge|metadata-only`
  et `--local-proxy-norito-spool=PATH`, permettant d'alterner le mode de
  Exécuter ou ajouter des bobines alternatives sans modifier la politique JSON.
- `downgrade_remediation` (facultatif) configure le crochet de rétrogradation automatique.
  Lorsque l'orquesteur est habilité à observer la télémétrie des relais pour
  détecter les rafagas de downgrade et, après le `threshold` configuré à l'intérieur de
  `window_secs`, prend le proxy local vers `target_mode` (par défaut
  `metadata-only`). Une fois que vous avez effectué les rétrogradations, le proxy vuelve a
  `resume_mode` après `cooldown_secs`. Utiliser l'arreglo `modes` pour limiter
  el déclenche des rôles de relais spécifiques (par défaut des relais d'entrée).

Lorsque le proxy est exécuté en mode bridge sirve deux services d'application :

- **`norito`** – l'objet du flux du client est résolu par rapport à
  `norito_bridge.spool_dir`. Los objetivos se sanitzan (sin traversal ni rutas)
  absolutas) et, lorsque l'archive n'a pas d'extension, elle s'applique au contenu
  configuré avant de transmettre la charge utile littéralement au navigateur.
- **`car`** – les objets du flux sont résolus à l'intérieur de
  `car_bridge.cache_dir`, voici l'extension par défaut configurée et
  les charges utiles rechazan sont réduites à moins que `allow_zst` soit activée. Los
  les ponts exitosos répondent avec `STREAM_ACK_OK` avant de transférer les octets
  del archivo para que los clients puedan canalizar la verificación.Dans d'autres cas, le proxy entre le HMAC de cache-tag (quand il existe une clé
cache de garde pendant la poignée de main) et enregistrement des codes de raison de télémétrie
`norito_*` / `car_*` pour que les tableaux de bord diffèrent des sorties, des archives
faltantes et fallos de sanitizacion d’un vistazo.

`Orchestrator::local_proxy().await` expose la poignée en éjection pour que les
les appels peuvent lire le PEM du certificat, obtenir le manifeste du
Le navigateur ou solliciter un apagado ordonné lorsque l'application est finalisée.

Une fois habilité, le proxy est maintenant enregistré **manifest v2**. Ademas du
certifié existant et la clé de cache de garde, v2 agréga :

- `alpn` (`"sorafs-proxy/1"`) et un règlement `capabilities` pour les clients
  confirmez le protocole de flux que vous devez utiliser.
- Un `session_id` pour la poignée de main et un blocage du sel `cache_tagging` pour dériver
  afinités de garder la session et les balises HMAC.
- Conseils de circuit et de sélection de garde (`circuit`, `guard_selection`,
  `route_hints`) pour que les intégrations du navigateur étendent une plus grande interface utilisateur
  rica avant d'ouvrir les flux.
- `telemetry_v2` avec boutons de musique et de confidentialité pour instrumentation locale.
- Cada `STREAM_ACK_OK` comprend `cache_tag_hex`. Les clients reflètent la valeur de
  l'en-tête `x-sorafs-cache-tag` à l'émetteur demande HTTP ou TCP pour cela
  sélections de garde en cache permanentes cifradas en reposo.

Ces champs préservent le format précédent ; les clients anciens peuvent ignorer
les nouvelles clés et continuer à utiliser le sous-ensemble v1.

## 2. Sémantique des erreurs

L'orquestador applique des restrictions de capacité et des présupposés avant
qui se transfiera un seul octet. Los fallos caen en trois catégories:

1. **Fallos de elegibilidad (pré-vol).** Proveedores sin capacidad de rango,
   adverts expirados o telemetria obsoleta quedan registrados en el artefacto
   del scoreboard y se omiten en la planificacion. Les CV de CLI llenan
   l'arreglo `ineligible_providers` avec des raisons pour que les opérateurs puissent le faire
   inspeccionar el drift de gobernanza sin raspar logs.
2. **Agotamiento en tiempo de ejecucion.** Chaque fournisseur enregistre les erreurs
   consécutifs. Une fois que vous parlez du `provider_failure_threshold`
   configuré, le fournisseur est marqué comme `disabled` pour le reste de la session.
   Si tous les fournisseurs pasan a `disabled`, l'orquestador devuelve
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Abortos deterministas.** Les limites strictes se présentent comme des erreurs
   structurés :
   - `MultiSourceError::NoCompatibleProviders` — le manifeste requiert un tramo
     de chunks o alineacion que los provenedores restantes no pueden cumplir.
   - `MultiSourceError::ExhaustedRetries` — pour consommer le présupposé de
     réintentions par morceau.
   - `MultiSourceError::ObserverFailed` — los observadores en aval (crochets de
     streaming) rechazaron un chunk vérifié.Chaque erreur inclut l'indice du morceau problématique et, lorsqu'il est disponible,
la raison finale de la chute du fournisseur. Trata estos fallos como bloqueadores de
release : los reintentos con la misma entrada reproduciran el fallo hasta que
modifier l'annonce, la télémétrie ou la santé du fournisseur subyacente.

### 2.1 Persistance du tableau de bord

Lorsque vous configurez `persist_path`, l'orquestador écrit le tableau de bord final
tras cada ejecución. Le document JSON contient :

- `eligibility` (`eligible` ou `ineligible::<reason>`).
- `weight` (peso normalizado asignado para esta ejecucion).
- métadonnées du `provider` (identifiant, points de terminaison, présupposé de
  concurrence).

Archivage des instantanés du tableau de bord avec les artefacts de sortie pour que
les décisions de liste noire et de déploiement signifient que les auditables sont effectués.

## 3. Télémétrie et dépuration

### 3.1 Métriques Prometheus

L'orchestre émet les mesures suivantes via `iroha_telemetry` :

| Métrique | Étiquettes | Description |
|---------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Jauge de récupération des orquestados en vuelo. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Histogramme qui enregistre la latence de récupération de l'extrême à l'extrême. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Contador de fallos terminales (reintentos agotados, sin proedores, fallo del observateur). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Contador de intentos de reintento por provenedor. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Contador de fallos a nivel de session qui llevan a deshabilitar provenedores. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Conteo de décisions politiques anonymisées (cumplida vs brownout) agrégées pour l'étape de déploiement et la raison de repli. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Histogramme de la cuota de relays PQ à l’intérieur de l’ensemble SoraNet sélectionné. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Histogramme des ratios d'offre de relais PQ sur l'instantané du tableau de bord. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Histogramme du déficit politique (brèche entre l'objet et la valeur PQ réelle). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Histogramme de la cuota de relays clasicos usada en chaque session. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Histogramme des conteos de relais clasicos sélectionnés par session. |

Intégrer les mesures dans les tableaux de bord de mise en scène avant d'activer les boutons de
production. La disposition recommandée reflète le plan d'observabilité SF-6 :

1. **Récupère les actifs** — alerte si la jauge est sous-jacente sans équivalents de complétions.
2. **Ratio de reintentos** — avisa cuando los contadores `retry` superan
   lignes de base historiques.
3. **Chutes du fournisseur** — afficher les alertes sur le pager lorsque c'est un fournisseur
   supera `session_failure > 0` pendant 15 minutes.### 3.2 Cibles des logs structurés

L'orchestre public d'événements structurés et cibles déterministes :

- `telemetry::sorafs.fetch.lifecycle` — marques de cycle de vie `start` et
  `complete` avec contenu de morceaux, réintentions et durée totale.
- `telemetry::sorafs.fetch.retry` — événements de réintégration (`provider`, `reason`,
  `attempts`) pour alimenter le manuel de triage.
- `telemetry::sorafs.fetch.provider_failure` — fournisseurs déshabilités par
  erreurs répétées.
- `telemetry::sorafs.fetch.error` — fallos terminales resumidos con `reason` y
  métadonnées facultatives du fournisseur.

Envoyez ces flux vers le pipeline de journaux Norito existant pour la réponse à
les incidents ont une seule source de vérité. Les événements du cycle de vie
expose la mezcla PQ/classica via `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` et ses contadores associés,
afin de faciliter le câblage des tableaux de bord sans raspar metricas. Durant les déploiements de GA,
fixer le niveau des journaux en `info` pour les événements de cycle de vie/reintento et usa
`warn` pour les terminaux de chute.

### 3.3 Reprise du JSON

Tanto `sorafs_cli fetch` comme le SDK de Rust a développé un CV structuré
que contient:

- `provider_reports` avec conteos de exito/fracaso et si un fournisseur de source
  déshabilité.
- `chunk_receipts` détaille la résolution de chaque morceau.
- arreglos `retry_stats` et `ineligible_providers`.

Archivage du fichier de reprise du paiement des fournisseurs problématiques : les reçus
mappé directement sur les métadonnées des journaux antérieurs.

## 4. Liste de contrôle opérationnelle

1. **Préparer la configuration en CI.** Éjecté `sorafs_fetch` avec la
   configuration de l'objet, passez `--scoreboard-out` pour capturer la vue de
   éligibilité et comparaison avec la version antérieure. Cualquier fournisseur inéligible
   inesperado detiene la promocion.
2. **Télémétrie valide.** Assurez-vous que le téléchargement exporte des mesures
   `sorafs.fetch.*` et les journaux structurés avant d'activer la récupération multi-origine
   pour les utilisateurs. L'utilité des mesures peut indiquer que la façade du
   orquestador no fue invocado.
3. **Documenter les remplacements.** À l'aide des paramètres d'urgence `--deny-provider`
   o `--boost-provider`, confirmez le JSON (ou l'appel CLI) dans votre journal des modifications.
   Les rollbacks doivent rétablir le remplacement et capturer un nouvel instantané du
   tableau de bord.
4. **Répétez les tests de fumée.** En modifiant les présupposés de réintention ou les limites
   des fournisseurs, vuelve a hacer fetch del luminaire canonico
   (`fixtures/sorafs_manifest/ci_sample/`) et vérifier que les reçus de
   des morceaux sigan siendo déterministes.

Suivre les étapes antérieures pour maintenir le comportement de l'orchestre
reproductible dans les déploiements par étapes et en fournissant la télémétrie nécessaire pour
la réponse aux incidents.

### 4.1 Annonces politiquesLes opérateurs peuvent fijar l'étape active de transport/anonimato sans éditer
la base de configuration estableciendo `policy_override.transport_policy` et
`policy_override.anonymity_policy` et votre JSON de `orchestrator` (ou Suministrando
`--transport-policy-override=` / `--anonymity-policy-override=` un
`sorafs_cli fetch`). Lorsque quelqu'un des remplacements est présent
l'orquestador omite la baisse de tension de secours habituelle : si le niveau PQ est sollicité non se
peut être satisfaisant, la récupération échoue avec `no providers` au lieu de dégradation
silence. Retourner au comportement par défaut est très simple pour le nettoyer
champs de remplacement.