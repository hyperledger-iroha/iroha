---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : puzzle-service-opérations
titre : Guide d'utilisation du service de puzzles
sidebar_label : Opérations du service de puzzles
description : Opération du démon `soranet-puzzle-service` pour les billets d'admission Argon2/ML-DSA.
---

:::note Fuente canonica
Refleja `docs/source/soranet/puzzle_service_operations.md`. Mantengan ambas versionses sincronizadas hasta que los docs heredados se retraiten.
:::

# Guide d'utilisation du service de puzzles

Le démon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) émet
billets d'entrée respaldados por Argon2 que reflejan la politique `pow.puzzle.*`
du relais et, lorsqu'il est configuré, jetons d'admission intermédia ML-DSA fr
nombre de relais bord. Exposez les points de terminaison cinco HTTP :

- `GET /healthz` - sonde de vivacité.
- `GET /v1/puzzle/config` - développer les paramètres efficaces de PoW/puzzle
  tomes du relais JSON (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` - détecte un ticket Argon2 ; un corps JSON facultatif
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  solliciter un TTL mas corto (pincer la fenêtre de politique), ata el ticket a un
  transcript hash et dévuelve un ticket firmado por el relay + la huella de
  la firma cuando hay claves de firmado configuradas.
- `GET /v1/token/config` - quand `pow.token.enabled = true`, devuelve la
  politique active de jeton d'admission (empreinte digitale de l'émetteur, limites de TTL/clock-skew,
  ID de relais et ensemble de révocation combinés).
- `POST /v1/token/mint` - détecte un jeton d'admission ML-DSA lié au CV
  condition de hachage ; le corps accepte `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

Les tickets produits par le service sont vérifiés lors de l'essai d'intégration
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, qui peut également être éjecté
les manettes du relais pendant les scénarios de DoS volumétriques.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Configurer l'émission de jetons

Configurer les champs JSON du relais sous `pow.token.*` (ver
`tools/soranet-relay/deploy/config/relay.entry.json` comme exemple) pour activer
les jetons ML-DSA. Au minimum, prouvez la clé publique de l'émetteur et une liste
révocation facultative :

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

Le service de puzzle réutilise ces valeurs et recharge automatiquement les archives
Norito JSON de révocations au runtime. Utiliser la CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) pour savoir et
inspecter les jetons hors ligne, ajouter les entrées `token_id_hex` aux archives de
révocaciones y auditar credenciales existantes avant de publier les mises à jour et
production.

Passer la clé secrète de l'émetteur du service de puzzle via flags CLI :

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` est également disponible lorsque le secret est géré par
un pipeline d'outillage hors bande. Le surveillant du fichier de révocation
maintenir `/v1/token/config` actualisé; coordonnées des mises à jour avec le commandant
`soranet-admission-token revoke` pour éviter les défaillances à l'état de révocation.Configurer `pow.signed_ticket_public_key_hex` dans le relais JSON pour annoncer
la clé publique ML-DSA-44 utilisée pour vérifier les billets PoW firmados ; `/v1/puzzle/config`
réplique la clave et su huella BLAKE3 (`signed_ticket_public_key_fingerprint_hex`)
pour que les clients puissent fijar le vérificateur. Les billets confirmés sont validés
contre l'ID de relais et les liaisons de transcription et compartimenter le même magasin de révocation ;
Les tickets PoW bruts de 74 octets sont toujours valides lorsque le vérificateur de
le billet signé est configuré. Pas le secret du cabinet via `--signed-ticket-secret-hex`
o `--signed-ticket-secret-path` pour démarrer le service de puzzle ; el arranque rechaza
pares de claves que no coïnciden si el secreto no valida contra `pow.signed_ticket_public_key_hex`.
`POST /v1/puzzle/mint` accepte `"signed": true` (et facultatif `"transcript_hash_hex"`) pour
transférer un ticket ferme Norito avec les octets du ticket en brut ; las
Les réponses incluent `signed_ticket_b64` et `signed_ticket_fingerprint_hex` pour aider
un rastrear empreintes digitales de replay. Les sollicitudes avec `signed = true` se rechazan
si le secret de l'entreprise n'est pas configuré.

## Playbook de rotation des touches

1. **Recoleccion del nuevo descriptor commit.** Gouvernance publique du relais
   descripteur commit dans le bundle du répertoire. Copier la chaîne hex fr
   `handshake.descriptor_commit_hex` dans la configuration JSON du relais
   partagé avec le service de puzzle.
2. **Révision des limites de la politique du puzzle.** Confirma que los valores
   `pow.puzzle.{memory_kib,time_cost,lanes}` actualisés selon le plan
   de libération. Les opérateurs doivent maintenir la configuration Argon2 déterministe
   entre les relais (minimum 4 MiB de mémoire, 1 <= voies <= 16).
3. **Préparer le programme.** Recharger l'unité système ou le conteneur une fois
   que la gouvernance annonce le basculement de la rotation. Le service ne prend pas en charge le rechargement à chaud ;
   vous avez besoin d'être rejoint pour lancer le nouveau descripteur commit.
4. **Valider.** Émettez un ticket via `POST /v1/puzzle/mint` et confirmez que les
   les valeurs `difficulty` et `expires_at` coïncident avec la nouvelle politique. El informer
   de trempage (`docs/source/soranet/reports/pow_resilience.md`) capture les limites
   de latencia espérés comme référence. Lorsque les jetons sont habilités,
   Consultez `/v1/token/config` pour vous assurer que l'empreinte digitale de l'émetteur est annoncée
   et le conteo de révocations coïncide avec les valeurs espérées.

## Procédure de déshabilitation d'urgence

1. Configurez `pow.puzzle.enabled = false` dans la configuration du relais.
   Mantiene `pow.required = true` si les tickets hashcash fallback doivent être suivis
   siendo obligatoires.
2. Application facultative des entrées `pow.emergency` pour rechercher des descripteurs
   obsolètes alors que la porte Argon2 est hors ligne.
3. Réinitialisez le relais et le service de puzzle pour appliquer le changement.
4. Monitorea `soranet_handshake_pow_difficulty` pour garantir la difficulté
   cae al valor hashcash espéré, et vérifier que `/v1/puzzle/config` rapporte
   `puzzle = null`.

## Surveillance et alertes- **Latence SLO :** Rastrea `soranet_handshake_latency_seconds` et maintient le P95
  par débarras de 300 ms. Les décalages du test de trempage ont prouvé les données de calibrage
  pour les manettes de garde.【docs/source/soranet/reports/pow_resilience.md:1】
- **Pression de quota :** Usa `soranet_guard_capacity_report.py` avec métriques de relais
  pour ajuster les temps de recharge de `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Alignement du puzzle :** `soranet_handshake_pow_difficulty` doit coïncider avec la
  difficulté à développer par `/v1/puzzle/config`. Configuration de la divergence indica
  le relais est périmé ou un roi est tombé en panne.
- **Préparation au jeton :** Alerte si `/v1/token/config` cae a `enabled = false`
  Inespéré ou si `revocation_source` signale des horodatages obsolètes. Les opérateurs
  devez faire pivoter le fichier de révocation Norito via CLI lorsque vous retirez un jeton
  pour maintenir ce point final précis.
- **Service santé :** Sondea `/healthz` avec la cadence habituelle de vivacité et d'alerte
  si `/v1/puzzle/mint` donne des réponses HTTP 500 (indique une incompatibilité des paramètres
  Argon2 ou erreurs RNG). Les erreurs de frappe de jetons apparaissent avec des réponses
  HTTP 4xx/5xx et `/v1/token/mint` ; trata fallas repetidas como condition de paging.

## Journalisation de conformité et d'audit

Les relais émettent des événements `handshake` structurés qui incluent des raisons de
Accélérateur et durée du temps de recharge. Assurez-vous que le pipeline de conformité décrit
et `docs/source/soranet/relay_audit_pipeline.md` ingère ces journaux pour les
changements de politique du puzzle sigan siendo auditables. Lorsque vous ouvrez la porte du puzzle
este habilité, archives des billets émis et instantané de configuration
Norito avec le ticket de déploiement pour les futurs auditoriums. Les jetons d'admission
les émissions avant les fenêtres de maintenance doivent être rastrées avec leurs valeurs
`token_id_hex` et insérer dans le fichier de révocation à l'expiration du
Sean Revocados.