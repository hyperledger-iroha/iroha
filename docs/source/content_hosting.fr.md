---
lang: fr
direction: ltr
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T17:57:58.226177+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Voie d'hébergement de contenu
% Iroha Noyau

# Voie d'hébergement de contenu

La voie de contenu stocke de petits ensembles statiques (archives tar) sur la chaîne et sert
fichiers individuels directement à partir de Torii.

- **Publier** : soumettre `PublishContentBundle` avec une archive tar, expiration facultative
  hauteur et un manifeste facultatif. L'ID du bundle est le hachage Blake2b du
  archive tar. Les entrées Tar doivent être des fichiers normaux ; les noms sont des chemins UTF-8 normalisés.
  Les limites de taille/chemin/nombre de fichiers proviennent de la configuration `content` (`max_bundle_bytes`,
  `max_files`, `max_path_len`, `max_retention_blocks`, `chunk_size_bytes`).
  Les manifestes incluent le hachage d'index Norito, l'espace de données/la voie et la politique de cache
  (`max_age_seconds`, `immutable`), mode d'authentification (`public` / `role:<role>` /
  `sponsor:<uaid>`), espace réservé à la stratégie de rétention et remplacements MIME.
- **Déduplication** : les charges utiles tar sont fragmentées (64 Ko par défaut) et stockées une fois par
  hachage avec décompte de références ; retirer un paquet décrémente et élague des morceaux.
- **Servir** : Torii expose `GET /v1/content/{bundle}/{path}`. Flux de réponses
  directement depuis le magasin de morceaux avec `ETag` = hachage de fichier, `Accept-Ranges: bytes`,
  Prise en charge de la plage et Cache-Control dérivé du manifeste. Les lectures honorent le
  mode d'authentification manifeste : les réponses dépendantes du rôle et du sponsor nécessitent des réponses canoniques
  en-têtes de requête (`X-Iroha-Account`, `X-Iroha-Signature`) pour le
  compte ; les bundles manquants/expirés renvoient 404.
- **CLI** : `iroha content publish --bundle <path.tar>` (ou `--root <dir>`) maintenant
  génère automatiquement un manifeste, émet un `--manifest-out/--bundle-out` facultatif et
  accepte `--auth`, `--cache-max-age-secs`, `--dataspace`, `--lane`, `--immutable`,
  et remplacements `--expires-at-height`. `iroha content pack --root <dir>` construit
  une archive tar déterministe + manifeste sans rien soumettre.
- **Config** : les boutons de cache/auth se trouvent sous `content.*` dans `iroha_config`
  (`default_cache_max_age_secs`, `max_cache_max_age_secs`, `immutable_bundles`,
  `default_auth_mode`) et sont appliqués au moment de la publication.
- **SLO + limites** : `content.max_requests_per_second` / `request_burst` et
  Capuchon `content.max_egress_bytes_per_second` / `egress_burst_bytes` côté lecture
  débit ; Torii applique les deux avant de servir les octets et les exportations
  `torii_content_requests_total`, `torii_content_request_duration_seconds` et
  Métriques `torii_content_response_bytes_total` avec étiquettes de résultats. Latence
  les cibles vivent sous `content.target_p50_latency_ms` /
  `content.target_p99_latency_ms` / `content.target_availability_bps`.
- **Contrôle des abus** : les tranches de taux sont saisies par UAID/jeton API/IP distante, et un
  La protection PoW en option (`content.pow_difficulty_bits`, `content.pow_header`) peut
  être requis avant la lecture. Les valeurs par défaut de disposition des bandes DA proviennent de
  `content.stripe_layout` et sont repris dans les hachages des reçus/manifestes.
- **Reçus et preuves DA** : les réponses réussies sont jointes
  `sora-content-receipt` (octets `ContentDaReceipt` encadrés en base64 Norito) transportant
  `bundle_id`, `path`, `file_hash`, `served_bytes`, la plage d'octets servie,
  `chunk_root` / `stripe_layout`, engagement PDP facultatif et horodatage donc
  les clients peuvent épingler ce qui a été récupéré sans relire le corps.

Références clés :- Modèle de données : `crates/iroha_data_model/src/content.rs`
- Exécution : `crates/iroha_core/src/smartcontracts/isi/content.rs`
- Gestionnaire Torii : `crates/iroha_torii/src/content.rs`
- Aide CLI : `crates/iroha_cli/src/content.rs`