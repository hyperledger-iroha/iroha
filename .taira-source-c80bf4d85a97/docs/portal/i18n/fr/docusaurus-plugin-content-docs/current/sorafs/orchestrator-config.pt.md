---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : configuration de l'orchestrateur
titre : Configuration de l'orchestre SoraFS
sidebar_label : Configuration de l'orchestre
description : Configurez l'orquestrador de fetch multi-origem, interprètez les erreurs et dépurez la dite de télémétrie.
---

:::note Fonte canonica
Cette page espelha `docs/source/sorafs/developer/orchestrator.md`. Mantenha ambas as copias sincronizadas ate que a documentacao alternativa seja retirada.
:::

# Guia do orquestrador de fetch multi-origem

L'explorateur de récupération multi-origes du SoraFS effectue des téléchargements déterministes et
parallèles à partir du groupe de fournisseurs publiés dans des publicités respaldados
par la gouvernance. Este guia explica como configurar o orquestrador, quais sinais
il faut attendre pendant les déploiements et les flux d'expériences de télémétrie
indicateurs de santé.

## 1. Visa général de configuration

L'explorateur combine trois sources de configuration :

| Fonte | Proposé | Notes |
|-------|-----------|-------|
| `OrchestratorConfig.scoreboard` | Normaliser les pesos des fournisseurs, valider la fresque de la télémétrie et conserver le tableau de bord JSON utilisé pour les auditoires. | Apoiado por `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Limites d'exécution de l'application (budgets de nouvelle tentative, limites de cohérence, bascules de vérification). | Carte pour `FetchOptions` et `crates/sorafs_car::multi_fetch`. |
| Paramètres de CLI / SDK | Limite du nombre de pairs, zones annexes de télémétrie et politique de refus/boost. | `sorafs_cli fetch` expose ces drapeaux directement ; OS SDK OS propagande via `OrchestratorConfig`. |

Os helpers JSON dans `crates/sorafs_orchestrator::bindings` sérialiser un
configuration complète dans Norito JSON, tornando-un portavel entre les liaisons des SDK
c'est automatique.

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

Persista o arquivo atraves do empilhamento habituel do `iroha_config` (`defaults/`,
utilisateur, actuel) pour que les déploiements soient déterminés par nos limites entre
nodos. Pour un profil de secours direct uniquement associé au déploiement SNNet-5a,
consulter `docs/examples/sorafs_direct_mode_policy.json` et orientation
correspondant au `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Remplacements de conformité

Un SNNet-9 intégré conforme est orienté vers le gouvernement de l'explorateur. Euh
nouvel objet `compliance` dans la configuration Norito JSON capture les exclusions que
forcam ou pipeline de récupération en mode direct uniquement :

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
  Instancia do Orquestrador Opera. Os codigos sao normalizados para maiusculas
  pendant l'analyse.
- `jurisdiction_opt_outs` inscrire le registre de gouvernance. Quando qualquer
  la juridiction de l'opérateur apparaît sur la liste, ou l'orquestrador aplica
  `transport_policy=direct-only` et émet le motif de repli
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` lista digests de manifest (CIDs cegados, codificados em
  hex maiusculo). Payloads correspondants également pour l'agenda de la caméra en direct uniquement
  et expõem le repli `compliance_blinded_cid_opt_out` par télémétrie.
- `audit_contacts` enregistré en tant qu'URI qui gouvernent les opérateurs
  publiquem nos playbooks GAR.
- `attestations` capture les paquets de conformité assassinés qui soutiennent le
  politique. Chaque entrée définit un `jurisdiction` facultatif (codigo ISO-3166
  alpha-2), um `document_uri`, o `digest_hex` canonico de 64 caractères, o
  horodatage de l'émission `issued_at_ms` et `expires_at_ms` facultatif. Essès
  artefatos alimentam o checklist de auditoria do orchestre pour que comme
  ferramentas de gouvernance possam vincular remplace une documentacao assinada.

Forneca o bloco de conformidade via o empilhamento habituelle de configuracao para
que les opérateurs reçoivent l'emporte sur les déterministes. O orquestrador aplica a
conformité _depois_ aux indications du mode d'écriture : même si un SDK est sollicité
`upload-pq-only`, opt-outs de jurisdicao ou manifest ainda forcam o transporte
pour direct-only et Falham rapidement lorsqu'il n'existe aucun fournisseur conforme.

Catalogues canoniques de désinscription vivez-les
`governance/compliance/soranet_opt_outs.json` ; o Conselho de Governanca publica
atualizacoes via des versions tagueadas. Un exemple complet de configuration
(y compris les attestations) esta disponivel em
`docs/examples/sorafs_compliance_policy.json`, et le processus opérationnel est
capturé non
[playbook de conformité GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Ajustements de CLI et SDK| Drapeau / Champ | Efeito |
|--------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Limitez les quantités fournies par le filtre du tableau de bord. Définissez `None` pour utiliser tous les fournisseurs élégants. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Limita réessaye par morceau. Dépasser la limite gera `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Injeta snapshots de latencia/falha no constructeur do scoreboard. Télémétrie obsolète mais de `telemetry_grace_secs` marque prouvée comme inélégive. |
| `--scoreboard-out` | Persistez sur le tableau de bord calculé (provedores elegiveis + inelegiveis) pour inspecter la position de course. |
| `--scoreboard-now` | Enregistrez l'horodatage du tableau de bord (secondaire Unix) pour gérer les captures de matchs déterministes. |
| `--deny-provider` / crochet de politique de score | Exclui provenores de forma deterministica sem deleter adverts. Utilisé pour mettre rapidement sur liste noire. |
| `--boost-provider=name:delta` | Ajustez les crédits à la ronde en tenant compte d'un fournisseur en gardant les pesos de gouvernance intacts. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Rotules métriques émises et journaux établis pour que les tableaux de bord puissent filtrer la géographie ou l'onde de déploiement. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | Le padrao agora et `soranet-first` est également que l'orquestrador multi-origem et une base. Utilisez `direct-only` pour préparer la rétrogradation ou suivre une directive de conformité, et réserver `soranet-strict` pour les pilotes PQ uniquement ; remplace la conformidade continuam sendo o teto rigido. |

SoraNet-first et maintenant le pad d'envoi, et les rollbacks doivent citar ou bloqueador
SNNet pertinent. Après que SNNet-4/5/5a/5b/6a/7/8/12/13 soit diplômé, un
gouvernance endurecera a postura requerida (rumo a `soranet-strict`); j'ai mangé là,
apenas remplace les motivations pour les incidents devem priorizar `direct-only`, et les éléments
les développeurs sont enregistrés dans le journal de déploiement.

Tous les drapeaux acima aceitam a sintaxe `--` tanto em `sorafs_cli fetch` quanto no
binario `sorafs_fetch` activé par les utilisateurs. Les SDK sont utilisés comme plusieurs opérations
par moi de constructeurs tipados.

### 1.4 Gestion du cache des gardes

La CLI intègre également le sélecteur de protections de SoraNet pour les opérateurs
réparer les relais d'entrée de forme déterministe avant le déploiement complet de
transporte SNNet-5. Trois nouveaux drapeaux contrôlent le flux :

| Drapeau | Proposé |
|------|-----------|
| `--guard-directory <PATH>` | Apont pour un fichier JSON qui décrit le consensus des relais le plus récent (sous-ensemble abaixo). Passer le répertoire à jour ou le cache des gardes avant d'exécuter ou de récupérer. |
| `--guard-cache <PATH>` | Conserver le code `GuardSet` dans Norito. Exécuter ultérieurement la réutilisation du cache dans un nouveau répertoire. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Remplace les options pour le nombre de gardes d'entrée à fixer (pas 3) et une fenêtre de retenue (pas 30 jours). |
| `--guard-cache-key <HEX>` | Vous devez éventuellement utiliser 32 octets pour marquer les caches de garde avec un MAC Blake3 afin que l'archive soit vérifiée avant sa réutilisation. |

En tant que chargés du répertoire des gardes, utilisez une esquema compacto :Le drapeau `--guard-directory` va espérer une charge utile `GuardDirectorySnapshotV2`
codifié sous Norito. Le contenu binaire instantané :

- `version` — versao do esquema (actuellement `2`).
-`directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  métadados de consensus que devem correspondant a cada certificado embutido.
- `validation_phase` — gate de politica de certificados (`1` = permitir uma
  assinatura Ed25519, `2` = préférer assinaturas duplas, `3` = exigir assinaturas
  duplas).
- `issuers` — émetteurs de gouvernance avec `fingerprint`, `ed25519_public` et
  `mldsa65_public`. Os empreintes digitales sao calculées como
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — une liste de bundles SRCv2 (dit de
  `RelayCertificateBundleV2::to_cbor()`). Chaque paquet carrega ou descripteur do
  relais, drapeaux de capacité, politique ML-KEM et assassinats duplas Ed25519/ML-DSA-65.

Une CLI vérifie chaque paquet contre les chaves de l'émetteur déclarées avant de
mesclar o directory com o cache de guards. Esbocos JSON alternatives pour plus d'informations
Aceitos; instantanés SRCv2 sao obrigatorios.

Appelez la CLI avec `--guard-directory` pour clarifier ou accepter le plus récemment avec l'
cache existant. Le sélecteur de protection garde les éléments fixés qui sont également à l'intérieur de la
Janela de retencao et sao elegiveis n'ont pas de répertoire ; novos relais remplacent les entrées
expirées. Après avoir récupéré le résultat, le cache actualisé et écrit en temps réel
pas de chemin fornecido via `--guard-cache`, mantendo sessoes nextes
critères. Les SDK peuvent reproduire votre même comportement
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` f
passer ou `GuardSet` résultant pour `SorafsGatewayFetchOptions`.

`ml_kem_public_hex` permet au sélecteur de prioriser les gardes avec la capacité PQ
Lorsque le déploiement de SNNet-5 est en cours. Os bascule de l'étape (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) agora rebaixam relais classiques
automatiquement : lorsque le garde PQ est disponible ou le sélecteur retire les broches
classicos excedentes para que sessoes nextes favorecam handshakes hibridos.
Les résumés CLI/SDK affichent la valeur résultante via `anonymity_status`/
`anonymity_reason`, `anonymity_effective_policy`, `anonymity_pq_selected`,
`anonymity_classical_selected`, `anonymity_pq_ratio`, `anonymity_classical_ratio`
Les champs complémentaires des candidats/déficit/delta d'approvisionnement, tornade claire
baisses de tension et replis classiques.

Les répertoires de guards peuvent maintenant intégrer un bundle SRCv2 complet via
`certificate_base64`. O orquestrador décodifica cada bundle, revalida as
Les assassinats Ed25519/ML-DSA et le certificat analysé avec le cache de
gardes. Quand un certificat est présenté, il se tourne vers la fonte canonique pour
chaves PQ, préférences de poignée de main et pondération; certificats expirés sao
descartados e o seletor retorna aos campos alternativos do descriptor. Certifiés
propagande-se pela gestao do cycle de vida de circuitsos e sao expostos via
`telemetry::sorafs.guard` et `telemetry::sorafs.circuit`, qui s'inscrivent à Janela
de validation, suites de poignée de main et assassinats duplas foram observés para
garde cada.Utilisez les assistants de la CLI pour gérer les instantanés en même temps que les utilisateurs :

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` Baixa et verifica o snapshot SRCv2 avant de graver dans la discothèque, en même temps
`verify` reproduz le pipeline de validation pour les artefatos vindos de outras equipes,
émet un résumé JSON qui s'affiche dans le sélecteur de gardes CLI/SDK.

### 1.5 Gestionnaire de cycle de vie de circuits

Quando um relay directory e um cache de guards sao fornecidos, o orquestrador
actif ou gestionnaire de cycle de vie de circuits pour la préconstruction et la rénovation
circuits SoraNet avant chaque récupération. Une configuration vive dans `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) via deux nouveaux champs :

- `relay_directory` : charger l'instantané dans le répertoire SNNet-3 pour les sauts
  milieu/sortie sejam selecionados de forma deterministica.
- `circuit_manager` : configuration facultative (habilitée par le contrôleur) qui contrôle le
  TTL fait un circuit.

Norito JSON vient d'être ajouté au bloc `circuit_manager` :

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Les SDK encaminham dados font le répertoire via
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), et une CLI ou une connexion automatique semper que
`--guard-directory` et fourni (`crates/iroha_cli/src/commands/sorafs.rs:365`).

O gestor renova circuitos semper que metadados do guard mudam (endpoint, chave
PQ ou timestamp corrigé) ou lorsque le TTL expire. O assistant `refresh_circuits`
appelé avant chaque récupération (`crates/sorafs_orchestrator/src/lib.rs:1346`)
émettre des journaux `CircuitEvent` pour que les opérateurs puissent prendre des décisions de cycle
de vie. O test de trempage `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) démonstration de latence estavel
atraves de tres rotacoes de gardes; voir ou relation avec eux
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Proxy QUIC local

L'explorateur peut éventuellement lancer un proxy QUIC local pour des extensions
du navigateur et des adaptateurs SDK ne génère pas précisément des certificats ou des chaves de
cache de gardes. O proxy liga à un bouclage endereco, encerra conexoes QUIC e
restituer le manifeste Norito décrit ou certifié et le cache de garde
facultatif pour le client. Événements de transport émis par proxy sao contados via
`sorafs_orchestrator_transport_events_total`.

Habiliter le proxy pour mon nouveau bloc `local_proxy` sans JSON pour l'explorateur :

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
```- `bind_addr` contrôle l'onde ou le proxy (utilisez le port `0` pour solliciter le port
  éphémères).
- `telemetry_label` propagande pour les mesures permettant de distinguer les tableaux de bord
  proxys de sessions de récupération.
- `guard_cache_key_hex` (facultatif) permet au proxy d'exposer le même cache de
  les gardes doivent utiliser les CLI/SDK pour étendre les extensions du navigateur.
- `emit_browser_manifest` alternative à la poignée de main dévolue à un manifeste
  des extensions peuvent être armées et validées.
- `proxy_mode` sélectionne le proxy Faz Bridge local (`bridge`) ou émet
  métadonnées pour les SDK d'Abram Circuits SoraNet par le propriétaire
  (`metadata-only`). Le proxy padrao e `bridge` ; utiliser `metadata-only` quand je suis
  le poste de travail développe l'exportation du manifeste pour retransmettre les flux.
- `prewarm_circuits`, `max_streams_per_circuit` et `circuit_ttl_hint_secs`
  expõem conseils supplémentaires sur le navigateur pour pouvoir écouter des flux parallèles et
  entendre le quanto ou le proxy réutiliser les circuits.
- `car_bridge` (facultatif) disponible pour un cache local des archives CAR. Le camp
  `extension` contrôle le suffixe ajouté lorsque le flux est omis `*.car` ;
  définir `allow_zst = true` pour servir les charges utiles `*.car.zst` précomprimées.
- `kaigi_bridge` (facultatif) utilise des rotations Kaigi dans la bobine vers le proxy. Le camp
  `room_policy` annonce le fonctionnement du pont en mode `public` ou `authenticated`
  pour que les clients du navigateur présélectionnent les étiquettes GAR corretos.
- `sorafs_cli fetch` expõe remplace `--local-proxy-mode=bridge|metadata-only` e
  `--local-proxy-norito-spool=PATH`, permettant un mode d'exécution alternatif ou
  proposer des bobines alternatives pour modifier la politique JSON.
- `downgrade_remediation` (facultatif) configure le crochet de rétrogradation automatique.
  Quando habilitado, o orquestrador observe a telemetria de relays para rajadas
  de downgrade e, apos o `threshold` configuré à l'intérieur de `window_secs`, força o
  proxy local pour `target_mode` (padrao `metadata-only`). Quand le système d'exploitation rétrograde-t-il
  cessam, le proxy répond à `resume_mode` après `cooldown_secs`. Utiliser un tableau
  `modes` pour limiter le gatilho aux fonctions de relais spécifiques (relais padrao
  de l'entrée).

Lorsque le proxy est utilisé en mode bridge, il sert deux services d'application :

- **`norito`** – le flux du client et la résolution relative à
  `norito_bridge.spool_dir`. Os alvos sao sanitizados (sem traversal, sem caminhos
  absolus), et lorsque l'archive n'est pas étendue, le suffixe configuré et appliqué
  avant que la charge utile soit transmise au navigateur.
- **`car`** – les flux de flux sont résolus dans le `car_bridge.cache_dir`, herdam
  un module d'extension configuré et des charges utiles réduites compressées à moindre coût
  `allow_zst` est habilité. Les ponts ont répondu avec succès avec `STREAM_ACK_OK`
  avant de transférer les octets de l'archive pour que les clients puissent utiliser le pipeline
  par vérification.Dans d'autres cas, le proxy fornece ou le HMAC fait cache-tag (quand il y a un chave de
cache de garde pendant la poignée de main) et enregistrement des codes de code de télémétrie
`norito_*` / `car_*` pour que les tableaux de bord diffèrent des succès, des archives ausentes
e falhas de sanitizacao rapidamente.

`Orchestrator::local_proxy().await` Expõe o handle em execucao para que chamadas
possam ler o PEM do certificado, buscar o manifest do navegador ou solicitar
encerramento gracioso quando aplicaçao finaliza.

Quando habilitado, o proxy agora serve registros **manifest v2**. Alem fais
certificat existant et du cache de garde, ajouté à la v2 :

- `alpn` (`"sorafs-proxy/1"`) et un tableau `capabilities` pour les clients
  Confirmez le protocole de flux qui doit être appliqué.
- Un `session_id` pour la poignée de main et un bloc de sel `cache_tagging` pour dériver
  affinités de garde pour la session et les balises HMAC.
- Conseils de circuit et de sélection de garde (`circuit`, `guard_selection`,
  `route_hints`) pour que les intégrateurs du navigateur exponent une interface utilisateur plus riche avant
  de ouvrir les ruisseaux.
- `telemetry_v2` avec boutons de réglage et de confidentialité pour instrument local.
- Cada `STREAM_ACK_OK` incluant `cache_tag_hex`. Clientes Espelham ou Valor sans en-tête
  `x-sorafs-cache-tag` pour émettre les exigences HTTP ou TCP pour sélectionner la garde
  dans le cache, les cryptographies permanentes sont enregistrées dans le répertoire.

Esses campos fazem parte do schema atual; les clients doivent utiliser le groupe
compléter la négociation de flux.

## 2. Sémantique des erreurs

L'explorateur demande de vérifier les valeurs de capacité et les budgets avant de l'utiliser
un seul octet est transféré. Comme falhas se enquadram em trois catégories:

1. **Falhas de elegibilidade (pré-vol).** Provedores sem capacidade de range,
   annonces expirados ou telemetria obsoleta sao registrados no artefato do
   tableau de bord et omissions de l'agenda. Résumé de la pré-installation CLI ou du tableau
   `ineligible_providers` avec des lames pour que les opérateurs inspectent la dérive de
   gouverner les journaux sem raspar.
2. **Écrire lors de l'exécution.** Chaque fournisseur a des erreurs consécutives. Quando
   `provider_failure_threshold` et atingido, le fournisseur et la marque comme `disabled`
   pelo restante da session. Se todos os provenores transicionarem para `disabled`,
   l'orquestrador retorna
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Abortos deterministas.** Limites rigidos surgem como erros estruturados :
   - `MultiSourceError::NoCompatibleProviders` — le manifeste exige une durée de
     chunks ou alinhamento que os provenores restantes nao conseguem honrar.
   - `MultiSourceError::ExhaustedRetries` — le budget de tentatives par morceau est foi
     consommé.
   - `MultiSourceError::ObserverFailed` — observadores en aval (crochets de
     streaming) rejeitaram um chunk verificado.

Chaque erreur incorpore l'indice du morceau problématique et, lorsqu'il est disponible, à la raison
final de falha do provenor. Traiter ces erreurs comme bloqueurs de version —
réessaye avec une entrée reproduite à falha mangé que la publicité, une télémétrie
ou a saude do provenor sous-jacente mudem.

### 2.1 Persistance du tableau de bordQuand `persist_path` est configuré, l'orchestre écrit la finale du tableau de bord
apos cada run. Le document JSON contient :

- `eligibility` (`eligible` ou `ineligible::<reason>`).
- `weight` (peso normalisé attribué à cette exécution).
- métadonnées du `provider` (identifiant, points de terminaison, budget de correspondance).

Archiver des instantanés sur le tableau de bord avec les artefatos de release pour que les décisions soient prises
de l'infrastructure et du déploiement sont audités en permanence.

## 3. Télémétrie et dépuration

### 3.1 Métriques Prometheus

L'orquestrador émet les mesures suivantes via `iroha_telemetry` :

| Métrique | Étiquettes | Description |
|---------|--------|---------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Gauge de récupère les orchestres em voo. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Histogramme enregistrant la latence de récupération de pont à pont. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Contador de falhas terminais (réessaye esgotados, semprovoreres, falha de observateur). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Contacteur de tentatives de nouvelle tentative par le fournisseur. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Contador de falhas de provenor na sessao qui levam a desabilitacao. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Contagem de décisions politiques anonymisées (cumprida vs brown-out) liées à la phase de déploiement et au motif de repli. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Histogramme de participation des relais PQ non associé à SoraNet sélectionné. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Histogramme des ratios d'offre de relais PQ aucun instantané du tableau de bord. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Histogramme du déficit politique (écart entre les niveaux et la participation du PQ réel). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Histogramme de participation des relais classiques utilisé à chaque session. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Histogramme des contagènes des relais classiques sélectionnés par session. |

Intégrer des mesures dans les tableaux de bord de mise en scène avant d'activer les boutons de
produire. La disposition recommandée est celle du plan d'observation SF-6 :

1. **Récupère ativos** — alerta se o gauge sobe sem achèvements correspondants.
2. **Razao de retries** — avisa quando contadores `retry` dépasse les lignes de base
   historiques.
3. **Falhas de provenor** — dispara alertas no pager quando qualquer provenor
   cruza `session_failure > 0` pendant 15 minutes.

### 3.2 Cibles de journal structurées

L'orquestrador publica eventos estruturados para cibles déterministas :- `telemetry::sorafs.fetch.lifecycle` — marqueurs `start` et `complete` avec
  contagem de chunks, tentatives et durée totale.
- `telemetry::sorafs.fetch.retry` — événements de nouvelle tentative (`provider`, `reason`,
  `attempts`) pour le manuel de triage alimentaire.
- `telemetry::sorafs.fetch.provider_failure` — prouvés désactivés en raison d'un
  erreurs répétées.
- `telemetry::sorafs.fetch.error` — faux terminaux de reprise avec `reason` e
  metadados opcionais do provenor.

Encaminhe esses fluxos para o pipeline de logs Norito existant para que a
répondre aux incidents aura une seule source de vérité. Événements de cycle de vie
exponem a mistura PQ/classica via `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` et vos contadores associés,
tornando simples intègre des tableaux de bord avec des métriques raspar. Durant les déploiements de
GA, correction du niveau de connexion dans `info` pour les événements de cycle de vie/réessayer et utiliser
`warn` pour faux terminaux.

### 3.3 CV JSON

Tanto `sorafs_cli fetch` quant au SDK Rust renvoie un résumé rédigé avec :

- `provider_reports` avec contagènes de succès/falha et se o fournisseur foi
  déstabilisé.
- `chunk_receipts` détaille le fournisseur pour chaque morceau.
- tableaux `retry_stats` et `ineligible_providers`.

Archiver ou archiver le curriculum vitae pour dépurer les fournisseurs problématiques — les reçus
mapeiam directement pour les métadonnées du journal acima.

## 4. Checklist opérationnelle

1. **Préparer la configuration sans CI.** Exécuter `sorafs_fetch` avec la configuration
   aussi, passe `--scoreboard-out` pour obtenir un visa d'éligibilité et
   comparer com o libération antérieure. Qu'importe quel fournisseur inelegivel inesperado
   interrompre une promotion.
2. **Télémétrie valide.** Garantie du déploiement des mesures exportées `sorafs.fetch.*`
   Les journaux structurés avant d'activer la récupération multi-originaux pour les utilisateurs. Un
   ausencia de metricas normalement indique que a fachada do orquestrador nao foi
   invoqué.
3. **Remplacements documentaires.** À appliquer `--deny-provider` ou `--boost-provider`
   émergent, comite o JSON (ou un appel CLI) pas de journal des modifications. Les rollbacks sont développés
   inverser ou remplacer et capturer un nouvel instantané du tableau de bord.
4. **Réexécuter les tests de fumée.** Après avoir modifié les budgets de nouvelle tentative ou les limites de
   fournisseurs, refaça o fetch do luminaire canonico
   (`fixtures/sorafs_manifest/ci_sample/`) et vérifier que les reçus des morceaux
   déterministes permanents.

Suivre les passos acima mantém ou le comportement de l'orchestre les reproduisant
les déploiements par phase et forment la télémétrie nécessaire pour répondre aux incidents.

### 4.1 Remplacements de politique

Les opérateurs peuvent réparer l'état du transport/anonymat sans l'éditer
définition de base de configuration `policy_override.transport_policy` e
`policy_override.anonymity_policy` dans votre JSON de `orchestrator` (ou fourni
`--transport-policy-override=` / `--anonymity-policy-override=` entre autres
`sorafs_cli fetch`). Quando qualquer override esta presente, o orquestrador pula
o baisse de tension de secours habituelle : si le niveau PQ est sollicité pour ne pas être satisfait, o
récupérez falha com `no providers` em vez degradar silencieusement. Ô restauration
pour le comportement correct et le tao simple quanto limpar os campos de override.