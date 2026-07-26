---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : configuration de l'orchestrateur
titre : SoraFS آرکسٹریٹر کنفیگریشن
sidebar_label : آرکسٹریٹر کنفیگریشن
description: ملٹی سورس fetch آرکسٹریٹر کو کنفیگر کریں، ناکامیوں کی تشریح کریں، اور ٹیلیمیٹری آؤٹ پٹ ڈی بگ کریں۔
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/developer/orchestrator.md` کی عکاسی کرتا ہے۔ جب تک پرانی ڈاکیومنٹیشن ریٹائر نہ ہو جائے، دونوں نقول کو ہم آہنگ رکھیں۔
:::

# ملٹی سورس fetch آرکسٹریٹر گائیڈ

SoraFS Vous êtes en train de chercher des publicités en ligne
پرووائیڈرز سیٹ سے ڈٹرمنسٹک اور متوازی ڈاؤن لوڈز چلاتا ہے۔ یہ گائیڈ وضاحت کرتی
ہے کہ آرکسٹریٹر کیسے کنفیگر کیا جائے، رول آؤٹس کے دوران کون سی ناکامی سگنلز
متوقع ہیں، اور کون سے ٹیلیمیٹری اسٹریمز صحت کے اشارے ظاہر کرتے ہیں۔

## 1. کنفیگریشن اوور ویو

آرکسٹریٹر تین سورسز کی کنفیگریشن کو مرج کرتا ہے:

| سورس | مقصد | نوٹس |
|------|------|------|
| `OrchestratorConfig.scoreboard` | Les poids et les poids sont des poids lourds et des poids lourds. Tableau de bord JSON en anglais | `crates/sorafs_car::scoreboard::ScoreboardConfig` en français |
| `OrchestratorConfig.fetch` | رن ٹائم حدود لگاتا ہے (budgets de nouvelle tentative, limites de concurrence, bascules de vérification)۔ | `crates/sorafs_car::multi_fetch` et `FetchOptions` sont en vente libre |
| CLI / SDK | peers کی تعداد محدود کرتے ہیں، régions de télémétrie لگاتے ہیں، اور deny/boost پالیسیز ظاہر کرتے ہیں۔ | `sorafs_cli fetch` یہ flags براہ راست دیتا ہے؛ SDKs `OrchestratorConfig` pour les kits de développement logiciel |

`crates/sorafs_orchestrator::bindings` pour les assistants JSON et Norito
JSON permet de sérialiser des liens entre les liaisons SDK et les liens vers les fichiers SDK.
بنایا جا سکے۔

### 1.1 Version JSON

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

Il s'agit d'une superposition `iroha_config` (`defaults/`, utilisateur, réel)
Les plus grands déploiements de déploiements sont les limites des limites de sécurité
Déploiement de SNNet-5a et solution de secours directe uniquement
`docs/examples/sorafs_direct_mode_policy.json` ici
`docs/source/sorafs/direct_mode_pack.md` دیکھیں۔

### 1.2 Remplacements de conformité

SNNet-9 est un système d'exploitation de réseau SNNet-9 NoritoJSON
La loi sur les exclusions `compliance` permet d'effectuer des exclusions et de les récupérer.
Il s'agit d'un service direct uniquement pour les utilisateurs suivants :

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` ISO‑3166 alpha‑2 pour le système d'exploitation
  آرکسٹریٹر انسٹینس آپریٹ کرتا ہے۔ analyse کے دوران کوڈز majuscule میں normaliser
  ہوتے ہیں۔
- `jurisdiction_opt_outs` miroir miroir noir جب کوئی آپریٹر
  juridiction فہرست میں ہو، آرکسٹریٹر `transport_policy=direct-only` نافذ
  La raison de repli `compliance_jurisdiction_opt_out` émet un message d'erreur
- Résumés de manifeste `blinded_cid_opt_outs` (CID aveugles, hexadécimal majuscule)
  ہے۔ planification directe des charges utiles correspondantes uniquement
  `compliance_blinded_cid_opt_out` solution de secours
- `audit_contacts` pour l'URI et le GAR
  playbooks میں شائع کرنے کی توقع کرتی ہے۔
- `attestations` کمپلائنس کی سائنڈ پیکٹس محفوظ کرتا ہے۔ ہر entrée ایک facultatif
  `jurisdiction` (ISO-3166 alpha-2)، `document_uri`, 64 caractères `digest_hex`,
  horodatage d'émission `issued_at_ms`, ou `expires_at_ms` facultatif pour le client
  artefacts de liste de contrôle d'audit et outils de gouvernance
  remplace les options de remplacement

کمپلائنس ذریعے دیں تاکہ آپریٹرز کو
ڈٹرمنسٹک remplace ملیں۔ آرکسٹریٹر کمپلائنس کو conseils de mode d'écriture کے _بعد_
Pour cela : le SDK `upload-pq-only` pour la juridiction et le manifeste
opt-outs pour le transport direct uniquement et pour les transports en commun conformes aux normes
fournisseur نہ ہو تو فوری فیل کرتے ہیں۔

Catalogues de désinscription canoniques `governance/compliance/soranet_opt_outs.json` میں
موجود ہیں؛ Conseil de gouvernance étiqueté communiqués کے ذریعے اپڈیٹس شائع کرتی ہے۔
Attestations سمیت مکمل مثال `docs/examples/sorafs_compliance_policy.json` میں
دستیاب ہے، اور آپریشنل پروسیس
[Playbook de conformité GAR](../../../source/soranet/gar_compliance_playbook.md)
میں درج ہے۔

### 1.3 CLI et boutons SDK| Drapeau/Champ | اثر |
|--------------|----------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | اس بات کی حد مقرر کرتا ہے کہ کتنے پرووائیڈر scoreboard فلٹر سے گزر سکیں۔ `None` پر سیٹ کریں تاکہ تمام éligible پرووائیڈرز استعمال ہوں۔ |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | فی-chunk retry حد مقرر کرتا ہے۔ حد سے تجاوز `MultiSourceError::ExhaustedRetries` اٹھاتا ہے۔ |
| `--telemetry-json` | instantanés de latence/échec et générateur de tableau de bord pour injecter des instantanés `telemetry_grace_secs` سے پرانی ٹیلیمیٹری پرووائیڈرز کو inéligible کر دیتی ہے۔ |
| `--scoreboard-out` | حساب شدہ tableau de bord (éligible + inéligible) کو پوسٹ رن معائنے کے لیے محفوظ کرتا ہے۔ |
| `--scoreboard-now` | L'horodatage du tableau de bord (secondes Unix) remplace l'appareil capture les valeurs déterministes |
| `--deny-provider` / crochet de politique de score | les publicités sont déterministes et excluent les annonces فوری بلیک لسٹنگ کے لیے مفید۔ |
| `--boost-provider=name:delta` | Les poids sont des crédits pondérés à la ronde et des crédits pondérés sont disponibles. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Il existe des journaux structurés et une vague de déploiement pour le pivot et la vague de déploiement. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | اب ڈیفالٹ `soranet-first` ہے کیونکہ ملٹی سورس آرکسٹریٹر baseline ہے۔ rétrogradation et directive de conformité `direct-only` pour les pilotes PQ uniquement `soranet-strict` pour les pilotes PQ uniquement remplacements de conformité پھر بھی سخت حد ہیں۔ |

SoraNet-first par défaut d'expédition et rollbacks et bloqueur SNNet
حوالہ دینا چاہیے۔ SNNet-4/5/5a/5b/6a/7/8/12/13 est disponible en ligne
posture requise pour la posture requise (`soranet-strict` کی طرف)؛ تب تک صرف
remplacements provoqués par incident `direct-only` pour le déploiement du déploiement
log میں ریکارڈ کرنا لازمی ہے۔

Utiliser les drapeaux `sorafs_cli fetch` et `sorafs_fetch` destiné aux développeurs
Utilisez la syntaxe `--` pour lire la syntaxe. SDK pour les constructeurs typés et les constructeurs typés
ایکسپوز کرتے ہیں۔

### 1.4 Cache de garde مینجمنٹ

CLI pour le sélecteur de garde SoraNet et le sélecteur filaire avec connecteur SNNet-5
déploiement du transport avec relais d'entrée et broches déterministes
Les drapeaux des États-Unis sont également présents dans les pays suivants :

| Drapeau | مقصد |
|------|------|
| `--guard-directory <PATH>` | JSON est un consensus de relais (sous-ensemble de sous-ensembles de consensus JSON) répertoire پاس کرنے سے fetch سے پہلے guard cache actualiser ہوتی ہے۔ |
| `--guard-cache <PATH>` | Norito-codé `GuardSet` محفوظ کرتا ہے۔ Vous avez besoin de réutiliser le cache pour réutiliser le répertoire |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | gardes d'entrée (3) et fenêtre de rétention (30 $) pour remplacements facultatifs |
| `--guard-cache-key <HEX>` | clé facultative de 32 octets pour Blake3 MAC et garde-cache et balise pour réutiliser et vérifier et vérifier |

Charges utiles du répertoire Guard avec schéma compact pour les détails :

Indicateur `--guard-directory` et charge utile `GuardDirectorySnapshotV2` codée en Norito
attendez کرتا ہے۔ instantané binaire میں شامل ہے :- `version` — schéma et (فی الحال `2`).
-`directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  métadonnées de consensus et certificat intégré et correspondance avec le certificat
- `validation_phase` — porte de politique de certificat (`1` = signature Ed25519 unique
  autoriser, `2` = double signature préférée, `3` = double signature requise).
- `issuers` — émetteurs de gouvernance comme `fingerprint`, `ed25519_public`, `mldsa65_public`.
  empreintes digitales اس طرح بنائے جاتے ہیں:
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — Bundles SRCv2 en option (sortie `RelayCertificateBundleV2::to_cbor()`).
  Descripteur de relais de bundle, indicateurs de capacité, politique ML-KEM, voir Ed25519/ML-DSA-65
  doubles signatures رکھتا ہے۔

CLI et bundle avec les clés d'émetteur déclarées pour vérifier le répertoire et le répertoire
instantanés لازمی ہیں۔

`--guard-directory` pour le consensus CLI et le cache
کے ساتھ fusionner کیا جا سکے۔ sélecteur épinglé gardes کو برقرار رکھتا ہے جو ابھی
fenêtre de rétention pour les répertoires éligibles نئے relais expirés
entrées کو remplacer کرتے ہیں۔ Je vais récupérer le cache mis à jour `--guard-cache`
کے chemin پر لکھ دی جاتی ہے، جس سے اگلی سیشنز déterministe رہتی ہیں۔ SDK یہی
comportement `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
Il s'agit d'un modèle `GuardSet` et d'un `SorafsGatewayFetchOptions`.
reproduire کر سکتی ہیں۔

Sélecteur `ml_kem_public_hex` pour protections compatibles PQ
Déploiement SNNet-5 en anglais bascules de scène (`anon-guard-pq`, `anon-majority-pq`,
`anon-strict-pq`) Pour les relais classiques, pour les relais rétrogradés: pour PQ
garde de sécurité et sélecteur de broches classiques broches de sécurité
poignées de main hybrides Résumés CLI/SDK pour le mixage
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` pour le delta candidat/déficit/offre فیلڈز کے
Il y a des coupures de courant et des replis classiques et des coupures de courant

Répertoires de garde comme `certificate_base64` pour l'intégration du bundle SRCv2
سکتی ہیں۔ Le bundle permet de décoder les signatures Ed25519/ML-DSA.
revalider le certificat analysé et le cache de garde
ہے۔ Il s'agit d'un certificat, d'une clé PQ, d'une préférence de prise de contact et d'une pondération
کا source canonique بن جاتا ہے؛ les certificats expirés sont jetés کر دیے جاتے ہیں اور
gestion کے ذریعے propager ہوتے ہیں اور `telemetry::sorafs.guard` اور
`telemetry::sorafs.circuit` Surface de la fenêtre de validité et fenêtre de validité
suites de poignée de main, ainsi que des signatures doubles et des signatures doubles

Les assistants CLI et les instantanés et les éditeurs et les fonctions de synchronisation :

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` Instantané SRCv2 pour vérifier et vérifier les détails `verify`
Il s'agit de créer des artefacts et un pipeline de validation et de créer des liens
La sortie du sélecteur de garde CLI/SDK correspond à l'émission du résumé JSON et à l'émission de ce dernier.

### 1.5 Gestionnaire du cycle de vie des circuitsDans le répertoire de relais et le cache de garde, le cycle de vie du circuit est disponible.
gestionnaire de circuits SoraNet et de récupération de circuits SoraNet et de pré-construction
renouveler کیا جائے۔ کنفیگریشن `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) Il s'agit d'une question de confiance :

- `relay_directory` : instantané du répertoire SNNet-3 et sauts intermédiaires/de sortie
  déterministe طریقے سے منتخب ہوں۔
- `circuit_manager` : optionnel (optionnel et activé) et circuit TTL
  کو کنٹرول کرتی ہے۔

Norito JSON et `circuit_manager` pour le fichier :

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Données du répertoire SDK ici
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`) Retour en avant vers l'avant vers la CLI vers l'avant
Câble d'alimentation filaire pour `--guard-directory` pour ordinateur portable
(`crates/iroha_cli/src/commands/sorafs.rs:365`).

Les circuits du gestionnaire doivent renouveler les métadonnées de garde (point final, clé PQ).
horodatage épinglé) par جائے یا TTL ختم ہو۔ ہر récupérer سے پہلے invoquer ہونے والا
assistant `refresh_circuits` (`crates/sorafs_orchestrator/src/lib.rs:1346`) `CircuitEvent`
L'émission est un cycle de vie complet et un cycle de vie complet test de trempage
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) Pour les rotations de gardes
latence رپورٹ `docs/source/soranet/reports/circuit_stability.md:1` میں
دیکھیں۔

### 1.6 pour le proxy QUIC

Utilisez le proxy QUIC pour ajouter des extensions de navigateur et du SDK
adaptateurs et certificats et clés de cache de garde gérer les clés bouclage proxy
Vous pouvez lier des connexions QUIC à un lien vers une connexion Norito.
manifeste et certificat et clé de cache de garde facultative pour le certificat
le proxy émet des événements de transport par `sorafs_orchestrator_transport_events_total`
میں شمار کیا جاتا ہے۔

La version JSON est `local_proxy` pour l'activation du proxy :

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
```- `bind_addr` کنٹرول کرتا ہے کہ proxy کہاں écouter کرے (پورٹ `0` سے éphémère
  پورٹ مانگیں)۔
- `telemetry_label` permet de propager des proxys de tableaux de bord et de
  fetch sessions میں فرق کر سکیں۔
- `guard_cache_key_hex` (اختیاری) proxy et cache de garde à clé
  Les CLI/SDK permettent de synchroniser les extensions de navigateur
- `emit_browser_manifest` pour basculer entre la poignée de main et le manifeste
  واپس کرے جسے extensions ذخیرہ/verify کر سکیں۔
- `proxy_mode` est un proxy pour le pont (`bridge`) est disponible
  les métadonnées émettent des SDK et des circuits SoraNet (`metadata-only`).
  ڈیفالٹ `bridge` ہے؛ جب صرف manifeste دینا ہو تو `metadata-only` سیٹ کریں۔
- `prewarm_circuits`, `max_streams_per_circuit`, `circuit_ttl_hint_secs`
  astuces pour la mise en place de flux parallèles pour un circuit proxy
  réutiliser رویے کو سمجھ سکے۔
- `car_bridge` (اختیاری) Cache d'archives CAR pour le cache d'archives CAR `extension`
  Le suffixe est le suffixe `*.car` pour le numéro de téléphone. `allow_zst = true`
  سے `*.car.zst` براہ راست serve ہو سکتا ہے۔
- `kaigi_bridge` (اختیاری) Kaigi routes et proxy pour exposer le système `room_policy`
  Pour le pont `public` et `authenticated` pour les clients de navigateur
  Étiquettes GAR en anglais
- `sorafs_cli fetch` `--local-proxy-mode=bridge|metadata-only` et
  `--local-proxy-norito-spool=PATH` remplace le code source par JSON.
  Utiliser le mode d'exécution et la bobine de démarrage pour le mode d'exécution
- `downgrade_remediation` (اختیاری) Crochet de déclassement pour le déclassement
  جب activé ہو تو آرکسٹریٹر télémétrie de relais میں downgrade bursts دیکھتا ہے اور
  `window_secs` pour `threshold` pour le proxy local et `target_mode`
  (ڈیفالٹ `metadata-only`) pour le client ہے۔ جب déclasse رک جائیں تو proxy
  `cooldown_secs` pour `resume_mode` en ce moment Tableau `modes` et spécifique
  rôles de relais

Le mode pont proxy est également disponible pour les services d'application:

- **`norito`** — La cible de flux `norito_bridge.spool_dir` est résolue
  ہوتا ہے۔ cibles et assainir les chemins absolus (traversée et chemins absolus)
  Il y a une extension supplémentaire et un suffixe configuré pour une charge utile
  براؤزر کو stream کیا جاتا ہے۔
- **`car`** — les cibles de flux `car_bridge.cache_dir` pour résoudre le problème
  L'extension par défaut hérite de charges utiles compressées et de charges utiles compressées.
  `allow_zst` en français Ponts de construction `STREAM_ACK_OK` Construction de ponts de construction
  Il s'agit d'octets d'archive pour la vérification de la vérification du pipeline et du pipeline.

Il s'agit d'une balise de cache proxy HMAC et d'une poignée de main de clé de cache de garde
Codes de raison de télémétrie `norito_*` / `car_*` Codes de raison de télémétrie
Il y a des tableaux de bord, des fichiers manquants et des échecs de désinfection.
سمجھ سکیں۔`Orchestrator::local_proxy().await` poignée de commande et exposition des pièces de rechange
Certificat de certificat PEM pour manifeste de navigateur Fichier de manifeste de navigateur
Comment procéder à un arrêt progressif

جب activé et proxy اب **manifest v2** ریکارڈز سرور کرتا ہے۔ certificat موجودہ
Clé de cache de garde pour v2 et clé de cache :

- `alpn` (`"sorafs-proxy/1"`) et `capabilities` array pour flux
  پروٹوکول کی تصدیق کر سکیں۔
- Poignée de main pour `session_id` et bloc `cache_tagging` par session
  affinités de garde et tags HMAC
- Conseils de sélection de garde et de circuit (`circuit`, `guard_selection`, `route_hints`)
  Les intégrations de navigateurs diffusent des flux et des interfaces utilisateur plus riches.
- `telemetry_v2` pour l'instrumentation et les boutons d'échantillonnage/de confidentialité
- ہر `STREAM_ACK_OK` یں `cache_tag_hex` شامل ہوتا ہے۔ کلائنٹس اسے
  `x-sorafs-cache-tag` en-tête miroir et sélections de garde mises en cache
  reste میں crypté رہیں۔

اور v1 subset پر انحصار جاری رکھ سکتے ہیں۔

## 2. Sémantique des échecs

آرکسٹریٹر ایک بائٹ منتقل ہونے سے پہلے سخت اور budget checks نافذ کرتا
ہے۔ ناکامیاں تین زمروں میں آتی ہیں:

1. **Échecs d'éligibilité (avant le vol).** La capacité de portée نہ رکھنے والے، a expiré
   publicités, télémétrie obsolète et fournisseurs, artefact du tableau de bord, journal
   جاتا ہے اور planning سے نکال دیا جاتا ہے۔ Résumés CLI `ineligible_providers`
   tableau des raisons pour les opérateurs journaux gratter pour la gouvernance
   dérive دیکھ سکیں۔
2. **Épuisement de l'exécution.** Les échecs du fournisseur et le suivi des échecs جب
   `provider_failure_threshold` Un fournisseur d'accès à Internet en ligne
   `disabled` est disponible en français Fournisseurs de services `disabled` pour les fournisseurs
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }` واپس کرتا ہے۔
3. **Abandons déterministes.** Il existe des erreurs structurées et des erreurs :
   - `MultiSourceError::NoCompatibleProviders` — manifeste pour une durée de morceau یا
     alignement مانگتا ہے جسے باقی fournisseurs honorent نہیں کر سکتے۔
   - `MultiSourceError::ExhaustedRetries` — budget de nouvelle tentative par fragment
   - `MultiSourceError::ObserverFailed` — observateurs en aval (hameçons de streaming)
     نے morceau vérifié رد کر دیا۔

ہر erreur incriminée chunk index اور جہاں ممکن ہو raison de l'échec du fournisseur final
کے ساتھ آتا ہے۔ Les bloqueurs de libération sont activés — Les entrées et les tentatives sont terminées
تب تک échec دہراتی رہیں گے جب تک publicité، télémétrie یا prestataire de santé

### 2.1 Persistance du tableau de bord

جب `persist_path` کنفیگر ہو تو آرکسٹریٹر ہر رن کے بعد final scoreboard لکھتا ہے۔
JSON prend en charge le contenu :

- `eligibility` (`eligible` ou `ineligible::<reason>`).
- `weight` (poids normalisé attribué).
- Métadonnées `provider` (identifiant, points de terminaison, budget de concurrence).

Instantanés du tableau de bord et artefacts de publication et archives et liste noire
اور déploiement فیصلے auditable رہیں۔

## 3. Télémétrie et débogage

### 3.1 Métriques Prometheus

Le `iroha_telemetry` émet un message d'erreur :| Métrique | Étiquettes | Descriptif |
|--------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | récupérations orchestrées en vol et jauge |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | latence de récupération de bout en bout par exemple et histogramme |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | échecs de terminal et compteur (tentatives épuisées, aucun fournisseur, échec de l'observateur) |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | فی-nouvelles tentatives du fournisseur کا compteur۔ |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | échecs du fournisseur au niveau de la session, compteur et désactivation |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | politique d'anonymat et étape du nombre (rencontre vs baisse de tension) et raison de repli |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Comment SoraNet a défini le partage de relais PQ et l'histogramme |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | instantané du tableau de bord et rapports d'alimentation du relais PQ et histogramme |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | déficit politique (cible et part réelle du PQ) et histogramme |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Il s'agit d'un partage de relais classique et d'un histogramme. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Il s'agit d'un relais classique et d'un histogramme. |

Boutons de production, métriques et tableaux de bord de mise en scène, ainsi que des indicateurs de production
Le plan d'observabilité de la disposition SF-6 suit les étapes suivantes :

1. **Récupérations actives** — Les achèvements de jauge et les achèvements de jauge
2. **Taux de nouvelle tentative** — Le `retry` compteurs les lignes de base et les avertissements.
3. **Échecs du fournisseur** — 15 mois après le fournisseur `session_failure > 0`.
   alertes de téléavertisseur۔

### 3.2 Cibles de journaux structurés

Il existe des cibles déterministes et des événements structurés comme :

- `telemetry::sorafs.fetch.lifecycle` — `start` et `complete` marqueurs de cycle de vie
  Nombre de fragments, tentatives et durée totale
- `telemetry::sorafs.fetch.retry` — événements de nouvelle tentative (`provider`, `reason`, `attempts`).
- `telemetry::sorafs.fetch.provider_failure` — erreurs répétées et désactivées
  fournisseurs۔
- `telemetry::sorafs.fetch.error` — `reason` et métadonnées facultatives du fournisseur
  ساتھ pannes de terminaux۔

Flux de données et pipeline de journaux Norito pour transmettre la réponse aux incidents
کو ایک ہی source de vérité ملے۔ événements du cycle de vie PQ/mix classique
`anonymity_effective_policy`, `anonymity_pq_ratio`, `anonymity_classical_ratio`
Il s'agit de compteurs, de tableaux de bord et de grattage de métriques.
کیے بغیر wire کرنا آسان ہوتا ہے۔ Déploiements de GA et événements de cycle de vie/nouvelle tentative
Niveau de journalisation `info` pour les erreurs de terminal et `warn` pour les erreurs de terminal

### 3.3 Résumés JSON

`sorafs_cli fetch` Le résumé structuré du SDK Rust contient les éléments suivants :

- `provider_reports` pour le nombre de réussites/échecs et la désactivation du fournisseur
- `chunk_receipts` est un fournisseur d'accès à un bloc de données
- Baies `retry_stats` et `ineligible_providers`Fournisseurs de services pour le débogage et les archives de fichiers récapitulatifs — reçus
Voici les métadonnées du journal et les métadonnées

## 4. Liste de contrôle opérationnelle

1. **CI میں کنفیگریشن stage کریں۔** `sorafs_fetch` کو target config کے ساتھ چلائیں،
   vue d'éligibilité pour `--scoreboard-out` pour la version finale et le diff
   کوئی غیر متوقع promotion de fournisseur inéligible
2. ** Les récupérations multi-sources de télémétrie permettent les récupérations de données multiples.
   Pour le déploiement des métriques `sorafs.fetch.*` et l'exportation de journaux structurés
   ہے۔ metrics est une façade d'orchestrateur
   invoquer نہیں ہوئی۔
3. **Remplace les paramètres de remplacement** par `--deny-provider` et `--boost-provider`
   Il s'agit d'un JSON (invocation CLI) et d'un journal des modifications et d'un commit rollbacks
   remplacer annuler l'instantané du tableau de bord capturer l'instantané
4. **Tests de fumée et budgets de nouvelle tentative et plafonds des fournisseurs pour plus de détails
   appareil canonique (`fixtures/sorafs_manifest/ci_sample/`) pour récupérer les fichiers
   اور vérifier les reçus de morceaux déterministes رہیں۔

Il s'agit de déploiements par étapes et de déploiements reproductibles
Réponse aux incidents et télémétrie

### 4.1 Remplacements de stratégie

Étape de transport/anonymat des opérateurs et configuration de base avec broche
کر سکتے ہیں، بس `policy_override.transport_policy` اور
`policy_override.anonymity_policy` et `orchestrator` JSON sont disponibles (en anglais)
`sorafs_cli fetch` et `--transport-policy-override=` / `--anonymity-policy-override=`
پاس کریں)۔ Remplacer l'option de secours en cas de baisse de tension habituelle et sauter
Nom du produit : Le niveau PQ est disponible pour récupérer `no providers` et est disponible
ہوتا ہے بجائے خاموشی سے downgrade ہونے کے۔ comportement par défaut پر واپسی اتنی ہی
Vous pouvez remplacer les champs par d'autres.