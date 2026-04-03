<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Rapport d'audit de sécurité

Dates : 2026-03-26

## Résumé

Cet audit s'est concentré sur les surfaces les plus à risque dans l'arborescence actuelle : flux HTTP/API/auth Torii, transport P2P, API de gestion des secrets, gardes de transport SDK et chemin de nettoyage des pièces jointes.

J'ai trouvé 6 problèmes exploitables :

- 2 constatations de gravité élevée
- 4 constatations de gravité moyenne

Les problèmes les plus importants sont :

1. Torii enregistre actuellement les en-têtes de requête entrante pour chaque requête HTTP, ce qui peut exposer les jetons de support, les jetons d'API, les jetons de session/amorçage de l'opérateur et les marqueurs mTLS transférés aux journaux.
2. Plusieurs routes et SDK publics Torii prennent toujours en charge l'envoi de valeurs `private_key` brutes au serveur afin que Torii puisse signer au nom de l'appelant.
3. Plusieurs chemins « secrets » sont traités comme des corps de requête ordinaires, notamment la dérivation de graines confidentielles et l'authentification de requête canonique dans certains SDK.

## Méthode

- Examen statique des chemins de gestion des secrets Torii, P2P, crypto/VM et SDK
- Commandes de validation ciblées :
  - `cargo check -p iroha_torii --lib --message-format short` -> réussir
  - `cargo check -p iroha_p2p --message-format short` -> réussir
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> réussir
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> réussite, avertissements de version en double uniquement
- Non complété dans cette passe :
  - construction/test/clippy d'un espace de travail complet
  - Suites de tests Swift/Gradle
  - Validation du runtime CUDA/Metal

## Résultats

### SA-001 Élevé : Torii enregistre les en-têtes de requêtes sensibles à l'échelle mondialeImpact : tout déploiement qui envoie le suivi des demandes peut divulguer des jetons de porteur/API/opérateur et du matériel d'authentification associé dans les journaux d'application.

Preuve :

- `crates/iroha_torii/src/lib.rs:20752` active `TraceLayer::new_for_http()`
- `crates/iroha_torii/src/lib.rs:20753` active `DefaultMakeSpan::default().include_headers(true)`
- Les noms d'en-tête sensibles sont activement utilisés ailleurs dans le même service :
  -`crates/iroha_torii/src/operator_auth.rs:40`
  -`crates/iroha_torii/src/operator_auth.rs:41`
  -`crates/iroha_torii/src/operator_auth.rs:42`
  -`crates/iroha_torii/src/operator_auth.rs:43`

Pourquoi c'est important :

- `include_headers(true)` enregistre les valeurs complètes de l'en-tête entrant dans des étendues de traçage.
- Torii accepte les éléments d'authentification dans les en-têtes tels que `Authorization`, `x-api-token`, `x-iroha-operator-session`, `x-iroha-operator-token` et `x-forwarded-client-cert`.
- Une compromission du récepteur de journaux, une collecte de journaux de débogage ou un ensemble de support peut donc devenir un événement de divulgation d'informations d'identification.

Correction recommandée :

- Arrêtez d'inclure les en-têtes de requête complets dans les délais de production.
- Ajoutez une rédaction explicite pour les en-têtes sensibles à la sécurité si la journalisation des en-têtes est toujours nécessaire pour le débogage.
- Traitez par défaut la journalisation des requêtes/réponses comme contenant un secret, à moins que les données ne soient positivement sur liste verte.

### SA-002 Élevé : les API publiques Torii acceptent toujours les clés privées brutes pour la signature côté serveur

Impact : les clients sont encouragés à transmettre des clés privées brutes sur le réseau afin que le serveur puisse signer en leur nom, créant ainsi un canal inutile d'exposition des secrets au niveau des couches API, SDK, proxy et mémoire du serveur.

Preuve:- La documentation des routes de gouvernance annonce explicitement la signature côté serveur :
  -`crates/iroha_torii/src/gov.rs:495`
- L'implémentation de la route analyse la clé privée fournie et signe côté serveur :
  -`crates/iroha_torii/src/gov.rs:1088`
  -`crates/iroha_torii/src/gov.rs:1091`
  -`crates/iroha_torii/src/gov.rs:1123`
  -`crates/iroha_torii/src/gov.rs:1125`
- Les SDK sérialisent activement `private_key` dans des corps JSON :
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

Remarques :

- Ce modèle n'est pas isolé d'une seule famille de routes. L'arborescence actuelle contient le même modèle pratique en matière de gouvernance, de trésorerie hors ligne, d'abonnements et d'autres DTO liés aux applications.
- Les contrôles de transport HTTPS uniquement réduisent le transport accidentel de texte en clair, mais ils ne résolvent pas le risque de gestion des secrets côté serveur ni le risque d'exposition à la journalisation/à la mémoire.

Correction recommandée :

- Déprécier tous les DTO de requête qui transportent des données brutes `private_key`.
- Exiger des clients qu'ils signent localement et soumettent des signatures ou des transactions/enveloppes entièrement signées.
- Supprimez les exemples `private_key` de OpenAPI/SDK après une fenêtre de compatibilité.

### SA-003 Medium : la dérivation de clé confidentielle envoie du matériel de départ secret à Torii et le renvoie en écho

Impact : L'API confidentielle de dérivation de clé transforme le matériel de départ en données utiles de requête/réponse normales, augmentant ainsi les risques de divulgation de semences via des proxys, des middlewares, des journaux, des traces, des rapports d'erreur ou une mauvaise utilisation des clients.

Preuve:- La demande accepte directement le matériel de semence :
  -`crates/iroha_torii/src/routing.rs:2736`
  -`crates/iroha_torii/src/routing.rs:2738`
  -`crates/iroha_torii/src/routing.rs:2740`
- Le schéma de réponse fait écho à la graine en hexadécimal et en base64 :
  -`crates/iroha_torii/src/routing.rs:2745`
  -`crates/iroha_torii/src/routing.rs:2746`
  -`crates/iroha_torii/src/routing.rs:2747`
- Le gestionnaire ré-encode et renvoie explicitement la graine :
  -`crates/iroha_torii/src/routing.rs:2797`
  -`crates/iroha_torii/src/routing.rs:2801`
  -`crates/iroha_torii/src/routing.rs:2802`
  -`crates/iroha_torii/src/routing.rs:2804`
- Le SDK Swift expose cela comme une méthode réseau standard et conserve la graine renvoyée dans le modèle de réponse :
  -`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  -`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  -`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  -`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  -`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

Correction recommandée :

- Préférez la dérivation de clé locale dans le code CLI/SDK et supprimez entièrement la route de dérivation à distance.
- Si l'itinéraire doit être maintenu, ne jamais renvoyer les graines dans l'intervention et marquer les corps semenciers comme sensibles dans tous les gardes de transport et les chemins de télémétrie/exploitation forestière.

### SA-004 Medium : la détection de la sensibilité du transport du SDK présente des angles morts pour le matériel secret non `private_key`

Impact : certains SDK appliqueront HTTPS pour les requêtes brutes `private_key`, mais permettront toujours à d'autres éléments de requête sensibles à la sécurité de circuler via HTTP non sécurisé ou vers des hôtes incompatibles.

Preuve:- Swift traite les en-têtes d'authentification des requêtes canoniques comme sensibles :
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- Mais Swift ne correspond toujours qu'au corps sur `"private_key"` :
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- Kotlin ne reconnaît que les en-têtes `authorization` et `x-api-token`, puis revient à la même heuristique de corps `"private_key"` :
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- Java/Android a la même limitation :
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- Les signataires de requêtes canoniques Kotlin/Java génèrent des en-têtes d'authentification supplémentaires qui ne sont pas classés comme sensibles par leurs propres gardes de transport :
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

Correction recommandée :

- Remplacer l'analyse heuristique du corps par une classification explicite des requêtes.
- Traitez les en-têtes d'authentification canoniques, les champs de graine/phrase secrète, les en-têtes de mutation signés et tout futur champ porteur de secret comme sensibles par contrat, et non par correspondance de sous-chaîne.
- Gardez les règles de sensibilité alignées sur Swift, Kotlin et Java.

### SA-005 Medium : la pièce jointe "bac à sable" n'est qu'un sous-processus plus `setrlimit`Impact : Le nettoyeur de pièces jointes est décrit et signalé comme "en bac à sable", mais l'implémentation n'est qu'un fork/exec du binaire actuel avec des limites de ressources. Un exploit d'analyseur ou d'archive s'exécuterait toujours avec les mêmes privilèges d'utilisateur, de vue du système de fichiers et de réseau/processus ambiant que Torii.

Preuve :

- Le chemin extérieur marque le résultat comme étant mis en bac à sable après la génération d'un enfant :
  -`crates/iroha_torii/src/zk_attachments.rs:756`
  -`crates/iroha_torii/src/zk_attachments.rs:760`
  -`crates/iroha_torii/src/zk_attachments.rs:776`
  -`crates/iroha_torii/src/zk_attachments.rs:782`
- L'enfant utilise par défaut l'exécutable actuel :
  -`crates/iroha_torii/src/zk_attachments.rs:913`
  -`crates/iroha_torii/src/zk_attachments.rs:919`
- Le sous-processus repasse explicitement en `AttachmentSanitizerMode::InProcess` :
  -`crates/iroha_torii/src/zk_attachments.rs:1794`
  -`crates/iroha_torii/src/zk_attachments.rs:1803`
- Le seul renforcement appliqué est le CPU/espace d'adressage `setrlimit` :
  -`crates/iroha_torii/src/zk_attachments.rs:1845`
  -`crates/iroha_torii/src/zk_attachments.rs:1850`
  -`crates/iroha_torii/src/zk_attachments.rs:1851`
  -`crates/iroha_torii/src/zk_attachments.rs:1872`

Correction recommandée :

- Soit implémentez un véritable bac à sable du système d'exploitation (par exemple espaces de noms/seccomp/landlock/isolation de style prison, suppression de privilèges, pas de réseau, système de fichiers contraint) ou arrêtez d'étiqueter le résultat comme `sandboxed`.
- Traitez la conception actuelle comme une « isolation des sous-processus » plutôt que comme un « sandboxing » dans les API, la télémétrie et la documentation jusqu'à ce qu'une véritable isolation existe.

### SA-006 Medium : les transports P2P TLS/QUIC en option désactivent la vérification du certificatImpact : lorsque `quic` ou `p2p_tls` est activé, le canal fournit le chiffrement mais n'authentifie pas le point de terminaison distant. Un attaquant actif sur le chemin peut toujours relayer ou mettre fin au canal, allant à l'encontre des attentes de sécurité normales que les opérateurs associent à TLS/QUIC.

Preuve :

- QUIC documente explicitement la vérification permissive des certificats :
  -`crates/iroha_p2p/src/transport.rs:12`
  -`crates/iroha_p2p/src/transport.rs:13`
  -`crates/iroha_p2p/src/transport.rs:14`
  -`crates/iroha_p2p/src/transport.rs:15`
- Le vérificateur QUIC accepte sans condition le certificat du serveur :
  -`crates/iroha_p2p/src/transport.rs:33`
  -`crates/iroha_p2p/src/transport.rs:35`
  -`crates/iroha_p2p/src/transport.rs:44`
  -`crates/iroha_p2p/src/transport.rs:112`
  -`crates/iroha_p2p/src/transport.rs:114`
  -`crates/iroha_p2p/src/transport.rs:115`
- Le transport TLS-over-TCP fait de même :
  -`crates/iroha_p2p/src/transport.rs:229`
  -`crates/iroha_p2p/src/transport.rs:232`
  -`crates/iroha_p2p/src/transport.rs:241`
  -`crates/iroha_p2p/src/transport.rs:279`
  -`crates/iroha_p2p/src/transport.rs:281`
  -`crates/iroha_p2p/src/transport.rs:282`

Correction recommandée :

- Vérifiez les certificats homologues ou ajoutez une liaison de canal explicite entre la prise de contact signée de couche supérieure et la session de transport.
- Si le comportement actuel est intentionnel, renommez/documentez la fonctionnalité en tant que transport chiffré non authentifié afin que les opérateurs ne la confondent pas avec une authentification homologue TLS complète.

## Ordre de correction recommandé1. Corrigez immédiatement SA-001 en supprimant ou en désactivant la journalisation des en-têtes.
2. Concevez et expédiez un plan de migration pour SA-002 afin que les clés privées brutes cessent de traverser les limites de l'API.
3. Supprimez ou réduisez la route de dérivation de clé confidentielle à distance et classez les corps porteurs de graines comme sensibles.
4. Alignez les règles de sensibilité du transport du SDK dans Swift/Kotlin/Java.
5. Décidez si l’assainissement des pièces jointes nécessite un véritable bac à sable ou un changement de nom/redéfinition honnête.
6. Clarifier et renforcer le modèle de menace P2P TLS/QUIC avant que les opérateurs n'activent les transports qui attendent un TLS authentifié.

## Notes de validation

- `cargo check -p iroha_torii --lib --message-format short` réussi.
- `cargo check -p iroha_p2p --message-format short` réussi.
- `cargo deny check advisories bans sources --hide-inclusion-graph` réussi après une exécution en dehors du bac à sable ; il a émis des avertissements de version en double mais a signalé `advisories ok, bans ok, sources ok`.
- Un test ciblé Torii pour la route confidentielle derive-keyset a été lancé au cours de cet audit mais ne s'est pas terminé avant la rédaction du rapport ; la conclusion est néanmoins étayée par une inspection directe à la source.