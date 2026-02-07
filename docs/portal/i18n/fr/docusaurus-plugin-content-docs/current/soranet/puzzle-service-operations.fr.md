---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : puzzle-service-opérations
titre : Guide d'opérations du service de puzzles
sidebar_label : Opérations du service de puzzles
description : Exploitation du démon `soranet-puzzle-service` pour les tickets d'entrée Argon2/ML-DSA.
---

:::note Source canonique
:::

# Guide d'opérations du service de puzzles

Le démon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) emet des
billets d'entrée bases sur Argon2 qui reflètent la politique `pow.puzzle.*` du
relay et, lorsque configure, orchestre des tokens d'admission ML-DSA pour les
bord des relais. Il expose cinq points de terminaison HTTP :

- `GET /healthz` - sonde de vivacité.
- `GET /v1/puzzle/config` - retourne les paramètres PoW/puzzle effectifs issus
  du relais JSON (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` - pour obtenir un ticket Argon2 ; un corps JSON optionnel
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  demander un TTL plus court (clamp au window de politique), lie le ticket a un
  transcript hash et renvoyer un ticket signé par le relay + l'empreinte de
  signature lorsque les clés de signature sont configurées.
- `GET /v1/token/config` - quand `pow.token.enabled = true`, retourne la politique
  d'admission-token actif (empreinte digitale de l'émetteur, limites TTL/clock-skew, identifiant de relais,
  et l'ensemble de révocation fusionnée).
- `POST /v1/token/mint` - emet un token d'admission ML-DSA se trouve au CV hash
  fourni; le corps accepte `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

Les tickets produits par le service sont vérifiés dans le test d'intégration
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, qui exerce aussi les
throttles du relay lors de scénarios DoS volumétriques.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Configurer l'émission de tokens

Définissez les champs JSON du relais sous `pow.token.*` (voir
`tools/soranet-relay/deploy/config/relay.entry.json` pour un exemple) afin
d'activer les tokens ML-DSA. Au minimum, fournissez la clé publique de l'émetteur
et une liste de révocation optionnelle :

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

Le service puzzle réutilise ces valeurs et recharge automatiquement le fichier
Norito JSON de révocation et d'exécution. Utilisez la CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) pour emettre et
inspecteur des tokens hors ligne, ajouter des entrées `token_id_hex` au fichier de
révocation, et auditer les informations d'identification existantes avant de pousser des mises à jour
en production.

Passez la clé secrète de l'émetteur au service puzzle via les flags CLI :

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` est également disponible lorsque le secret est germé par un
pipeline hors bande. Le watcher du fichier de révocation garde `/v1/token/config`
un jour; coordonnez les mises à jour avec la commande `soranet-admission-token revoke`
pour éviter un état de révocation en retard.Définissez `pow.signed_ticket_public_key_hex` dans le relais JSON pour annoncer
la clé publique ML-DSA-44 utilisée pour vérifier les panneaux de tickets PoW ; `/v1/puzzle/config`
repete la cle et son empreinte BLAKE3 (`signed_ticket_public_key_fingerprint_hex`) afin
que les clients peuvent épingler le vérificateur. Les panneaux tickets sont valides
contre le relay ID et les transcript binds et partager le meme store de
révocation; les billets PoW bruts de 74 octets restent valides quand le vérificateur
le ticket signé est configuré. Passez le secret de signature via `--signed-ticket-secret-hex`
ou `--signed-ticket-secret-path` au lancement du service de puzzle; le démarrage
rejette les keypairs incohérents si le secret ne valide pas contre
`pow.signed_ticket_public_key_hex`. `POST /v1/puzzle/mint` accepter `"signed": true`
(et optionnel `"transcript_hash_hex"`) pour renvoyer un ticket signé Norito fr
plus des octets du ticket brut; les réponses incluent `signed_ticket_b64` et
`signed_ticket_fingerprint_hex` pour suivre les empreintes digitales de replay. Les
Les requêtes avec `signed = true` sont rejetées si le secret de signature n'est pas
configurer.

## Playbook de rotation des clés

1. **Collecter le nouveau descriptor commit.** Governance publie le relay
   descripteur commit dans le répertoire bundle. Copiez la chaîne hex dans
   `handshake.descriptor_commit_hex` dans la configuration JSON relay partagee
   avec le service puzzles.
2. **Verifier les bornes de Policy Puzzle.** Confirmez que les valeurs
   `pow.puzzle.{memory_kib,time_cost,lanes}` mises à jour s'alignent avec le plan
   de libération. Les opérateurs doivent garder la configuration Argon2 déterministe
   entre relais (minimum 4 Mo de mémoire, 1 <= voies <= 16).
3. **Préparez le redemarrage.** Rechargez l'unité système ou le conteneur une
   fois que la gouvernance annonce le basculement de rotation. Le service ne supporte pas
   pas le hot-reload; un redemarrage est requis pour prendre le nouveau descriptor
   commettre.
4. **Valider.** Émettez un ticket via `POST /v1/puzzle/mint` et confirmez que
   `difficulty` et `expires_at` correspondant à la nouvelle politique. Le rapport
   trempage (`docs/source/soranet/reports/pow_resilience.md`) capture des bornes de
   latence attendue pour référence. Lorsque les tokens sont actifs, lisez
   `/v1/token/config` pour vérifier que l'émetteur d'empreintes digitales annonce et le
   compte de révocation correspondant aux valeurs attendues.

## Procédure de désactivation d'urgence

1. Définissez `pow.puzzle.enabled = false` dans la configuration relay partagee.
   Gardez `pow.required = true` si les tickets hashcash fallback doivent rester
   obligatoires.
2. Optionnellement, imposez des entrées `pow.emergency` pour rejeter les
   descripteurs obsolètes pendant que la porte Argon2 est hors ligne.
3. Redémarrez à la fois le relais et le service puzzle pour appliquer le
   changement.
4. Surveillez `soranet_handshake_pow_difficulty` pour vérifier que la difficulté
   tombe à la valeur hashcash attendue, et validez que `/v1/puzzle/config`
   rapport `puzzle = null`.

## Surveillance et alerte- **Latency SLO:** Suivez `soranet_handshake_latency_seconds` et gardez le P95
  sous 300 ms. Les offsets du test de trempage fournissent des données de calibrage
  pour les throttles de guard.【docs/source/soranet/reports/pow_resilience.md:1】
- **Quota Pressure :** Utilisez `soranet_guard_capacity_report.py` avec les
  metrics relay pour ajuster les cooldowns `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Alignement du puzzle :** `soranet_handshake_pow_difficulty` doit correspondre à la
  difficulté retournée par `/v1/puzzle/config`. Une divergence indique une configuration
  relais périmé ou un taux de remarrage.
- **Préparation du jeton :** Alertez si `/v1/token/config` chute à `enabled = false`
  de manière inattendue ou si `revocation_source` rapporte des timestamps obsolètes.
  Les opérateurs doivent faire tourner le fichier de révocation Norito via le CLI
  des qu'un token est retiré pour garder cet endpoint précis.
- **Service santé :** Probez `/healthz` avec la cadence de vivacité habituelle et
  alertez si `/v1/puzzle/mint` renvoie des réponses HTTP 500 (indiquer un décalage
  des paramètres Argon2 ou des résultats RNG). Les erreurs de token minting se
  se manifeste via des réponses HTTP 4xx/5xx sur `/v1/token/mint`; traitez les
  les echecs se répètent comme une condition de pagination.

## Journalisation de conformité et d'audit

Les relais emettent des événements `handshake` structures qui incluent les raisons
throttle et les durées de recharge. Assurez-vous que le pipeline de conformité
descrit dans `docs/source/soranet/relay_audit_pipeline.md` ingère ces logs afin
que les changements de politique restent vérifiables. Puzzle Quand la porte
est actif, archivez des echantillons de tickets menthes et le snapshot de
configuration Norito avec le ticket de déploiement pour les audits futurs. Les
les jetons d'admission menthes avant les fenêtres de maintenance doivent être suivis
avec leurs valeurs `token_id_hex` et insérées dans le fichier de révocation une
fois expire ou révoque.