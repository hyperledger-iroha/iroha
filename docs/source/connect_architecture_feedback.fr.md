---
lang: fr
direction: ltr
source: docs/source/connect_architecture_feedback.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 097ea58d49f48d059cda762cd719bc62f0b2d6f6ddecedef3f9bac030ae46aec
source_last_modified: "2026-01-03T18:07:58.674563+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! Liste de contrôle des commentaires sur l'architecture Connect

Cette liste de contrôle capture les questions ouvertes de l'architecture de session Connect.
homme de paille qui nécessite une entrée des leads Android et JavaScript avant le
Atelier cross-SDK de février 2026. Utilisez-le pour collecter des commentaires de manière asynchrone, suivre
l’appropriation et débloquer l’agenda de l’atelier.

> La colonne Statut/Notes a capturé les réponses finales des prospects Android et JS à partir du
> la synchronisation pré-atelier de février 2026 ; lier les nouveaux problèmes de suivi en ligne si les décisions sont prises
> évoluer.

## Cycle de vie et transport des sessions

| Sujet | Propriétaire d'Android | Propriétaire JS | Statut / Remarques |
|-------|--------------|---------------|----------------|
| Stratégie d'attente de reconnexion WebSocket (exponentielle ou linéaire plafonnée) | Réseau Android TL | Responsable JS | ✅ Convenu sur un recul exponentiel avec gigue, plafonné à 60 s ; JS reflète les mêmes constantes pour la parité navigateur/nœud. |
| Valeurs par défaut de la capacité du tampon hors ligne (homme de paille actuel : 32 images) | Réseau Android TL | Responsable JS | ✅ Confirmation par défaut de 32 images avec remplacement de la configuration ; Android persiste via `ConnectQueueConfig`, JS respecte `window.connectQueueMax`. |
| Notifications de reconnexion de type push (FCM/APNS vs sondage) | Réseau Android TL | Responsable JS | ✅ Android exposera le hook FCM facultatif pour les applications de portefeuille ; JS reste basé sur des sondages avec un recul exponentiel, en tenant compte des contraintes de poussée du navigateur. |
| Garde-corps de cadence de ping/pong pour clients mobiles | Réseau Android TL | Responsable JS | ✅ Ping standardisé de 30 secondes avec une tolérance d'échec de 3 × ; Android équilibre l'impact de Doze, JS se fixe à ≥ 15 s pour éviter la limitation du navigateur. |

## Chiffrement et gestion des clés

| Sujet | Propriétaire d'Android | Propriétaire JS | Statut / Remarques |
|-------|--------------|---------------|----------------|
| Attentes de stockage des clés X25519 (contextes sécurisés StrongBox, WebCrypto) | Android Crypto TL | Responsable JS | ✅ Android stocke X25519 dans StrongBox lorsqu'il est disponible (revient à TEE) ; JS impose WebCrypto en contexte sécurisé pour les dApps, en revenant au pont natif `iroha_js_host` dans Node. |
| ChaCha20-Poly1305 partage de gestion des cas occasionnels entre les SDK | Android Crypto TL | Responsable JS | ✅ Adoptez l'API de compteur `sequence` partagée avec Wrap Guard 64 bits et tests partagés ; JS utilise des compteurs BigInt pour correspondre au comportement de Rust. |
| Schéma de charge utile d'attestation basée sur le matériel | Android Crypto TL | Responsable JS | ✅ Schéma finalisé : `attestation { platform, evidence_b64, statement_hash }` ; JS facultatif (navigateur), Node utilise le hook de plug-in HSM. |
| Flux de récupération des portefeuilles perdus (poignée de main avec rotation des clés) | Android Crypto TL | Responsable JS | ✅ Poignée de main pour la rotation du portefeuille acceptée : dApp émet le contrôle `rotate`, le portefeuille répond avec une nouvelle clé publique + un accusé de réception signé ; JS ressaisit immédiatement le matériel WebCrypto. |

## Ensembles d'autorisations et de preuves| Sujet | Propriétaire d'Android | Propriétaire JS | Statut / Remarques |
|-------|--------------|---------------|----------------|
| Schéma d'autorisation minimum (méthodes/événements/ressources) pour GA | Modèle de données Android TL | Responsable JS | ✅ Référence GA : `methods`, `events`, `resources`, `constraints` ; JS aligne les types TypeScript avec le manifeste Rust. |
| Charge utile de rejet du portefeuille (`reason_code`, messages localisés) | Réseau Android TL | Responsable JS | ✅ Codes finalisés (`user_declined`, `permissions_mismatch`, `compliance_failed`, `internal_error`) plus `localized_message` en option. |
| Champs facultatifs de l’ensemble de preuves (pièces jointes de conformité/KYC) | Modèle de données Android TL | Responsable JS | ✅ Tous les SDK acceptent les `attachments[]` (Norito `AttachmentRef`) et `compliance_manifest_id` en option ; aucun changement de comportement n’est requis. |
| Alignement sur le schéma JSON Norito par rapport aux structures générées par le pont | Modèle de données Android TL | Responsable JS | ✅ Décision : préférer les structures générées par le pont ; Le chemin JSON reste uniquement pour le débogage, JS conserve l'adaptateur `Value`. |

## Façades du SDK et forme de l'API

| Sujet | Propriétaire d'Android | Propriétaire JS | Statut / Remarques |
|-------|--------------|---------------|----------------|
| Parité des interfaces asynchrones de haut niveau (`Flow`, itérateurs asynchrones) | Réseau Android TL | Responsable JS | ✅ Android expose `Flow<ConnectEvent>` ; JS utilise `AsyncIterable<ConnectEvent>` ; les deux correspondent au `ConnectEventKind` partagé. |
| Mappage de taxonomie des erreurs (`ConnectError`, sous-classes typées) | Réseau Android TL | Responsable JS | ✅ Adoptez l'énumération partagée {`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, `Internal`} avec des détails de charge utile spécifiques à la plate-forme. |
| Sémantique d'annulation pour les demandes de signalisation en vol | Réseau Android TL | Responsable JS | ✅ Introduction du contrôle `cancelRequest(hash)` ; les deux SDK font apparaître des coroutines/promesses annulables respectant la reconnaissance du portefeuille. |
| Hooks de télémétrie partagés (événements, dénomination des métriques) | Réseau Android TL | Responsable JS | ✅ Noms des métriques alignés : `connect.queue_depth`, `connect.latency_ms`, `connect.reconnects_total` ; échantillon d’exportateurs documentés. |

## Persistance et journalisation hors ligne

| Sujet | Propriétaire d'Android | Propriétaire JS | Statut / Remarques |
|-------|--------------|---------------|----------------|
| Format de stockage pour les trames en file d'attente (binaire Norito vs JSON) | Modèle de données Android TL | Responsable JS | ✅ Stockez le binaire Norito (`.to`) partout ; JS utilise IndexedDB `ArrayBuffer`. |
| Politique de conservation des journaux et limites de taille | Réseau Android TL | Responsable JS | ✅ Rétention par défaut 24h et 1Mo par session ; configurable via `ConnectQueueConfig`. |
| Résolution de conflits lorsque les deux côtés rejouent des images | Réseau Android TL | Responsable JS | ✅ Utilisez `sequence` + `payload_hash` ; doublons ignorés, les conflits déclenchent `ConnectError.Internal` avec événement de télémétrie. |
| Télémétrie pour la profondeur de la file d'attente et le succès de la relecture | Réseau Android TL | Responsable JS | ✅ Émettre une jauge `connect.queue_depth` et un compteur `connect.replay_success_total` ; les deux SDK se connectent au schéma de télémétrie Norito partagé. |

## Pointes de mise en œuvre et références- **Appareils Rust Bridge :** `crates/connect_norito_bridge/src/lib.rs` et les tests associés couvrent les chemins d'encodage/décodage canoniques utilisés par chaque SDK.
- **Explorateur de démonstration Swift :** Exercices `examples/ios/NoritoDemoXcode/NoritoDemoXcodeTests/ConnectViewModelTests.swift` Connectez les flux de session avec des transports simulés.
- **Swift CI gating :** exécutez `make swift-ci` lors de la mise à jour des artefacts Connect pour valider la parité des appareils, les flux du tableau de bord et les métadonnées Buildkite `ci/xcframework-smoke:<lane>:device_tag` avant de les partager avec d'autres SDK.
- **Tests d'intégration du SDK JavaScript :** `javascript/iroha_js/test/integrationTorii.test.js` valide les assistants d'état/de session Connect par rapport à Torii.
- **Remarques sur la résilience du client Android :** `java/iroha_android/README.md:150` documente les expériences de connectivité actuelles qui ont inspiré les valeurs par défaut de file d'attente/back-off.

## Articles de préparation à l'atelier

- [x] Android : attribuez une personne responsable à chaque ligne du tableau ci-dessus.
- [x] JS : attribuez une personne responsable pour chaque ligne du tableau ci-dessus.
- [x] Collectez des liens vers des pics ou des expériences de mise en œuvre existantes.
- [x] Planifier un examen préalable aux travaux avant le conseil de février 2026 (réservé pour le 29/01/2026 à 15h00 UTC avec Android TL, JS Lead, Swift Lead).
- [x] Mettre à jour `docs/source/connect_architecture_strawman.md` avec les réponses acceptées.

## Package de pré-lecture

- ✅ Bundle enregistré sous `artifacts/connect/pre-read/20260129/` (généré via `make docs-html` après avoir actualisé le Strawman, les guides SDK et cette liste de contrôle).
- 📄Résumé + étapes de distribution en direct dans `docs/source/project_tracker/connect_architecture_pre_read.md` ; incluez le lien dans l'invitation à l'atelier de février 2026 et dans le rappel `#sdk-council`.
- 🔁 Lors de l'actualisation du bundle, mettez à jour le chemin et le hachage dans la note de pré-lecture et archivez l'annonce dans `status.md` sous les journaux de préparation IOS7/AND7.