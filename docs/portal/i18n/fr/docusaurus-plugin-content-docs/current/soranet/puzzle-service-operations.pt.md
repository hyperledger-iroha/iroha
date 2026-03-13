---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : puzzle-service-opérations
titre : Guia de operacoes do Puzzle Service
sidebar_label : opérations du service Puzzle
description: Operacao do daemon `soranet-puzzle-service` pour billets d'entrée Argon2/ML-DSA.
---

:::note Fonte canonica
Espelha `docs/source/soranet/puzzle_service_operations.md`. Mantenha ambas comme copies synchronisées.
:::

# Guide des opéras du service Puzzle

O démon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) émet
billets d'entrée respaldados por Argon2 qui reflètent une politique `pow.puzzle.*`
faire le relais et, une fois configuré, faz courtier de jetons d'admission ML-DSA en nom
relais dos edge. Il expose cinq points de terminaison HTTP :

- `GET /healthz` - sonde de vivacité.
- `GET /v2/puzzle/config` - retour des paramètres efficaces de PoW/puzzle extraidos
faire JSON faire le relais (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v2/puzzle/mint` - émettre un ticket Argon2 ; euh corps JSON facultatif
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  pede um TTL mineur (clamp na janela de Policy), vincula o ticket a um transcript
  hash et retour d'un ticket assassiné par relais + empreinte digitale de l'Assinatura quando
  car les chaves d’assinatura sont configurés.
- `GET /v2/token/config` - quand `pow.token.enabled = true`, retourner une politique
  ativa de admission-token (empreinte digitale de l'émetteur, limites de TTL/clock-skew, ID de relais
  e o ensemble de révocation mesclado).
- `POST /v2/token/mint` - émet un jeton d'admission ML-DSA confirmé par le hachage de reprise
  fornécido; o corps aceita `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

Les billets produits par le service sao vérifiés dans le test d'intégration
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, que vous exercez également
les manettes font le relais pendant les scénarios de DoS volumétriques.
(tools/soranet-relay/tests/adaptive_and_puzzle.rs:337)

## Configurer l'émission de jetons

Définir les champs JSON pour le relais dans `pow.token.*` (voir
`tools/soranet-relay/deploy/config/relay.entry.json` comme exemple) pour habiliter
Jetons ML-DSA. Au minimum, fournir une clé publique à l'émetteur et à une liste de révocation
facultatif :

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

Le service de puzzle réutilise ses valeurs et les récupère automatiquement.
Norito JSON de retour au runtime. Utilisez la CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) pour émetteur
inspecter les jetons hors ligne, ajouter les entrées `token_id_hex` à l'archive de revogacao
Et vérifier les références existantes avant de publier les mises à jour dans la production.

Transmettez une clé secrète d'émetteur au service de puzzle via la CLI des indicateurs :

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` également disponible lorsque le secret est géré par un
pipeline d'outillage forum de bande. O watcher do arquivo de revogacao mantem
`/v2/token/config` actualisé ; coordonner les mises à jour avec le commando
`soranet-admission-token revoke` pour éviter un état de revogacao défasado.Définir `pow.signed_ticket_public_key_hex` aucun relais JSON pour annoncer un message
publica ML-DSA-44 utilisé pour vérifier les billets de prisonnier de guerre assassinés ; `/v2/puzzle/config`
reproduire votre doigt et votre empreinte digitale BLAKE3 (`signed_ticket_public_key_fingerprint_hex`)
pour que les clients puissent réparer ou vérifier. Billets assinados sao validados contra
o l'ID de relais et les liaisons de transcription et le compartiment du magasin de révocation mesmo ; PoW
billets bruts de 74 octets valides en permanence lorsque le vérificateur de billets signés
ainda esta configuré. Transmettez le secret du signataire via `--signed-ticket-secret-hex` ou
`--signed-ticket-secret-path` pour démarrer le service de puzzle ; o startup rejeita
Les paires de clés divergentes sont celles secrètes non valides contre `pow.signed_ticket_public_key_hex`.
`POST /v2/puzzle/mint` Aceita `"signed": true` (et optionnel `"transcript_hash_hex"`) pour
renvoyer un ticket signé Norito avec les octets du ticket brut ; comme réponses
inclure `signed_ticket_b64` et `signed_ticket_fingerprint_hex` pour ajuster le rastrear
empreintes digitales de relecture. Demandes com `signed = true` sao rejeitadas se o signataire secret
n'est pas configuré.

## Playbook de rotation de chaves

1. **Coletar o novo descriptor commit.** Un relais de gouvernance public
   le descripteur ne valide aucun ensemble de répertoires. Copier une chaîne hex para
   `handshake.descriptor_commit_hex` dans la configuration JSON du relais partagé
   com ou service de puzzles.
2. **Réviser les limites de la politique de puzzle.** Confirmer que les valeurs actualisées
   `pow.puzzle.{memory_kib,time_cost,lanes}` est en cours d'alignement sur le plan de sortie.
   Les opérateurs développent une configuration Argon2 déterministe entre les relais
   (minimum 4 Mio de mémoire, 1 <= voies <= 16).
3. **Préparer le redémarrage.** Réinitialiser l'unité système ou le conteneur lorsque
   gouvernance annonce le basculement de rotation. Le service nao prend en charge le rechargement à chaud ;
   Un redémarrage est nécessaire pour récupérer le nouveau descripteur commit.
4. **Valider.** Émettre un ticket via `POST /v2/puzzle/mint` et confirmer que
   `difficulty` et `expires_at` correspondent à une nouvelle politique. O rapport de trempage
   (`docs/source/soranet/reports/pow_resilience.md`) capture des limites de latence
   espérados para referencia. Quando tokens estiverem habilitados, busque
   `/v2/token/config` pour garantir que l'empreinte digitale de l'émetteur est annoncée et a
   le contagem de revogacoes correspondam aos valeurs esperados.

## Procédure de désactivation d'urgence

1. Définissez `pow.puzzle.enabled = false` pour configurer le compartiment du relais.
   Mantenha `pow.required = true` se os tickets hashcash repli précis
   permanecer obrigatorios.
2. Facultativement, forcer l'entrée `pow.emergency` pour refuser les descripteurs
   antigos enquanto o gate Argon2 est disponible hors ligne.
3. Réinitialisez le relais et le service de puzzle pour appliquer un mudanca.
4. Surveillez `soranet_handshake_pow_difficulty` pour garantir qu'il y a des difficultés
   caia para o valor hashcash attendu et vérifié `/v2/puzzle/config` rapport
   `puzzle = null`.

## Surveillance et alertes- **Latence SLO :** Accompagné de `soranet_handshake_latency_seconds` et du support P95
  abaixo de 300 ms. Os offsets do trempage test fornecem dados de calibracao para
  manettes de protection. (docs/source/soranet/reports/pow_resilience.md:1)
- **Pression du quota :** Utiliser les métriques du relais com `soranet_guard_capacity_report.py`
  pour ajuster les temps de recharge de `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`). (docs/source/soranet/relay_audit_pipeline.md:68)
- **Alignement du puzzle :** `soranet_handshake_pow_difficulty` développe le correspondant a
  difficulté de retour par `/v2/puzzle/config`. Configuration relais Divergencia indica
  obsolète ou redémarrez Falho.
- **Préparation du jeton :** Alerte se `/v2/token/config` cair para `enabled = false`
  Inespéré ou les horodatages du rapport `revocation_source` sont périmés. Opérateurs
  développer une rotation ou un fichier Norito de revogacao via CLI lorsqu'un jeton pour
  retiré pour maintenir ce point final précis.
- **Santé du service :** Sondage `/healthz` avec la cadence habituelle de vivacité et d'alerte
  se `/v2/puzzle/mint` renvoie HTTP 500 (indique une incompatibilité des paramètres Argon2 ou
  (falhas de RNG). Les erreurs de frappe de jetons apparaissent comme des réponses HTTP 4xx/5xx à
  `/v2/token/mint` ; Traite falhas repetidas como condicao de paging.

## Conformité et journalisation des audits

Les relais émettent des événements `handshake` structurés qui incluent des motifs d'accélérateur et
durées de recharge. Garantir que le pipeline de conformité le décrit
`docs/source/soranet/relay_audit_pipeline.md` ingira esses logs para que mudancas
da politique de puzzle fiquem auditaveis. Quando o puzzle gate estiver habilitado,
archiver les tickets émis et l'instantané de configuration Norito avec
ticket de déploiement pour les futurs auditoriums. Jetons d'admission émis avant le mois
de manutencao doit être rastreados pelos valeurs `token_id_hex` et insérés non
archivage de revogacao quando expirarem ou forem revogados.