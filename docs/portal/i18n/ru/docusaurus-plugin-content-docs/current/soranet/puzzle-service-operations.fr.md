---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: puzzle-service-operations
title: Guide d'operations du service de puzzles
sidebar_label: Ops du service de puzzles
description: Exploitation du daemon `soranet-puzzle-service` pour les tickets d'admission Argon2/ML-DSA.
---

:::note Source canonique
:::

# Guide d'operations du service de puzzles

Le daemon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) emet des
admission tickets bases sur Argon2 qui refletent la policy `pow.puzzle.*` du
relay et, lorsque configure, orchestre des tokens d'admission ML-DSA pour les
relays edge. Il expose cinq endpoints HTTP:

- `GET /healthz` - probe de liveness.
- `GET /v1/puzzle/config` - retourne les parametres PoW/puzzle effectifs issus
  du JSON relay (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` - emet un ticket Argon2; un body JSON optionnel
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  demande un TTL plus court (clamp au window de policy), lie le ticket a un
  transcript hash et renvoie un ticket signe par le relay + l'empreinte de
  signature lorsque des cles de signature sont configurees.
- `GET /v1/token/config` - quand `pow.token.enabled = true`, retourne la policy
  d'admission-token active (issuer fingerprint, limites TTL/clock-skew, relay ID,
  et l'ensemble de revocation fusionne).
- `POST /v1/token/mint` - emet un token d'admission ML-DSA lie au resume hash
  fourni; le body accepte `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

Les tickets produits par le service sont verifies dans le test d'integration
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, qui exerce aussi les
throttles du relay lors de scenarios DoS volumetriques.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Configurer l'emission de tokens

Definissez les champs JSON du relay sous `pow.token.*` (voir
`tools/soranet-relay/deploy/config/relay.entry.json` pour un exemple) afin
d'activer les tokens ML-DSA. Au minimum, fournissez la cle publique de l'issuer
et une liste de revocation optionnelle:

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

Le puzzle service reutilise ces valeurs et recharge automatiquement le fichier
Norito JSON de revocation en runtime. Utilisez le CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) pour emettre et
inspecter des tokens offline, ajouter des entrees `token_id_hex` au fichier de
revocation, et auditer les credentials existants avant de pousser des updates
en production.

Passez la cle secrete de l'issuer au puzzle service via les flags CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` est aussi disponible lorsque le secret est gere par un
pipeline out-of-band. Le watcher du fichier de revocation garde `/v1/token/config`
a jour; coordonnez les mises a jour avec la commande `soranet-admission-token revoke`
pour eviter un etat de revocation en retard.

Definissez `pow.signed_ticket_public_key_hex` dans le JSON relay pour annoncer
la cle publique ML-DSA-44 utilisee pour verifier les PoW tickets signes; `/v1/puzzle/config`
repete la cle et son empreinte BLAKE3 (`signed_ticket_public_key_fingerprint_hex`) afin
que les clients puissent pinner le verificateur. Les tickets signes sont valides
contre le relay ID et les transcript bindings et partagent le meme store de
revocation; les PoW tickets bruts de 74 octets restent valides quand le verifier
signed-ticket est configure. Passez le secret de signature via `--signed-ticket-secret-hex`
ou `--signed-ticket-secret-path` au lancement du puzzle service; le demarrage
rejette les keypairs incoherents si le secret ne valide pas contre
`pow.signed_ticket_public_key_hex`. `POST /v1/puzzle/mint` accepte `"signed": true`
(et optionnel `"transcript_hash_hex"`) pour renvoyer un ticket signe Norito en
plus des bytes du ticket brut; les reponses incluent `signed_ticket_b64` et
`signed_ticket_fingerprint_hex` pour suivre les fingerprints de replay. Les
requêtes avec `signed = true` sont rejetees si le secret de signature n'est pas
configure.

## Playbook de rotation des cles

1. **Collecter le nouveau descriptor commit.** Governance publie le relay
   descriptor commit dans le directory bundle. Copiez la chaine hex dans
   `handshake.descriptor_commit_hex` dans la configuration JSON relay partagee
   avec le puzzle service.
2. **Verifier les bornes de policy puzzle.** Confirmez que les valeurs
   `pow.puzzle.{memory_kib,time_cost,lanes}` mises a jour s'alignent avec le plan
   de release. Les operateurs doivent garder la configuration Argon2 deterministe
   entre relays (minimum 4 MiB de memoire, 1 <= lanes <= 16).
3. **Preparer le redemarrage.** Rechargez l'unite systemd ou le container une
   fois que governance annonce le cutover de rotation. Le service ne supporte
   pas le hot-reload; un redemarrage est requis pour prendre le nouveau descriptor
   commit.
4. **Valider.** Emettez un ticket via `POST /v1/puzzle/mint` et confirmez que
   `difficulty` et `expires_at` correspondent a la nouvelle policy. Le rapport
   soak (`docs/source/soranet/reports/pow_resilience.md`) capture des bornes de
   latence attendues pour reference. Lorsque les tokens sont actives, lisez
   `/v1/token/config` pour verifier que l'issuer fingerprint annonce et le
   compte de revocation correspondent aux valeurs attendues.

## Procedure de desactivation d'urgence

1. Definissez `pow.puzzle.enabled = false` dans la configuration relay partagee.
   Gardez `pow.required = true` si les tickets hashcash fallback doivent rester
   obligatoires.
2. Optionnellement, imposez des entrees `pow.emergency` pour rejeter les
   descriptors obsoletes pendant que la porte Argon2 est offline.
3. Redemarrez a la fois le relay et le puzzle service pour appliquer le
   changement.
4. Surveillez `soranet_handshake_pow_difficulty` pour verifier que la difficulte
   tombe a la valeur hashcash attendue, et validez que `/v1/puzzle/config`
   rapporte `puzzle = null`.

## Monitoring et alerting

- **Latency SLO:** Suivez `soranet_handshake_latency_seconds` et gardez le P95
  sous 300 ms. Les offsets du soak test fournissent des donnees de calibration
  pour les throttles de guard.【docs/source/soranet/reports/pow_resilience.md:1】
- **Quota pressure:** Utilisez `soranet_guard_capacity_report.py` avec les
  metrics relay pour ajuster les cooldowns `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Puzzle alignment:** `soranet_handshake_pow_difficulty` doit correspondre a la
  difficulte retournee par `/v1/puzzle/config`. Une divergence indique une config
  relay stale ou un redemarrage rate.
- **Token readiness:** Alertez si `/v1/token/config` chute a `enabled = false`
  de maniere inattendue ou si `revocation_source` rapporte des timestamps stale.
  Les operateurs doivent faire tourner le fichier de revocation Norito via le CLI
  des qu'un token est retire pour garder cet endpoint precis.
- **Service health:** Probez `/healthz` avec la cadence de liveness habituelle et
  alertez si `/v1/puzzle/mint` renvoie des reponses HTTP 500 (indique un mismatch
  des parametres Argon2 ou des echecs RNG). Les erreurs de token minting se
  manifestent via des reponses HTTP 4xx/5xx sur `/v1/token/mint`; traitez les
  echecs repetes comme une condition de paging.

## Compliance et audit logging

Les relays emettent des evenements `handshake` structures qui incluent les raisons
throttle et les durees de cooldown. Assurez-vous que le pipeline de compliance
descrit dans `docs/source/soranet/relay_audit_pipeline.md` ingere ces logs afin
que les changements de policy puzzle restent auditables. Quand la porte puzzle
est active, archivez des echantillons de tickets mintes et le snapshot de
configuration Norito avec le ticket de rollout pour les audits futurs. Les
admission tokens mintes avant les fenetres de maintenance doivent etre suivis
avec leurs valeurs `token_id_hex` et inseres dans le fichier de revocation une
fois expires ou revoques.
