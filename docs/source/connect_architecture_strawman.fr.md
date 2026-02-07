---
lang: fr
direction: ltr
source: docs/source/connect_architecture_strawman.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1a6bcc6bca3d7f70b82e35734b71d706ac46d8dc9c728351fabbd8a61dd3f31
source_last_modified: "2026-01-04T10:50:53.610255+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Connecter l'architecture de session Strawman (Swift / Android / JS)

Cette proposition de paille décrit la conception partagée des flux de travail Nexus Connect
à travers les SDK Swift, Android et JavaScript. Il est destiné à soutenir le
Atelier cross-SDK de février 2026 et capture des questions ouvertes avant la mise en œuvre.

> Dernière mise à jour : 2026-01-29  
> Auteurs : responsable du SDK Swift, Android Networking TL, responsable JS  
> Statut : projet pour examen par le conseil (modèle de menace + alignement sur la conservation des données ajouté le 12/03/2026)

## Objectifs

1. Alignez le cycle de vie du portefeuille ↔ de la session dApp, y compris l'amorçage de la connexion,
   approbations, demandes de signature et démontage.
2. Définir le schéma de l'enveloppe Norito (ouvrir/approuver/signer/contrôler) partagé par tous
   SDK et assurer la parité avec `connect_norito_bridge`.
3. Répartir les responsabilités entre le transport (WebSocket/WebRTC), le chiffrement
   (cadres Norito Connect + échange de clés), et couches applicatives (façades SDK).
4. Garantir un comportement déterministe sur les plates-formes de bureau/mobiles, y compris
   mise en mémoire tampon et reconnexion hors ligne.

## Cycle de vie de la session (haut niveau)

```
┌────────────┐      ┌─────────────┐      ┌────────────┐
│  dApp SDK  │←────→│  Connect WS │←────→│ Wallet SDK │
└────────────┘      └─────────────┘      └────────────┘
      │                    │                    │
      │ 1. open (app→wallet) frame (metadata, permissions, chain_id)
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 2. route frame     │
      │                    │────────────────────│
      │                    │                    │
      │                    │     3. approve frame (wallet pk, account,
      │                    │        permissions, proof/attest)
      │<────────────────────────────────────────│
      │                    │                    │
      │ 4. sign request    │                    │
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 5. sign result     │
      │                    │────────────────────│
      │                    │                    │
      │ 6. control frames for reject/close, error propagation, heartbeats.
```

## Enveloppe/Schéma Norito

Tous les SDK DOIVENT utiliser le schéma canonique Norito défini dans `connect_norito_bridge` :

- `EnvelopeV1` (ouvrir/approuver/signer/contrôler)
- `ConnectFrameV1` (trames de texte chiffré avec charge utile AEAD)
- Codes de contrôle :
  - `open_ext` (métadonnées, autorisations)
  - `approve_ext` (compte, autorisations, preuves, signature)
  -`reject`, `close`, `ping/pong`, `error`

Swift a déjà livré des encodeurs JSON d'espace réservé (`ConnectCodec.swift`). Depuis avril 2026, le SDK
utilise toujours le pont Norito et échoue lorsque le XCFramework est manquant, mais cet homme de paille
reflète toujours le mandat qui a conduit à l’intégration du pont :

| Fonction | Descriptif | Statut |
|--------------|-------------|--------|
| `connect_norito_encode_control_open_ext` | cadre ouvert dApp | Implémenté en pont |
| `connect_norito_encode_control_approve_ext` | Approbation du portefeuille | Mis en œuvre |
| `connect_norito_encode_envelope_sign_request_tx/raw` | Signer les demandes | Mis en œuvre |
| `connect_norito_encode_envelope_sign_result_ok/err` | Signer les résultats | Mis en œuvre |
| `connect_norito_decode_*` | Analyse des portefeuilles/dApps | Mis en œuvre |

### Travaux requis

- Swift : remplacez les assistants JSON `ConnectCodec` par des appels de pont et une surface.
  wrappers typés (`ConnectFrame`, `ConnectEnvelope`) utilisant les types Norito partagés. ✅ (avril 2026)
- Android/JS : assurez-vous que les mêmes wrappers existent ; aligner les codes d’erreur et les clés de métadonnées.
- Partagé : cryptage de documents (échange de clés X25519, AEAD) avec dérivation de clé cohérente
  selon la spécification Norito et fournissez des exemples de tests d'intégration à l'aide du pont Rust.

## Contrat de transport- Transport primaire : WebSocket (`/v1/connect/ws?sid=<session_id>`).
- Futur facultatif : WebRTC (à déterminer) – hors de portée pour l'homme de paille initial.
- Stratégie de reconnexion : recul exponentiel avec gigue totale (base 5 s, max 60 s) ; constantes partagées sur Swift, Android et JS afin que les tentatives restent prévisibles.
- Cadence ping/pong : battement de coeur de 30 s avec tolérance de trois pongs manqués avant reconnexion ; JS limite l'intervalle minimum à 15 secondes pour satisfaire aux règles de limitation du navigateur.
- Push hooks : le SDK du portefeuille Android expose une intégration FCM facultative pour les réveils, tandis que JS reste basé sur des sondages (limitations documentées pour les autorisations push du navigateur).
- Responsabilités du SDK :
  - Maintenez les battements de cœur de ping/pong (évitez de vider les batteries du mobile).
  - Mettre en mémoire tampon les trames sortantes hors ligne (file d'attente limitée, conservée pour dApp).
- Fournir une API de flux d'événements (Swift Combine `AsyncStream`, Android Flow, JS async iter).
- Surface reconnectez les crochets et autorisez le réabonnement manuel.
- Rédaction de télémétrie : émettre uniquement des compteurs au niveau session (hachage `sid`, direction,
  fenêtre de séquence, profondeur de file d'attente) avec des sels documentés dans la télémétrie Connect
  guider; les en-têtes/clés ne doivent jamais apparaître dans les journaux ou les chaînes de débogage.

## Chiffrement et gestion des clés

### Identifiants de session et sels

- `sid` est un identifiant de 32 octets dérivé de `BLAKE2b-256("iroha-connect|sid|" || chain_id || app_ephemeral_pk || nonce16)`.  
  Les DApps le calculent avant d'appeler `/v1/connect/session` ; les portefeuilles le font écho dans les trames `approve` afin que les deux parties puissent saisir les journaux et la télémétrie de manière cohérente.
- Le même sel alimente chaque étape de dérivation de clé afin que les SDK ne dépendent jamais de l'entropie récoltée à partir de la plate-forme hôte.

### Gestion des clés éphémères

- Chaque session utilise du nouveau matériel clé X25519.  
  Swift le stocke dans le trousseau/Secure Enclave via `ConnectCrypto`, les portefeuilles Android sont par défaut StrongBox (en revenant aux magasins de clés sauvegardés par TEE) et JS nécessite une instance WebCrypto à contexte sécurisé ou le plug-in natif `iroha_js_host`.
- Les cadres ouverts incluent la clé publique éphémère dApp ainsi qu'un ensemble d'attestation en option. Les approbations du portefeuille renvoient la clé publique du portefeuille et toute attestation matérielle nécessaire aux flux de conformité.
- Les charges utiles d'attestation suivent le schéma accepté :  
  `attestation { platform, evidence_b64, statement_hash }`.  
  Les navigateurs peuvent omettre le blocage ; les portefeuilles natifs l’incluent chaque fois que des clés matérielles sont utilisées.

### Touches directionnelles et AEAD

- Les secrets partagés sont étendus avec HKDF-SHA256 (via les assistants de pont Rust) et des chaînes d'informations séparées par domaine :
  - `iroha-connect|k_app` → trafic application → portefeuille.
  - `iroha-connect|k_wallet` → portefeuille → trafic d'application.
- AEAD est ChaCha20-Poly1305 pour l'enveloppe v1 (`connect_norito_bridge` expose les helpers sur chaque plateforme).  
  Les données associées sont égales à `("connect:v1", sid, dir, seq_le, kind=ciphertext)`, donc la falsification des en-têtes est détectée.
- Les noms occasionnels sont dérivés du compteur de séquence 64 bits (`nonce[0..4]=0`, `nonce[4..12]=seq_le`). Les tests d'assistance partagés garantissent que les conversions BigInt/UInt se comportent de manière identique dans tous les SDK.

### Poignée de main de rotation et de récupération- La rotation reste facultative mais le protocole est défini : les dApps émettent une trame `Control::RotateKeys` lorsque les compteurs de séquence s'approchent du wrap guard, les portefeuilles répondent avec la nouvelle clé publique plus un accusé de réception signé, et les deux parties dérivent immédiatement de nouvelles clés directionnelles sans fermer la session.
- La perte de clé côté portefeuille déclenche la même poignée de main suivie d'un contrôle `resume` afin que les dApp sachent vider le texte chiffré mis en cache qui ciblait la clé retirée.

Pour les solutions de repli historiques de CryptoKit, voir `docs/connect_swift_ios.md` ; Kotlin et JS ont des références correspondantes sous `docs/connect_kotlin_ws*.md`.

## Autorisations et preuves

- Les manifestes d'autorisation doivent faire un aller-retour via la structure Norito partagée exportée par le pont.  
  Champs :
  - `methods` — verbes (`sign_transaction`, `sign_raw`, `submit_proof`, …).  
  - `events` — abonnements auxquels la dApp est autorisée à s'attacher.  
  - `resources` — filtres de compte/actifs en option afin que les portefeuilles puissent accéder à l'accès.  
  - `constraints` — ID de chaîne, TTL ou boutons de stratégie personnalisés que le portefeuille applique avant de signer.
- Les métadonnées de conformité accompagnent les autorisations :
  - Le `attachments[]` en option contient les références de pièces jointes Norito (bundles KYC, reçus du régulateur).  
  - `compliance_manifest_id` lie la demande à un manifeste préalablement approuvé afin que les opérateurs puissent vérifier la provenance.
- Les réponses du portefeuille utilisent les codes convenus :
  -`user_declined`, `permissions_mismatch`, `compliance_failed`, `internal_error`.  
  Chacun peut porter un `localized_message` pour les astuces d'interface utilisateur ainsi qu'un `reason_code` lisible par machine.
- Les cadres d'approbation incluent le compte/contrôleur sélectionné, l'écho d'autorisation, l'ensemble de preuves (preuve ZK ou attestation) et toutes les bascules de politique (par exemple, `offline_queue_enabled`).  
  Les rejets reflètent le même schéma avec un `proof` vide, mais enregistrent toujours le `sid` à des fins d'audit.

## Façades du SDK

| SDK | API proposée | Remarques |
|-----|--------------|-------|
| Rapide | `ConnectClient`, `ConnectSession`, `ConnectRequest`, `ConnectApproval` | Remplacez les espaces réservés par des wrappers tapés + des flux asynchrones. |
| Android | Coroutines Kotlin + classes scellées pour les frames | Alignez-vous sur la structure Swift pour la portabilité. |
| JS | Itérateurs asynchrones + énumérations TypeScript pour les types de cadres | Fournissez un SDK convivial pour les bundles (navigateur/nœud). |

### Comportements courants- `ConnectSession` orchestre le cycle de vie :
  1. Établissez WebSocket, effectuez une poignée de main.
  2. Échangez des cadres ouverts/approuvés.
  3. Gérer les demandes/réponses de signature.
  4. Émettez des événements à la couche application.
- Fournir des aides de haut niveau :
  -`requestSignature(tx, metadata)`
  -`approveSession(account, permissions)`
  -`reject(reason)`
  - `cancelRequest(hash)` – émet une trame de contrôle reconnue par le portefeuille.
- Gestion des erreurs : mappez les codes d'erreur Norito aux erreurs spécifiques au SDK ; inclure
  codes spécifiques au domaine pour l'interface utilisateur utilisant la taxonomie partagée (`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, `Internal`). Le guide d'implémentation de base + de télémétrie de Swift se trouve dans [`connect_error_taxonomy.md`](connect_error_taxonomy.md) et est la référence pour la parité Android/JS.
- Émettez des hooks de télémétrie pour la profondeur de la file d'attente, le nombre de reconnexions et la latence des demandes (`connect.queue_depth`, `connect.reconnects_total`, `connect.latency_ms`).
 
## Numéros de séquence et contrôle de flux

- Chaque direction conserve un compteur `sequence` 64 bits dédié qui démarre à zéro à l'ouverture de la session. Les types d'assistant partagé bloquent les incréments et déclenchent une poignée de main `ConnectError.sequenceOverflow` + rotation de clé bien avant que le compteur ne se termine.
- Les noms occasionnels et les données associées font référence au numéro de séquence, de sorte que les doublons peuvent être rejetés sans analyser les charges utiles. Les SDK doivent stocker `{sid, dir, seq, payload_hash}` dans leurs journaux pour rendre la déduplication déterministe lors des reconnexions.
- Les portefeuilles annoncent la contre-pression via une fenêtre logique (trames de contrôle `FlowControl`). Les DApps ne sont retirés de la file d'attente que lorsqu'un jeton de fenêtre est disponible ; les portefeuilles émettent de nouveaux jetons après avoir traité le texte chiffré pour maintenir les pipelines délimités.
- La reprise de la négociation est explicite : les deux parties émettent `Control::Resume { seq_app_max, seq_wallet_max, queue_depths }` après la reconnexion afin que les observateurs puissent vérifier la quantité de données renvoyées et si les journaux contiennent des lacunes.
- Les conflits (par exemple, deux charges utiles avec le même `(sid, dir, seq)` mais des hachages différents) dégénèrent en `ConnectError.Internal` et forcent un nouveau `sid` pour éviter une divergence silencieuse.

## Modèle de menace et alignement sur la conservation des données- **Surfaces considérées :** transport WebSocket, encodage/décodage du pont Norito,
  persistance du journal, exportateurs de télémétrie et rappels orientés application.
- **Objectifs principaux :** protéger les secrets de session (clés X25519, clés AEAD dérivées,
  compteurs de nonce/séquence) contre les fuites dans les journaux/télémétrie, empêcher la relecture et
  les attaques de déclassement et la conservation limitée des journaux et des rapports d’anomalies.
- **Atténuations codifiées :**
  - Les journaux contiennent uniquement du texte chiffré ; les métadonnées stockées sont limitées aux hachages, à la longueur
    champs, horodatages et numéros de séquence.
  - Les charges utiles de télémétrie suppriment tout contenu d'en-tête/charge utile et incluent uniquement
    hachages salés de `sid` plus compteurs globaux ; liste de contrôle de rédaction partagée
    entre les SDK pour la parité d’audit.
  - Les journaux de session sont alternés et expirent après 7 jours par défaut. Les portefeuilles exposent
    un bouton `connectLogRetentionDays` (SDK par défaut 7) et documenter le comportement
    les déploiements réglementés peuvent donc épingler des fenêtres plus strictes.
  - Utilisation abusive de l'API Bridge (liaisons manquantes, texte chiffré corrompu, séquence invalide)
    renvoie les erreurs typées sans faire écho aux charges utiles ou aux clés brutes.

Les questions en attente de l'examen sont suivies dans `docs/source/sdk/swift/connect_workshop.md`.
et sera résolu dans le procès-verbal du conseil ; une fois fermé, l'homme de paille sera
promu de projet à accepté.

## Mise en mémoire tampon et reconnexions hors ligne

### Contrat de journalisation

Chaque SDK gère un journal d'ajouts uniquement par session afin que le dApp et le portefeuille
peut mettre les trames en file d'attente hors ligne, reprendre sans perte de données et fournir des preuves
pour la télémétrie. Le contrat reflète les types de ponts Norito donc le même octet
la représentation survit dans les piles mobiles/JS.- Les journaux vivent sous un identifiant de session haché (`sha256(sid)`), produisant deux
  fichiers par session : `app_to_wallet.queue` et `wallet_to_app.queue`. Utilisations rapides
  un wrapper de fichiers en bac à sable, Android stocke les fichiers via `Room`/`FileChannel`,
  et JS écrit dans IndexedDB ; tous les formats sont binaires et endian-stables.
- Chaque enregistrement est sérialisé comme `ConnectJournalRecordV1` :
  -`direction: u8` (`0 = app→wallet`, `1 = wallet→app`)
  -`sequence: u64`
  - `payload_hash: [u8; 32]` (Blake3 du texte chiffré + en-têtes)
  -`ciphertext_len: u32`
  -`received_at_ms: u64`
  -`expires_at_ms: u64`
  - `ciphertext: [u8; ciphertext_len]` (cadre Norito exact déjà enveloppé dans AEAD)
- Les journaux stockent le texte chiffré textuellement. Nous ne chiffrons jamais à nouveau la charge utile ; AEAD
  les en-têtes authentifient déjà les touches de direction, donc la persistance se réduit à
  fsyncing l’enregistrement annexé.
- Une structure `ConnectQueueState` en mémoire reflète les métadonnées du fichier (profondeur,
  octets utilisés, séquence la plus ancienne/la plus récente). Il alimente les exportateurs de télémétrie et les
  Assistant `FlowControl`.
- Limite des journaux à 32 images/1 Mo par défaut ; frapper le capuchon expulse le
  entrées les plus anciennes (`reason=overflow`). `ConnectFeatureConfig.max_queue_len`
  remplace ces valeurs par défaut par déploiement.
- Les journaux conservent les données pendant 24h (`expires_at_ms`). Le GC en arrière-plan supprime les fichiers obsolètes
  segmente avec impatience afin que l'empreinte sur le disque reste limitée.
- Sécurité en cas de crash : ajouter, fsync et mettre à jour le miroir de mémoire _avant_ de notifier
  l'appelant. Au démarrage, les SDK analysent le répertoire, valident les sommes de contrôle des enregistrements,
  et reconstruisez `ConnectQueueState`. La corruption fait que le dossier incriminé est
  ignoré, signalé via la télémétrie et éventuellement mis en quarantaine pour les vidages de support.
- Parce que le texte chiffré satisfait déjà à l'enveloppe de confidentialité Norito, la seule
  Les métadonnées supplémentaires enregistrées sont l'identifiant de session haché. Des applications qui en veulent plus
  la confidentialité peut opter pour `telemetry_opt_in = false`, qui stocke les journaux mais
  supprime les exportations de profondeur de file d’attente et désactive le partage du hachage `sid` dans les journaux.
- Les SDK exposent `ConnectQueueObserver` afin que les portefeuilles/dApps puissent inspecter la profondeur de la file d'attente,
  les drains et les résultats du GC ; ce hook alimente les interfaces utilisateur d’état sans analyser les journaux.

### Sémantique de relecture et de reprise

1. Lors de la reconnexion, les SDK émettent `Control::Resume` avec `{seq_app_max,
   seq_wallet_max, queued_app, queued_wallet, journal_hash}`. Le hachage est le
   Résumé Blake3 du journal en annexe uniquement afin que les pairs qui ne correspondent pas puissent détecter la dérive.
2. L'homologue récepteur compare la charge utile de reprise avec son état et demande
   retransmission en cas d'écarts et reconnaît les images relues via
   `Control::ResumeAck`.
3. Les trames rejouées respectent toujours l'ordre d'insertion (`sequence` puis temps d'écriture).
   Les SDK de portefeuille DOIVENT appliquer une contre-pression en émettant des jetons `FlowControl` (également
   journalisé) afin que les dApps ne puissent pas inonder la file d'attente lorsqu'ils sont hors ligne.
4. Les journaux stockent le texte chiffré textuellement, donc la relecture pompe simplement les octets enregistrés
   retour via le transport et le décodeur. Aucun réencodage par SDK n’est autorisé.

### Flux de reconnexion1. Transport rétablit WebSocket et négocie un nouvel intervalle de ping.
2. dApp relit les images en file d'attente dans l'ordre, en respectant la contre-pression du portefeuille
   (`ConnectSession.nextControlFrame()` génère des jetons `FlowControl`).
3. Wallet déchiffre les résultats mis en mémoire tampon, vérifie la monotonie de la séquence et
   rejoue les approbations/résultats en attente.
4. Les deux côtés émettent un contrôle `resume` résumant `seq_app_max`, `seq_wallet_max`,
   et les profondeurs de file d'attente pour la télémétrie.
5. Les trames en double (correspondant à `sequence` + `payload_hash`) sont reconnues et supprimées ; les conflits génèrent `ConnectError.Internal` et déclenchent un redémarrage forcé de la session.

### Modes de défaillance

- Si la session est considérée comme obsolète (`offline_timeout_ms`, 5 minutes par défaut),
  les trames mises en mémoire tampon sont purgées et le SDK génère `ConnectError.sessionExpired`.
- En cas de corruption du journal, les SDK tentent une seule réparation par décodage Norito ; sur
  échec, ils abandonnent le journal et émettent la télémétrie `connect.queue_repair_failed`.
- L'inadéquation de séquence déclenche `ConnectError.replayDetected` et force un nouveau
  poignée de main (redémarrage de session avec le nouveau `sid`).

### Plan de mise en mémoire tampon hors ligne et contrôles de l'opérateur

Le livrable de l'atelier nécessite un plan documenté afin que chaque SDK soit livré de la même manière.
comportement hors ligne, flux de remédiation et surfaces de preuves. Le plan ci-dessous est
commun à Swift (`ConnectSessionDiagnostics`), Android
(`ConnectDiagnosticsSnapshot`) et JS (`ConnectQueueInspector`).

| État | Déclencheur | Réponse automatique | Commande manuelle | Drapeau de télémétrie |
|-------|---------|----------|-----------------|----------------|
| `Healthy` | Utilisation de la file d'attente  5/min | Suspendre les nouvelles demandes de signature, émettre des jetons de contrôle de flux à moitié débit | Les applications peuvent appeler `clearOfflineQueue(.app|.wallet)` ; Le SDK réhydrate l'état du homologue une fois en ligne | Jauge `connect.queue_state=\"throttled\"`, `connect.queue_watermark` |
| `Quarantined` | Utilisation ≥ `disk_watermark_drop` (par défaut 85 %), corruption détectée deux fois ou `offline_timeout_ms` dépassé | Arrêtez la mise en mémoire tampon, augmentez `ConnectError.QueueQuarantined`, exigez l'accusé de réception de l'opérateur | `ConnectSessionDiagnostics.forceReset()` supprime les journaux après l'exportation du lot | Compteur `connect.queue_state=\"quarantined\"`, `connect.queue_quarantine_total` |- Les seuils vivent dans `ConnectFeatureConfig` (`disk_watermark_warn`,
  `disk_watermark_drop`, `max_disk_bytes`, `offline_timeout_ms`). Lorsqu'un hôte
  omet une valeur, les SDK reviennent à leurs valeurs par défaut et enregistrent un avertissement afin que les configurations
  peut être audité à partir de la télémétrie.
- Les SDK exposent `ConnectQueueObserver` ainsi que des assistants de diagnostic :
  - Swift : `ConnectSessionDiagnostics.snapshot()` donne `{état, profondeur, octets,
    Reason}` and `exportJournalBundle(url:)` conserve les deux files d'attente pour la prise en charge.
  -Android : `ConnectDiagnostics.snapshot()` + `exportJournalBundle(path)`.
  - JS : `ConnectQueueInspector.read()` renvoie la même structure et un handle de blob
    ce code d'interface utilisateur peut être téléchargé vers les outils de support Torii.
- Lorsqu'une application bascule `offline_queue_enabled=false`, les SDK se vident immédiatement et
  purger les deux journaux, marquer l'état comme `Disabled` et émettre un terminal
  événement de télémétrie. La préférence orientée utilisateur est reflétée dans le Norito
  trame d’approbation afin que les pairs sachent s’ils peuvent reprendre les trames mises en mémoire tampon.
- Les opérateurs exécutent `connect queue inspect --sid <sid>` (wrapper CLI autour du SDK
  diagnostics) lors des tests de chaos ; cette commande imprime les transitions d'état,
  filigraner l'historique et reprendre les preuves afin que les examens de gouvernance ne dépendent pas de
  outils spécifiques à la plate-forme.

### Workflow du groupe de preuves

Les équipes de support et de conformité s'appuient sur des preuves déterministes lors de l'audit
comportement hors ligne. Chaque SDK implémente donc le même export en trois étapes :

1. `exportJournalBundle(..)` écrit `{app_to_wallet,wallet_to_app}.queue` plus un
   manifeste décrivant le hachage de build, les indicateurs de fonctionnalités et les filigranes de disque.
2. `exportQueueMetrics(..)` émet les 1000 derniers échantillons de télémétrie donc les tableaux de bord
   peut être reconstruit hors ligne. Les exemples incluent l'identifiant de session haché lorsque le
   l'utilisateur s'est inscrit.
3. L'assistant CLI compresse les exportations et joint un fichier de métadonnées Norito signé.
   (`ConnectQueueEvidenceV1`) afin que l'ingestion Torii puisse archiver le bundle dans SoraFS.

Les offres groupées dont la validation échoue sont rejetées avec `connect.evidence_invalid`
télémétrie afin que l'équipe SDK puisse reproduire et corriger l'exportateur.

## Télémétrie et diagnostics- Émettre des événements JSON Norito via des exportateurs OpenTelemetry partagés. Métriques obligatoires :
  - `connect.queue_depth{direction}` (jauge) alimenté par `ConnectQueueState`.
  - `connect.queue_bytes{direction}` (jauge) pour l'empreinte sauvegardée sur disque.
  - `connect.queue_dropped_total{reason}` (compteur) pour `overflow|ttl|repair`.
  - `connect.offline_flush_total{direction}` (compteur) s'incrémente lors des files d'attente
    vidange sans transport; les échecs incrémentent `connect.offline_flush_failed`.
  -`connect.replay_success_total`/`connect.replay_error_total`.
  - Histogramme `connect.resume_latency_ms` (temps entre reconnexion et stabilisation
    état) plus `connect.resume_attempts_total`.
  - Histogramme `connect.session_duration_ms` (par session terminée).
  - Événements structurés `connect.error` avec `code`, `fatal`, `telemetry_profile`.
- Les exportateurs DOIVENT apposer les étiquettes `{platform, sdk_version, feature_hash}` afin
  les tableaux de bord peuvent être divisés par version du SDK. Le `sid` haché est facultatif et uniquement
  émis lorsque l’opt-in de télémétrie est vrai.
- Les hooks au niveau du SDK font apparaître les mêmes événements afin que les applications puissent exporter plus de détails :
  -Swift : `ConnectSession.addObserver(_:) -> ConnectEvent`.
  -Androïde : `Flow<ConnectEvent>`.
  - JS : itérateur asynchrone ou rappel.
- CI gating : les tâches Swift exécutent `make swift-ci`, Android utilise `./gradlew sdkConnectCi`,
  et JS exécute `npm run test:connect` donc la télémétrie/les tableaux de bord restent verts avant
  fusionner les modifications de Connect.
- Les journaux structurés incluent les fichiers hachés `sid`, `seq`, `queue_depth` et `sid_epoch`.
  valeurs afin que les opérateurs puissent corréler les problèmes des clients. Les journaux dont la réparation échoue émettent
  Événements `connect.queue_repair_failed{reason}` plus un chemin de vidage sur incident facultatif.

### Hooks de télémétrie et preuves de gouvernance

- `connect.queue_state` sert également d'indicateur de risque de la feuille de route. Groupe Tableaux de bord
  par `{platform, sdk_version}` et restituer le temps dans l'état afin que la gouvernance puisse échantillonner
  les preuves mensuelles des exercices avant d’approuver les déploiements par étapes.
- `connect.queue_watermark` et `connect.queue_bytes` alimentent le score de risque Connect
  (`risk.connect.offline_buffer`), qui recherche automatiquement SRE lorsque plus de
  5 % des sessions passent plus de 10 minutes en `Throttled`.
- Les exportateurs attachent le `feature_hash` à chaque événement afin que les outils d'audit puissent le confirmer.
  que le codec Norito + le plan hors ligne correspondent à la version examinée. Échec du SDK CI
  rapide lorsque la télémétrie signale un hachage inconnu.
- L'homme de paille nécessite toujours une annexe de modèle de menace ; lorsque les mesures dépassent le
  seuils de politique, les SDK émettent des événements `connect.policy_violation` résumant les
  côté incriminé (haché), état et action résolue (`drain|purge|quarantine`).
- Les preuves capturées via `exportQueueMetrics` atterrissent dans le même espace de noms SoraFS
  comme les artefacts du runbook Connect afin que les évaluateurs du conseil puissent retracer chaque exercice
  revenir à des échantillons de télémétrie spécifiques sans demander de journaux internes.

## Propriété et responsabilités du cadre| Cadre / Contrôle | Propriétaire | Domaine de séquence | Le journal a persisté ? | Étiquettes de télémétrie | Remarques |
|-----------------|-------|-----------------|--------------------|------------------|-------|
| `Control::Open` | dApp | `seq_app` | ✅ (`app_to_wallet`) | `event=open` | Transporte des métadonnées + un bitmap d'autorisation ; les portefeuilles rejouent la dernière ouverture avant les invites. |
| `Control::Approve` | Portefeuille | `seq_wallet` | ✅ (`wallet_to_app`) | `event=approve` | Comprend le compte, les preuves, les signatures. Incréments de version des métadonnées enregistrés ici. |
| `Control::Reject` | Portefeuille | `seq_wallet` | ✅ | `event=reject`, `reason` | Message localisé facultatif ; dApp abandonne les demandes de signature en attente. |
| `Control::Close` (initialisation) | dApp | `seq_app` | ✅ | `event=close`, `initiator=app` | Wallet reconnaît avec son propre `Close`. |
| `Control::Close` (accusé de réception) | Portefeuille | `seq_wallet` | ✅ | `event=close`, `initiator=wallet` | Confirme le démontage ; GC supprime les journaux une fois que les deux côtés persistent dans le cadre. |
| `SignRequest` | dApp | `seq_app` | ✅ | `event=sign_request`, `payload_hash` | Hachage de charge utile enregistré pour la détection des conflits de relecture. |
| `SignResult` | Portefeuille | `seq_wallet` | ✅ | `event=sign_result`, `status=ok|err` | Comprend le hachage BLAKE3 d'octets signés ; les échecs soulèvent `ConnectError.Signing`. |
| `Control::Error` | Portefeuille (la plupart) / dApp (transport) | domaine du propriétaire correspondant | ✅ | `event=error`, `code` | Des erreurs fatales forcent le redémarrage de la session ; marques de télémétrie `fatal=true`. |
| `Control::RotateKeys` | Portefeuille | `seq_wallet` | ✅ | `event=rotate_keys`, `reason` | Annonce de nouvelles touches de direction ; dApp répond avec `RotateKeysAck` (journalisé côté application). |
| `Control::Resume` / `ResumeAck` | Les deux | domaine local uniquement | ✅ | `event=resume`, `direction=app|wallet` | Résume la profondeur de la file d'attente + l'état de la séquence ; Le résumé du journal haché facilite le diagnostic. |

- Les clés de chiffrement directionnel restent symétriques par rôle (`app→wallet`, `wallet→app`).
  Les propositions de rotation de portefeuille sont annoncées via `Control::RotateKeys` et dApps
  accuser réception en émettant `Control::RotateKeysAck` ; les deux images doivent atteindre le disque
  avant l'échange des clés pour éviter les interruptions de relecture.
- Les pièces jointes des métadonnées (icônes, noms localisés, preuves de conformité) sont signées par
  le portefeuille et mis en cache par la dApp ; les mises à jour nécessitent un nouveau cadre d'approbation avec
  incrémenté `metadata_version`.
- La matrice de propriété ci-dessus est référencée à partir de la documentation du SDK, donc CLI/web/automation
  les clients suivent les mêmes défauts de contrat et d’instrumentation.

## Questions ouvertes

1. **Découverte de session** : Avons-nous besoin de codes QR/d'une poignée de main hors bande comme WalletConnect ? (Travaux futurs.)
2. **Multisig** : Comment les approbations multi-signes sont-elles représentées ? (Étendez le résultat du signe pour prendre en charge plusieurs signatures.)
3. **Conformité** : Quels champs sont obligatoires pour les flux régulés (par feuille de route) ? (Attendez les conseils de l’équipe de conformité.)
4. **Emballage SDK** : Devrions-nous prendre en compte le code partagé (par exemple, les codecs Norito Connect) dans une caisse multiplateforme ? (à déterminer.)

## Prochaines étapes- Faites circuler cet homme de paille au conseil du SDK (réunion de février 2026).
- Recueillir des commentaires sur les questions ouvertes et mettre à jour le document en conséquence.
- Planifier la répartition de la mise en œuvre par SDK (jalons Swift IOS7, Android AND7, JS Connect).
- Suivre les progrès via la liste chaude de la feuille de route ; mettre à jour `status.md` une fois que Strawman sera ratifié.