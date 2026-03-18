---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-config.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: orchestrator-config
כותרת: Configuration de l'orchestraateur SoraFS
sidebar_label: Configuration de l'orchestrator
תיאור: Configurer l'orchestrateur de fetch multi-source, interpreter les echecs et deboguer la telemetrie.
---

:::הערה מקור קנוניק
Cette page reflète `docs/source/sorafs/developer/orchestrator.md`. Gardez les deux copies Syncées jusqu'à ce que la documentation héritée soit retirée.
:::

# Guide de l'orchestrator de fetch multi-source

L'orchestrator de fetch multi-source de SoraFS pilote des telechargements
parallèles et déterministes depuis l’ensemble de fournisseurs publié dans des
מודעות soutenus par la governance. מדריך להגדיר הערות מפורש
l'orchestrateur, quels signaux d'échec attendre תליון להפצות ותליונים
flux de télémétrie exposent des indikers de santé.

## 1. תצורת Vue d'ensemble de la

L'orchestrator fusionne trois sources de configuration:

| מקור | Objectif | הערות |
|--------|--------|-------|
| `OrchestratorConfig.scoreboard` | נרמל את הפורמטים של הארבעה, תקף לתקשורת ותמשך את לוח התוצאות של JSON לשימוש בביקורות. | Adossé à `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Applique des limites d'exécution (תקציבים של ניסיון חוזר, הסכמה, bascules de vérification). | Mappé à `FetchOptions` ב-`crates/sorafs_car::multi_fetch`. |
| Paramètres CLI / SDK | Limite le nombre de pairs, attache des régions de télémétrie וחשיפה לפוליטיקה של הכחשה/חיזוק. | `sorafs_cli fetch` חשיפת כיוון דגלי ces; les SDK les propagent via `OrchestratorConfig`. |

Les helpers JSON dans `crates/sorafs_orchestrator::bindings` sérialsent la
קונפיגורציה הושלמה ב-Norito JSON, עבור כריכות SDK ניידות רנדנט
אוטומציה.

### 1.1 דוגמה לתצורת JSON

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

Persistez le fichier via l’empilement habituel `iroha_config` (`defaults/`, משתמש,
בפועל) afin que les déploiements déterministes héritent des mêmes limites sur
les nœuds. יוצקים את הפרופיל של תשובה ישיר בלבד aligné sur le rollout SNNet-5a,
consultez `docs/examples/sorafs_direct_mode_policy.json` et les אינדיקציות
associées dans `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 ביטולי התאמה

SNNet-9 intègre la conformité pilotée par la governance dans l'orchestrateur.
חפץ חדש `compliance` בתצורת Norito לכידת JSON
carve-outs qui forcent le pipeline de fetch en mode ישירות בלבד:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` declare les codes ISO‑3166 alpha‑2 où opère cette
  instance de l'orchestrateur. Les codes sont normalisés en majuscules lors du
  ניתוח.
- `jurisdiction_opt_outs` reflète le registre de governance. Lorsqu'une
  שיפוט אופרטור מכשיר dans la list, l'orchestrateur להטיל
  `transport_policy=direct-only` et émet la raison de fallback
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` רשימה les digests de manifest (CIDs masqués, encodés en
  הקסדצימלי מג'וסקול). Les payloads correspondants forcent aussi une
  יישור ישיר בלבד וחשיפה ל-fallback
  `compliance_blinded_cid_opt_out` בטלפון.
- `audit_contacts` הרשמה ל-URI que la governance attend des opérateurs
  dans leurs playbooks GAR.
- `attestations` לכידת את חבילות הקונפורמציה החתומות qui justifient la
  פוליטיקה. Chaque entrée définit un `jurisdiction` optionnel (קוד ISO-3166
  alpha‑2), un `document_uri`, le `digest_hex` canonique à 64 caractères, le
  חותמת זמן של מיסיון `issued_at_ms`, et un `expires_at_ms` אופציונלי. Ces
  חפצי אמנות alimentent la checklist d'audit de l'orchestrateur afin que les
  outils de gouvernance puissent relier les overrides aux documents signés.

Fournissez le bloc de conformité via l'empilement habituel de configuration pour
que les opérateurs reçoivent des overrides déterministes. ל'תזמורת
applique la conformité _après_ les רמזים למצב כתיבה: même si un SDK demande
`upload-pq-only`, ביטולי הסכמת שיפוט או נסיעות בסיסיות
vers le transport direct-only et échouent rapidement lorsqu'aucun fournisseur
conforme n'existe.

Les catalogs canonique d'opt-out résident dans
`governance/compliance/soranet_opt_outs.json` ; le Conseil de gouvernance publie
les mises à jour via des releases taggées. הושלם דוגמה לתצורה
(כולל אישורים) est disponible dans
`docs/examples/sorafs_compliance_policy.json`, et le processus operationnel est
capturé dans le
[playbook de conformité GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 פרמטרים CLI & SDK| דגל / אלוף | אפפט |
|-------------|-------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Limite le nombre de fournisseurs qui passant le filter du לוח התוצאות. Mettez `None` pour utiliser tous les fournisseurs éligibles. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Plafonne les retries par chunk. Dépasser la limite déclenche `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | הזרקת צילומי מצב של חביון/איכה בלוח התוצאות. Une télémétrie périmée au-delà de `telemetry_grace_secs` render les fournisseurs inéligibles. |
| `--scoreboard-out` | Persiste le scoreboard calculé (זכאים פורניסטים + לא זכאים) לשפוך בדיקה לאחר הריצה. |
| `--scoreboard-now` | Surchage le timestamp du scoreboard (secondes Unix) pour garder les captures de fixtures déterministes. |
| `--deny-provider` / הוק de politique de score | Exclut des fournisseurs de manière déterministe sans supprimer les adverts. השתמש ברשימה השחורה בתגובה מהירה. |
| `--boost-provider=name:delta` | Ajuste les crédits de round-robin pondéré d'un fournisseur sans toucher aux poids de gouvernance. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | כללי השיטות והיומנים של לוחות המחוונים נועדו להופיע באופן מעורפל. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | Par défaut `soranet-first` תחזוקה que l'orchestrator multi-source est la base. Utilisez `direct-only` lors d'un הורדת דירוג או d'une directive de conformité, et réservez `soranet-strict` aux pilotes PQ-only ; les overrides de conformité restent le plafond dur. |

SoraNet-first est désormais le défaut livré, et les rollbacks doivent citer le
כתב bloquant SNNet. Après la Graduation de SNNet-4/5/5a/5b/6a/7/8/12/13,
la governance durcira la posture requise (vers `soranet-strict`); d'ici là, les
seuls עוקף את המניעים של תקרית בפרטיות `direct-only`, et ils
sont à consigner dans le log de rollout.

Tous les flags ci-dessus acceptent la תחביר `--` dans `sorafs_cli fetch` et le
binaire orienté developpeurs `sorafs_fetch`. Les SDK חשוף אפשרויות les mêmes
באמצעות סוגים של בוני דה.

### 1.4 Gestion du cache des guards

La CLI câble désormais le sélecteur de guards SoraNet pour permettre aux
opérateurs d'épingler les relays d'entrée de manière déterministe avant le
השלמה הושלמה עבור תחבורה SNNet-5. Trois Nouveaux flags contrôlent ce flux :| דגל | Objectif |
|------|--------|
| `--guard-directory <PATH>` | Pointe vers un fichier JSON décrivaant le consensus de relays le plus récent (sous-ensemble ci-dessous). Passer le directory rafraîchit le cache des guards avant l’exécution du fetch. |
| `--guard-cache <PATH>` | Persiste le `GuardSet` encode en Norito. Les exécutions suivantes réutilisent le cache même sans nouveau directory. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | עוקף optionnels pour le nombre de guards d'entrée à épingler (par défaut 3) et la fenêtre de rétention (par défaut 30 jours). |
| `--guard-cache-key <HEX>` | Clé optionnelle de 32 octets utilisée pour taguer les caches de guard avec un MAC Blake3 afin de vérifier le fichier avant réutilisation. |

מטענים שימושיים במדריך השומרים וקומפקטית:

Le flag `--guard-directory` attend désormais unloadload `GuardDirectorySnapshotV2`
מקודד ב-Norito. התוכן הבינארי של תמונת מצב:

- `version` — גרסה דומה (בפועל `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  מדדי קונצנזוס devant correspondre à chaque certificat intégré.
- `validation_phase` — שער פוליטיקה דה אישורים (`1` = autoriser une
  חתימת seule Ed25519, `2` = חתימות פרפר לס כפולות, `3` = exiger
  חתימות כפולות).
- `issuers` — émetteurs de governance avec `fingerprint`, `ed25519_public` et
  `mldsa65_public`. Les טביעות אצבעות מגיעות
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` - רשימה אחת של חבילות SRCv2 (סדרה
  `RelayCertificateBundleV2::to_cbor()`). צרור צ'אק כולל תיאור דו
  ממסר, les flags de capacité, la politique ML-KEM et les חתימות כפילות
  Ed25519/ML-DSA-65.

La CLI vérifie chaque bundle contre les clés d'émetteurs déclarées avant de
fusionner ספריית המטמון של השומרים. Les esquisses JSON héritées ne
sont plus acceptees ; לתמונות SRCv2 יש צורך.

Appelez la CLI avec `--guard-directory` pour fusionner le consensus le plus
récent avec le cache existant. Le sélecteur conserve les guards épinglés הדרן
valides dans la fenêtre de rétention et éligibles dans le directory; les
נובו ממסרים remplacent les entrées expirées. Après un fetch réussi, le cache
mis à jour est récrit dans le chemin fourni via `--guard-cache`, gardant les
מפגשים suivantes déterministes. Les SDK reprodusent le même comportement en
המערער `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
et en injectant le `GuardSet` résultant dans `SorafsGatewayFetchOptions`.`ml_kem_public_hex` permet au sélecteur de prioriser les guards capables PQ
תליון להפצת SNNet-5. Les toggles d'étape (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) dégradent désormais automatiquement les
ממסרים קלאסיים: lorsqu'un guard PQ est disponible, le sélecteur supprime les
pins classiques en trop afin que les sessions suivantes favorisent les
לחיצות ידיים היברידיות. Les résumés CLI/SDK exposent le mix résultant via
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` et les champs associés de candidats/déficit/delta de
אספקה, רנדנט מפורש ל-Brownouts ו-fallbacks classiques.

Les directories de guards peuvent désormais embarquer un bundle SRCv2 complet via
`certificate_base64`. L'orchestrator décode chaque צרור, revalide les
חתימות Ed25519/ML-DSA et conserve le certificat analysé aux côtés du cache
דה שומרים. Lorsqu'un certificat est présent, il devient la source canonique pour
les clés PQ, les préférences de handshake et la pondération; les Certificates
expirés sont écartés et le sélecteur revient aux champs hérités du descripteur.
Les certificats se propagent dans la gestion du cycle de vie des circuits et
חשיפת sont דרך `telemetry::sorafs.guard` et `telemetry::sorafs.circuit`, qui
consignent la fenêtre de validité, les suites de handshake et l'observation ou
חתימות non de doubles pour chaque guard.

לנצל את העוזרים.

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` טלפון ובדיקת תמונת מצב SRCv2 avant de l'écrire sur disk,
tandis que `verify` rejoue le pipeline de validation pour des artefacts issus
d'autres équipes, en émettant un resumé JSON qui reflète la sortie du sélecteur
de guards CLI/SDK.

### 1.5 Gestionnaire du cycle de vie des circuits

Lorsque le directory de relays et le cache de guards sont fournis, l'orchestrateur
active le gestionnaire du cycle de vie des circuits pour pré-construire et
renouveler des circuits SoraNet avant chaque להביא. La configuration se trouve
sous `OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) באמצעות deux
אלופי נובו:

- `relay_directory`: העבר את תמונת המצב של המדריך SNNet-3 עבור כל
  sauts middle/exit soient sélectionnés de manière déterministe.
- `circuit_manager`: Configuration optionnelle (activée par défaut) contrôlant le
  TTL des circuits.

Norito JSON מקבל את העיצוב ללא גוש `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Les SDK transmettent les données de directory via
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), et la CLI le câble automatiquement dès que
`--guard-directory` est fourni (`crates/iroha_cli/src/commands/sorafs.rs:365`).Le gestionnaire renouvelle les circuits lorsque les métadonnées de guard changent
(נקודת קצה, clé PQ ou timestamp d'épinglage) או lorsque le TTL expire. לה עוזר
`refresh_circuits` invoqué avant chaque fetch (`crates/sorafs_orchestrator/src/lib.rs:1346`)
émet des logs `CircuitEvent` pour que les opérateurs puissent tracer les décisions
liées au cycle de vie. מבחן להשרות
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) השהייה יציבה
sur trois rotations de guards; voir le rapport associé dans
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Proxy QUIC מקומי

L'orchestrator peut optionnelle lancer un proxy QUIC local afin que les
הרחבות navigateur et les adaptateurs SDK n'aient pas à gérer les certificats
ni les clés du cache de guards. Le proxy se lie à une adresse loopback, termine
les connexions QUIC et renvoie un manifest Norito décrivant le certificat et la
clé de cache de guard optionnelle au client. Les événements de transport émis par
le proxy sont מתחברת דרך `sorafs_orchestrator_transport_events_total`.

Activez le proxy via le nouveau bloc `local_proxy` ב-JSON de l'orchestrator:

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
```- `bind_addr` contrôle l'adresse d'écoute du proxy (ניצול הנמל `0` pour
  demander un port éphémère).
- `telemetry_label` se propage aux métriques pour que les dashboards distinguent
  les proxies des sessions de fetch.
- `guard_cache_key_hex` (אופציונלי) permet au proxy d'exposer le même cache de
  guards à clé que les CLI/SDK, gardant les extensions navigateur alignées.
- `emit_browser_manifest` bascule le retour d'un manifest que les extensions
  peuvent stocker et valider.
- `proxy_mode` בחר את מקום התנועה (`bridge`) או
  s'il n'émet que des métadonnées pour que les SDK ouvrent eux-mêmes des circuits
  SoraNet (`metadata-only`). Le proxy est par défaut en `bridge`; choisissez
  `metadata-only` quand un poste doit exposer le manifest sans relayer les flux.
- `prewarm_circuits`, `max_streams_per_circuit` ו-`circuit_ttl_hint_secs`
  חשיפת רמזים משלימים au navigateur afin de budgéter les flux
  מקבילים ומסבירים את רמת הניצול מחדש של המעגלים.
- `car_bridge` (אופציונלי) pointe vers un cache local d'archives CAR. לה אלוף
  `extension` contrôle le suffixe ajouté lorsque la cible omet `*.car` ; définissez
  `allow_zst = true` pour servir direction des payloads `*.car.zst` טרום-קומפרסים.
- `kaigi_bridge` (אופציונלי) לחשוף את המסלולים Kaigi spoolées au proxy. לה אלוף
  `room_policy` מודעה si le bridge fonctionne en mode `public` ou
  `authenticated` afin que les clients navigateur pre-sélectionnent les labels
  GAR appriés.
- `sorafs_cli fetch` לחשוף את העקיפות `--local-proxy-mode=bridge|metadata-only`
  et `--local-proxy-norito-spool=PATH`, קבוע דה basculer le mode d'exécution
  ou de pointer vers des spools alternatifs sans modifier la politique JSON.
- `downgrade_remediation` (אופציונלי) להגדיר את ה-hook degradation automatique.
  Lorsqu'il est activé, l'orchestrateur surveille la télémétrie des relays pour
  détecter des rafales de downgrade et, après dépassement du `threshold` dans la
  fenêtre `window_secs`, force le proxy local vers `target_mode` (par défaut
  `metadata-only`). אין בעיה להורדת דירוג מפסיקים, le proxy revient au
  `resume_mode` לפני `cooldown_secs`. Utilisez le tableau `modes` מגביל לשפוך
  le déclencheur à des rôles de relay spécifiques (par défaut les relays d'entrée).

Quand le proxy tourne en mode bridge, il sert deux services applicatifs:- **`norito`** - la cible de flux du client est résolue par rapport à
  `norito_bridge.spool_dir`. Les cibles sont sanitizées (pas de traversal, pas de
  chemins absolus), et lorsqu'un fichier n'a pas d'extension, le suffixe configuré
  est appliqué avant de streamer le payload tel quel vers le navigateur.
- **`car`** - les cibles de flux se résolvent dans `car_bridge.cache_dir`, héritent
  de l'extension par défaut configurée et rejettent les payloads compressés sauf
  si `allow_zst` פעיל. Les bridges réussis répondent avec `STREAM_ACK_OK`
  avant de transférer les octets de l'archive afin que les clients puissent
  צינור לאימות.

Dans les deux cas, le proxy fournit l'HMAC du cache-tag (quand une clé de cache
était présente lors du handshake) et enregistre les codes de raison de télémétrie
`norito_*` / `car_*` pour que les לוחות מחוונים ייחודיים להפיכה
בהצלחה, les fichiers manquants et les échecs de sanitisation.

`Orchestrator::local_proxy().await` לחשוף את הידית en cours pour que les
המערערים puissent lire le PEM du certificat, récupérer le manifest navigateur
ou demander un arrêt gracieux à la fermeture de l'application.

Lorsque le proxy est active, il sert désormais des enregistrements **manifest v2**.
En plus du certificat existant et de la clé de cache de guard, la v2 ajoute :

- `alpn` (`"sorafs-proxy/1"`) et un tableau `capabilities` pour que les clients
  מאשר את פרוטוקול השטף à utiliser.
- Un `session_id` par לחיצת יד et un bloc de salage `cache_tagging` pour dériver
  des affinités de guard par session et des tags HMAC.
- Des hints de circuit et de sélection de guard (`circuit`, `guard_selection`,
  `route_hints`) afin que les intégrations navigateur exposent une UI plus riche
  avant l'ouverture des flux.
- `telemetry_v2` avec des knobs d'échantillonnage et de confidentialité pour
  המקום של מכשירים.
- Chaque `STREAM_ACK_OK` כולל `cache_tag_hex`. Les clients reflètent la valeur
  dans l'en-tête `x-sorafs-cache-tag` lors de requêtes HTTP או TCP afin que les
  sélections de guard en cache restent chiffrées au repos.

Ces champs s'ajoutent au manifest v2; פחות לקוחות לא מזמינים
explicitement les clés qu'ils supportent et ignorer le reste.

## 2. Sémantique des échecs

L'orchestrator applique des vérifications strictes de capacités et de budgets
אוונט דה טרנספירר לה מונדרה אוקטט. Les échecs tombent dans trois קטגוריות:1. **Échecs d'éligibilité (pré-vol).** Les fournisseurs sans capacité de plage,
   aux adverts expirés ou à la télémétrie périmée sont consignés dans l'artefact
   אתה לוח תוצאות וחסר דה לה תכנון. Les קורות חיים CLI remplisent le
   tableau `ineligible_providers` avec les raisons afin que les opérateurs
   המפקח על הממשל ללא מגרד בולי עץ.
2. **Épuisement à l'exécution.** Chaque fournisseur suit les échecs consécutifs.
   Une fois `provider_failure_threshold` atteint, le fournisseur est marqué
   `disabled` pour le reste de la session. Si tous les fournisseurs deviennent
   `disabled`, l'orchestrateur renvoie
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Arrêts déterministes.** Les limites dures remontent sous forme d'erreurs
   מבנים:
   - `MultiSourceError::NoCompatibleProviders` — המניפסט דורש un span de
     chunks ou un alignement que les fournisseurs restants ne peuvent honorer.
   - `MultiSourceError::ExhaustedRetries` — התקציב של שוב ושוב לפי נתח אחד
     consommé.
   - `MultiSourceError::ObserverFailed` - לצופים במורד הזרם (הווים de
     סטרימינג) ont rejeté un chunk vérifié.

Chaque erreur embarque l'index du chunk fautif et, lorsque disponible, la raison
final d'échec du fournisseur. Traitez ces erreurs comme des bloqueurs de release
- les retries avec la même entrée reproduiront l'échec tant que l'advert, la
telemetrie ou la santé du fournisseur sous-jacent ne changent pas.

### 2.1 עמידה בלוח התוצאות

Quand `persist_path` est configuré, l'orchestrateur écrit le לוח התוצאות הגמר
ריצת אפרה צ'אק. תוכן המסמך JSON:

- `eligibility` (`eligible` או `ineligible::<reason>`).
- `weight` (פויד normalisé assigné pour ce run).
- métadonnées du `provider` (זיהוי, נקודות קצה, תקציב בהסכמה).

ארכיון תמונות בזק של לוח התוצאות עם חפצי אמנות לשחרור אחרונים
בחירה ברשימה השחורה והפצת חומרי ביקורת.

## 3. Télémétrie et débogage

### 3.1 Métriques Prometheus

L'orchestrateur émet les métriques suivantes via `iroha_telemetry` :| מטריקה | תוויות | תיאור |
|--------|--------|------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Gauge des fetches orchestrés en vol. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | נרשם היסטוגרמה la latence de fetch de bout en bout. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Compteur des échecs terminaux (Retries épuisés, aucun fournisseur, échec observateur). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Compteur des tentatives de retry par fournissur. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Compteur des échecs de fournisseur au niveau session menant à la désactivation. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Compte des décisions de politique d'anonymat (תקופה לעומת בראיון) קבוצתיות ל-Etape de rollout et raison de fallback. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | היסטוגרמה דה לה חלק דה ממסרים PQ dans le set SoraNet sélectionné. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Histogramme des ratios d'offre de relays PQ ב-Snapshot של לוח התוצאות. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Histogramme du déficit de politique (écart entre l'objectif et la part PQ réelle). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | היסטוגרמה דה לה חלק דה ממסרים קלאסיים שימושיים בסשן צ'אקה. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Histogramme des compteurs de relays classiques sélectionnés par session. |

Intégrez ces métriques dans les dashboards de staging avant d'activer les knobs
בהפקה. La disposition recommandée reflète le plan d'observabilité SF-6:

1. **מחזיר אקטיביות** - התראה על מתכתבי ה-Monte Sans Completions.
2. **יחס של ניסיונות חוזרים** - הימנע מנסיונות חוזרים `retry` dépassent les
   היסטוריית קווים בסיסיים.
3. **Échecs fournisseurs** - déclenche les alertes pager quand un fournisseur
   pass `session_failure > 0` ב-15 דקות.

### 3.2 Cibles de logs structurés

L'orchestrator publie des événements structurés vers des cibles déterministes:

- `telemetry::sorafs.fetch.lifecycle` — מרקרים `start` et `complete` avec
  nombre de chunks, retries et durée total.
- `telemetry::sorafs.fetch.retry` — événements de rery (`provider`, `reason`,
  `attempts`) pour l’analyse manuelle.
- `telemetry::sorafs.fetch.provider_failure` — fournisseurs désactivés pour
  שגיאות répétées.
- `telemetry::sorafs.fetch.error` — échecs terminaux résumés avec `reason` et
  métadonnées optionnelles du fournisseur.Acheminez ces flux vers le pipeline de logs Norito קיים afin que la réponse
אירועי aux ait une source unique de vérité. Les événements de cycle de vie
exposent le mix PQ/classique דרך `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` וחברות מתחרים,
ce qui facilite le câblage des לוחות מחוונים sans scraper les métriques. תליון
les rollouts GA, bloquez le niveau de logs à `info` pour les événements de cycle
de vie/retry et utilisez `warn` pour les erreurs terminales.

### 3.3 קורות חיים JSON

`sorafs_cli fetch` et le SDK תוכן חלודה ומבנה קורות חיים:

- `provider_reports` avec les compteurs succès/échec et l’état de désactivation.
- `chunk_receipts` détaillant quel fournissur a satisfait chaque chunk.
- les tableaux `retry_stats` et `ineligible_providers`.

Archivez le fichier de résumé pour déboguer les fournisseurs défaillants : les
קבלות מאפיין כיוון aux métadonnées de logs ci-dessus.

## 4. רשימת פעולות

1. **מכין את התצורה ב-CI.** Lancez `sorafs_fetch` avec la
   כבל תצורה, passez `--scoreboard-out` pour capturer la vue
   d'éligibilité, et faites un diff avec le release précédent. טאוט פורניסיור
   לא כשיר inattendu bloque la promotion.
2. ** Valider la télémétrie.** Assurez-vous que le déploiement exporte les
   métriques `sorafs.fetch.*` et les logs structurés avant d'activer les fetches
   רב מקורות pour les utilisateurs. L'absence de métriques indique souvent
   que la façade de l'orchestraateur n'a pas été appelée.
3. **Documenter les overrides.** Lors d'un `--deny-provider` ou `--boost-provider`
   d'urgence, consignez le JSON (ou l'invocation CLI) ב-le changelog. לס
   rollbacks doivent révoquer l'override et capturer un nouveau snapshot de
   לוח התוצאות.
4. **מתמחה בבדיקות עשן
   caps de fournisseurs, refetcher la fixture canonique
   (`fixtures/sorafs_manifest/ci_sample/`) et vérifiez que les receipts de chunks
   restent déterministes.

Suivre les étapes ci-dessus garde le comportement de l'orchestrateur
ניתן לשחזור בהפצות לשלבים וארבעה שנייה
לה réponse aux תקריות.

### 4.1 עוקף את הפוליטיקה

Les opérateurs peuvent épingler la phase de transport/anonymat active sans
משנה את תצורת הבסיס דה-פיננסיסנט
`policy_override.transport_policy` et `policy_override.anonymity_policy` dans
leur JSON `orchestrator` (ou en fournissant
`--transport-policy-override=` / `--anonymity-policy-override=` à
`sorafs_cli fetch`). Lorsqu'un override est present, l'orchestrateur saute le
fallback brownout habituel : סי le niveau PQ demandé ne peut pas être satisfait,
le fetch échoue avec `no providers` au lieu de dégrader silencieusement. לה
Retour au comportement par défaut מורכב מפשטות א ודר לס Champs
d'override.