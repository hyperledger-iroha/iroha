---
lang: fr
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2026-01-03T18:08:01.837162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Taxonomie des erreurs de connexion (base de référence Swift)

Cette note suit IOS-CONNECT-010 et documente la taxonomie des erreurs partagées pour
Nexus Connectez les SDK. Swift exporte désormais le wrapper canonique `ConnectError`,
qui mappe toutes les pannes liées à Connect à l'une des six catégories, donc la télémétrie,
les tableaux de bord et la copie UX restent alignés sur toutes les plateformes.

> Dernière mise à jour : 2026-01-15  
> Propriétaire : Swift SDK Lead (intendant de la taxonomie)  
> Statut : implémentations Swift + Android + JavaScript **réalisées** ; intégration de la file d'attente en attente.

## Catégories

| Catégorie | Objectif | Sources typiques |
|----------|---------|-----------------|
| `transport` | Échecs de transport WebSocket/HTTP qui mettent fin à une session. | `URLError(.cannotConnectToHost)`, `ConnectClient.ClientError.closed` |
| `codec` | Échecs de sérialisation/pont lors de l’encodage/décodage des trames. | `ConnectEnvelopeError.invalidPayload`, `DecodingError` |
| `authorization` | Échecs TLS/attestation/politique nécessitant une correction par l’utilisateur ou l’opérateur. | `URLError(.secureConnectionFailed)`, Torii Réponses 4xx |
| `timeout` | Expirations inactives/hors ligne et chiens de garde (file d'attente TTL, délai d'expiration de la demande). | `URLError(.timedOut)`, `ConnectQueueError.expired` |
| `queueOverflow` | Signaux de contre-pression FIFO afin que les applications puissent se charger gracieusement. | `ConnectQueueError.overflow(limit:)` |
| `internal` | Tout le reste : utilisation abusive du SDK, pont Norito manquant, journaux corrompus. | `ConnectSessionError.missingDecryptionKeys`, `ConnectCryptoError.*` |

Chaque SDK publie un type d'erreur conforme à la taxonomie et expose
Attributs de télémétrie structurés : `category`, `code`, `fatal` et facultatif
métadonnées (`http_status`, `underlying`).

## Cartographie rapide

Swift exporte `ConnectError`, `ConnectErrorCategory` et les protocoles d'assistance dans
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`. Erreur de connexion publique
les types sont conformes à `ConnectErrorConvertible`, les applications peuvent donc appeler `error.asConnectError()`
et transmettez le résultat aux couches de télémétrie/journalisation.| Erreur rapide | Catégorie | Codes | Remarques |
|-------------|----------|------|-------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | Indique le double `start()` ; erreur du développeur. |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | Levé lors de l'envoi/réception après une clôture. |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | WebSocket a fourni une charge utile textuelle en attendant du binaire. |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | La contrepartie a fermé le flux de manière inattendue. |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | L'application a oublié de configurer les clés symétriques. |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | Il manque des champs obligatoires dans la charge utile Norito. |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` | Charge utile future vue par l'ancien SDK. |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Le pont Norito est manquant ou n'a pas réussi à encoder/décoder les octets de trame. |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | Pont indisponible ou longueurs de clé incompatibles. |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` | La longueur de la file d'attente hors ligne a dépassé la limite configurée. |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | Apparu par `URLSessionWebSocketTask`. |
| `URLError` Cas TLS | `authorization` | `network.tls_failure` | Échecs de la négociation ATS/TLS. |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | Le décodage/encodage JSON a échoué ailleurs dans le SDK ; Le message utilise le contexte du décodeur Swift. |
| Tout autre `Error` | `internal` | `unknown_error` | Fourre-tout garanti ; le message reflète `LocalizedError`. |

Verrouillage des tests unitaires (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`)
le mappage afin que les futurs refactors ne puissent pas changer silencieusement les catégories ou les codes.

### Exemple d'utilisation

```swift
do {
    try await session.nextEnvelope()
} catch {
    let connectError = error.asConnectError()
    telemetry.emit(event: "connect.error",
                   attributes: connectError.telemetryAttributes(fatal: true))
    logger.error("Connect failure: \(connectError.code) – \(connectError.message)")
}
```

## Télémétrie et tableaux de bord

Le SDK Swift fournit `ConnectError.telemetryAttributes(fatal:httpStatus:)`
qui renvoie la carte d'attributs canoniques. Les exportateurs doivent les transmettre
attributs dans les événements `connect.error` OTEL avec des extras facultatifs :

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

Les tableaux de bord mettent en corrélation les compteurs `connect.error` avec la profondeur de la file d'attente (`connect.queue_depth`)
et reconnectez les histogrammes pour détecter les régressions sans journaux de spéléologie.

## Cartographie AndroidLe SDK Android exporte `ConnectError`, `ConnectErrorCategory`, `ConnectErrorTelemetryOptions`,
`ConnectErrorOptions` et les utilitaires d'assistance sous
`org.hyperledger.iroha.android.connect.error`. Les assistants de style constructeur convertissent n'importe quel `Throwable`
dans une charge utile conforme à la taxonomie, déduire des catégories à partir d'exceptions de transport/TLS/codec,
et exposez les attributs de télémétrie déterministes afin que les piles OpenTelemetry/sampling puissent consommer le
résultat sans adaptateurs sur mesure.
`ConnectQueueError` implémente déjà `ConnectErrorConvertible`, émettant le queueOverflow/timeout
catégories pour les conditions de débordement/expiration afin que l'instrumentation de file d'attente hors ligne puisse se connecter au même flux.
Le README du SDK Android fait désormais référence à la taxonomie et montre comment envelopper les exceptions de transport.
avant d'émettre la télémétrie, en gardant le guidage dApp aligné sur la ligne de base Swift.【java/iroha_android/README.md:167】

## Cartographie JavaScript

Les clients Node.js/browser importent `ConnectError`, `ConnectQueueError`, `ConnectErrorCategory` et
`connectErrorFrom()` de `@iroha/iroha-js`. L'assistant partagé inspecte les codes d'état HTTP,
Codes d'erreur de nœud (socket, TLS, délai d'attente), noms `DOMException` et échecs du codec pour émettre le
mêmes catégories/codes documentés dans cette note, tandis que les définitions TypeScript modélisent la télémétrie
l'attribut remplace afin que les outils puissent émettre des événements OTEL sans conversion manuelle.
Le SDK README documente le flux de travail et renvoie à cette taxonomie afin que les équipes d'application puissent
copiez les extraits d'instrumentation textuellement.【javascript/iroha_js/README.md:1387】

## Étapes suivantes (cross-SDK)

- **Intégration de la file d'attente :** une fois la file d'attente hors ligne expédiée, assurez-vous de la logique de retrait/abandon de la file d'attente
  fait apparaître les valeurs `ConnectQueueError` afin que la télémétrie de débordement reste digne de confiance.