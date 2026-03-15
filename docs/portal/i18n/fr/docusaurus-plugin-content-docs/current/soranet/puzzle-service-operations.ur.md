---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : puzzle-service-opérations
titre : Guide des opérations du service Puzzle
sidebar_label : Opérations de service de puzzle
description : Billets d'entrée Argon2/ML-DSA pour le démon `soranet-puzzle-service`.
---

:::note Source canonique
`docs/source/soranet/puzzle_service_operations.md` pour le client جب تک پرانا documentation set retiré نہ ہو، دونوں ورژنز رکھیں۔
:::

# Guide des opérations du service de puzzle

Démon `tools/soranet-puzzle-service/` et `soranet-puzzle-service`
Billets d'entrée soutenus par Argon2 et politique de relais `pow.puzzle.*`
Un miroir est utilisé pour configurer un relais de bord et un ML-DSA
courtier en jetons d'admission کرتا ہے۔ Les points de terminaison HTTP exposent les éléments suivants :

- `GET /healthz` - sonde de vivacité.
- `GET /v1/puzzle/config` - relais JSON (`handshake.descriptor_commit_hex`, `pow.*`) سے
  Voici les paramètres PoW/puzzle et les détails
- `POST /v1/puzzle/mint` - Billet Argon2 neuf corps JSON facultatif
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  Il s'agit d'un TTL ou d'un ticket (fenêtre de politique, pince), d'un ticket et d'un hachage de transcription.
  سے bind کرتا ہے، اور clés de signature configurées ہوں تو ticket signé par relais +
  signature empreinte digitale واپس کرتا ہے۔
- `GET /v1/token/config` - pour `pow.token.enabled = true` et un jeton d'admission actif
  politique واپس کرتا ہے (empreinte digitale de l'émetteur, limites TTL/inclinaison d'horloge, ID de relais, اور
  ensemble de révocation fusionné).
- `POST /v1/token/mint` - Jeton d'admission ML-DSA neuf et hachage de CV fourni
  سے lié ہوتا ہے؛ corps de demande `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`
  قبول کرتا ہے۔

Service et tickets de service et test d'intégration
`volumetric_dos_soak_preserves_puzzle_and_latency_slo` میں verify کیا جاتا ہے، جو
scénarios DoS volumétriques avec manettes de relais et exercices
【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Configuration de l'émission de jetons کرنا

`pow.token.*` est un ensemble de champs JSON de relais définis (pour plus de détails
`tools/soranet-relay/deploy/config/relay.entry.json` (`tools/soranet-relay/deploy/config/relay.entry.json`) pour les jetons ML-DSA
activer ہوں۔ Il s'agit d'une clé publique de l'émetteur et d'une liste de révocation facultative.

```json
"pow": {
  "token": {
    "enabled": true,
    "issuer_public_key_hex": "<ML-DSA-44 public key>",
    "revocation_list_hex": [],
    "revocation_list_path": "/etc/soranet/relay/token_revocations.json"
  }
}
```

Service de puzzle valeurs et réutilisation du temps d'exécution et révocation JSON Norito
فائل کو خودکار طور پر reload کرتا ہے۔ CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) استعمال کریں تاکہ
jetons hors ligne menthe/inspecter et révocation ici `token_id_hex` entrées ajouter ici
اور mises à jour de production et audit des informations d'identification ہوں۔

Clé secrète de l'émetteur et drapeaux CLI pour le service de puzzle et le service :

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` est un pipeline d'outils hors bande secret pour gérer et gérer
Observateur de fichiers de révocation `/v1/token/config` et version actuelle mises à jour
`soranet-admission-token revoke` Coordonnées de l'état de révocation
décalage نہ کرے۔Relay JSON میں `pow.signed_ticket_public_key_hex` set کریں تاکہ tickets PoW signés
vérifier la clé publique ML-DSA-44 et la publicité `/v1/puzzle/config` clé et
L'empreinte digitale BLAKE3 (`signed_ticket_public_key_fingerprint_hex`) echo est disponible
broche de vérification des clients کر سکیں۔ ID de relais de billets signés et liaisons de transcription
valider le magasin de révocation et partager le magasin tickets PoW bruts de 74 octets
vérificateur de billets signés configuré ہونے پر بھی valide رہتے ہیں۔ Secret du signataire
`--signed-ticket-secret-hex` et `--signed-ticket-secret-path` pour le lancement du service
پر passer کریں؛ les paires de clés incompatibles au démarrage rejettent le secret
`pow.signed_ticket_public_key_hex` pour valider ici `POST /v1/puzzle/mint`
`"signed": true` (`"transcript_hash_hex"` en option) encodé en mode Norito
ticket signé octets bruts du ticket کے ساتھ واپس ہو؛ réponses میں `signed_ticket_b64`
اور `signed_ticket_fingerprint_hex` شامل ہوتے ہیں تاکہ replay empreintes digitales track ہوں۔
` signed = true` et demandes de rejet ہیں اگر secret du signataire configuré نہ ہو۔

## Manuel de rotation des clés

1. **Commit de descripteur de bonne qualité** Ensemble d'annuaires de gouvernance et relais
   descripteur commit publier کرتی ہے۔ Chaîne hexadécimale et configuration JSON du relais
   `handshake.descriptor_commit_hex` copie du service de puzzle et service de puzzle partagé
2. ** Examen des limites de la politique de puzzle ici ** Mise à jour des limites de la politique de puzzle
   Plan de publication des valeurs `pow.puzzle.{memory_kib,time_cost,lanes}` Opérateurs ici
   Les relais de configuration Argon2 sont dotés d'une mémoire déterministe (avec une mémoire de 4 Mo)
   1 <= voies <= 16).
3. **Étape de redémarrage ici** Annonce du basculement de la rotation de la gouvernance dans l'unité systemd ici
   rechargement de conteneurs Service de recharge à chaud disponible نیا descripteur commit لینے کے لئے
   redémarrer ضروری ہے۔
4. **Valider le ticket** `POST /v1/puzzle/mint` pour l'émission du ticket et le ticket de caisse.
   `difficulty` et `expires_at` Politique de conformité et correspondance Rapport de trempage
   (`docs/source/soranet/reports/pow_resilience.md`) référence aux limites de latence attendues
   capturer کرتا ہے۔ Les jetons permettent à `/v1/token/config` de récupérer un émetteur annoncé
   empreinte digitale et nombre de révocations valeurs attendues et correspondance

## Procédure de désactivation d'urgence

1. Configuration du relais partagé selon `pow.puzzle.enabled = false` défini sur
   `pow.required = true` Billets de repli pour hashcash
2. Les entrées facultatives de type `pow.emergency` appliquent la porte Argon2 hors ligne.
   les descripteurs périmés rejettent ہوں۔
3. Relay اور puzzle service دونوں restart کریں تاکہ تبدیلی appliquer ہو۔
4. `soranet_handshake_pow_difficulty` surveille la difficulté de la valeur de hashcash attendue
   تک drop ہو، اور verify کریں کہ `/v1/puzzle/config` `puzzle = null` رپورٹ کرے۔

## Surveillance et alerte- **SLO de latence :** Piste `soranet_handshake_latency_seconds` pour P95 à 300 ms pour la durée de vie
  Le test de trempage décale les régulateurs de protection et les données d'étalonnage
  【docs/source/soranet/reports/pow_resilience.md:1】
- **Pression de quota :** `soranet_guard_capacity_report.py` et métriques de relais
  Temps de recharge `pow.quotas` (`soranet_abuse_remote_cooldowns`, `soranet_handshake_throttled_remote_quota_total`) réglés
  【docs/source/soranet/relay_audit_pipeline.md:68】
- **Alignement du puzzle :** `soranet_handshake_pow_difficulty` et `/v1/puzzle/config` et `soranet_handshake_pow_difficulty` et `/v1/puzzle/config`.
  difficulté کے ساتھ match ہونا چاہئے۔ Divergence de configuration de relais obsolète et échec du redémarrage
- **Préparation au jeton :** اگر `/v1/token/config` غیر متوقع طور پر `enabled = false` ہو جائے یا
  `revocation_source` horodatages périmés et alerte d'alerte Opérateurs et CLI pour Norito
  révocation rotation du fichier rotation du jeton retrait du point de terminaison
- **Santé du service :** `/healthz` pour la cadence de vivacité et la sonde et l'alerte.
  `/v1/puzzle/mint` Réponses HTTP 500 pour (incompatibilité des paramètres Argon2 et échecs RNG par exemple).
  Erreurs de création de jetons `/v1/token/mint` dans les réponses HTTP 4xx/5xx échecs répétés
  Condition de pagination

## Conformité et journalisation d'audit

Les événements structurés `handshake` des relais émettent des raisons d'accélération et des durées de refroidissement.
L'application `docs/source/soranet/relay_audit_pipeline.md` fournit un pipeline de conformité pour les journaux et l'ingestion
Les changements de politique de puzzle vérifiables La porte de puzzle active ہو تو des échantillons de billets émis et la configuration Norito
instantané et ticket de déploiement pour les archives et les audits pour les audits Fenêtres de maintenance سے پہلے
menthe il y a des jetons d'admission et des valeurs `token_id_hex` et une piste de suivi et d'expiration et de révocation
پر fichier de révocation میں insert کیا جانا چاہئے۔