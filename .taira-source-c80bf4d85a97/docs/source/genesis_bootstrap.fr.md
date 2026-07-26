---
lang: fr
direction: ltr
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2026-01-03T18:08:01.368173+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Genesis Bootstrap de pairs de confiance

Les pairs Iroha sans `genesis.file` local peuvent récupérer un bloc Genesis signé auprès de pairs de confiance
en utilisant le protocole d'amorçage codé Norito.

- **Protocole :** les pairs échangent `GenesisRequest` (`Preflight` pour les métadonnées, `Fetch` pour la charge utile) et
  Trames `GenesisResponse` saisies par `request_id`. Les répondeurs incluent l'identifiant de la chaîne, la clé publique du signataire,
  hachage et un indice de taille facultatif ; les charges utiles sont renvoyées uniquement sur `Fetch` et les identifiants de demande en double
  recevez `DuplicateRequest`.
- **Gardes :** les répondeurs appliquent une liste autorisée (`genesis.bootstrap_allowlist` ou les pairs de confiance
  ensemble), la correspondance d'identifiant de chaîne/de clé de publication/de hachage, les limites de débit (`genesis.bootstrap_response_throttle`) et un
  capuchon de taille (`genesis.bootstrap_max_bytes`). Les demandes en dehors de la liste verte reçoivent `NotAllowed`, et
  les charges utiles signées par la mauvaise clé reçoivent `MismatchedPubkey`.
- **Flux de requête :** lorsque le stockage est vide et que `genesis.file` n'est pas défini (et
  `genesis.bootstrap_enabled=true`), le nœud effectue un contrôle en amont des homologues de confiance avec l'option facultative
  `genesis.expected_hash`, récupère ensuite la charge utile, valide les signatures via `validate_genesis_block`,
  et persiste `genesis.bootstrap.nrt` aux côtés de Kura avant d'appliquer le bloc. Nouvelles tentatives d'amorçage
  honorer `genesis.bootstrap_request_timeout`, `genesis.bootstrap_retry_interval` et
  `genesis.bootstrap_max_attempts`.
- **Modes d'échec :** les demandes sont rejetées en cas d'échec de la liste d'autorisation, de non-concordance de chaîne/clé de publication/hachage, de taille
  violations de plafond, limites de débit, genèse locale manquante ou identifiants de demande en double. Hachages contradictoires
  entre pairs, abandonnez la récupération ; aucun répondeur/délai d'attente ne revient à la configuration locale.
- **Étapes de l'opérateur :** assurez-vous qu'au moins un homologue de confiance est accessible avec une genèse valide, configurez
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` et les boutons de nouvelle tentative, et
  épinglez éventuellement `expected_hash` pour éviter d’accepter des charges utiles incompatibles. Les charges utiles persistantes peuvent être
  réutilisé lors des démarrages suivants en pointant `genesis.file` vers `genesis.bootstrap.nrt`.