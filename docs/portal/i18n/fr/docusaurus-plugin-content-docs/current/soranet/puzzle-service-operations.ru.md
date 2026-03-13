---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : puzzle-service-opérations
titre : Recherche pour l'expédition Puzzle Service
sidebar_label : Opérations de service de puzzle
description : Démon d'utilisation `soranet-puzzle-service` pour les billets d'entrée Argon2/ML-DSA.
---

:::note Канонический источник
:::

# Руководство по эксплуатации Puzzle Service

Démon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`)
Billets d'entrée soutenus par Argon2, selon la politique `pow.puzzle.*` du relais
Et, maintenant, vous pouvez échanger des jetons d'admission ML-DSA à partir de plusieurs relais de bord.
Pour préparer les points de terminaison HTTP :

- `GET /healthz` - sonde de vivacité.
- `GET /v2/puzzle/config` - améliore les paramètres d'effet PoW/puzzle,
  Il s'agit du relais JSON (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v2/puzzle/mint` - billet Argon2 ; corps JSON facultatif
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  запрашивает более короткий TTL (fenêtre de politique serrée), привязывает ticket
  к hachage de transcription и возвращает ticket signé par relais + empreinte digitale de signature
  pour la signature des clés.
- `GET /v2/token/config` - avec `pow.token.enabled = true`, votre activité est active
  politique de jeton d'admission (empreinte digitale de l'émetteur, limites TTL/clock-skew, ID de relais,
  и ensemble de révocation fusionné).
- `POST /v2/token/mint` - jeton d'admission ML-DSA, directement fourni
  reprendre le hachage ; corps принимает `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

Billets, service client, vérification lors du test d'intégration
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, cette prise
Utiliser les manettes de relais pour les scénarios DoS volumétriques.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Les jetons de votre choix

Ajoutez le relais JSON au module `pow.token.*` (avec.
`tools/soranet-relay/deploy/config/relay.entry.json` par exemple), que
включить ML-DSA jetons. Prévoir de manière minimale la clé publique de l'émetteur et facultatif
liste de révocation :

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

Le service de puzzle utilise généralement ces fonctions et fonctions automatiques
Norito JSON a été révoqué au moment de l'exécution. Utiliser la CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) que vous pouvez télécharger et
prouver les jetons hors ligne, ajouter les entrées `token_id_hex` dans le fichier de révocation et
fournir des informations d'identification d'audit pour la publication des mises à jour en production.

Veuillez sélectionner la clé secrète de l'émetteur dans le service de puzzle à partir des indicateurs CLI :

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` installé, le secret de la mise en place d'outils hors bande
canalisation. Observateur de fichiers de révocation держит `/v2/token/config` актуальным;
coordonner la maintenance avec la commande `soranet-admission-token revoke`, pour vous
избежать отставания état de révocation.Installez `pow.signed_ticket_public_key_hex` dans le relais JSON pour le faire fonctionner
Clé publique ML-DSA-44 pour les tickets PoW signés ; `/v2/puzzle/config` ici
возвращает key и его BLAKE3 empreinte digitale (`signed_ticket_public_key_fingerprint_hex`),
Les clients peuvent vérifier les broches. Billets signés vérifiés par identification de relais et
liaisons de transcription et utilisation du magasin de révocation ; tickets PoW bruts de 74 octets
остаются валидными при настроенном vérificateur de billets signés. Передайте secret du signataire
Pour `--signed-ticket-secret-hex` ou `--signed-ticket-secret-path` à l'achat
service de casse-tête; commencer à ouvrir les paires de clés manquantes, si le secret n'est pas validé
pour `pow.signed_ticket_public_key_hex`. `POST /v2/puzzle/mint` принимает
`"signed": true` (et `"transcript_hash_hex"` en option) à utiliser
Le ticket signé codé en Norito contient des octets bruts du ticket ; ответы включают
`signed_ticket_b64` et `signed_ticket_fingerprint_hex` pour la relecture des empreintes digitales.
La procédure `signed = true` est ouverte, si le secret du signataire n'est pas découvert.

## Playbook rotation des clés

1. **Соберите новый descriptor commit.** Gouvernance публикует relay descriptor
   commit dans un ensemble de répertoires. Copier la chaîne hexadécimale dans `handshake.descriptor_commit_hex`
   Il s'agit d'un relais de configuration JSON, qui est un service de puzzle complet.
2. **Résolvez le casse-tête de la politique des limites.** Résolvez les problèmes les plus récents
   `pow.puzzle.{memory_kib,time_cost,lanes}` plan de sortie proposé. Opérateurs
   il est possible de configurer les relais Argon2 de manière à détecter les relais (min. 4 Mio
   памяти, 1 <= voies <= 16).
3. **Démarrez le redémarrage.** Démarrez l'unité système ou le conteneur après cela, comme
   la gouvernance объявит rotation bascule. Le service ne prend pas en charge le rechargement à chaud ; pour
   применения нового descripteur commit требуется redémarrage.
4. **Провалидируйте.** Выпустите ticket через `POST /v2/puzzle/mint` и подтвердите,
   C'est `difficulty` et `expires_at` qui correspondent à une nouvelle politique. Rapport de trempage
   (`docs/source/soranet/reports/pow_resilience.md`) permet d'éliminer les limites de latence
   для справки. Alors jetons un coup d'oeil, запросите `/v2/token/config`, чтобы убедиться,
   Les empreintes digitales de l'émetteur annoncées et le nombre de révocations sont disponibles.

## Procédure de désactivation d'urgence

1. Installez `pow.puzzle.enabled = false` dans les configurations de relais correspondantes. Оставьте
   `pow.required = true`, si les tickets de secours hashcash sont déjà disponibles.
2. Saisissez éventuellement les entrées `pow.emergency` pour ouvrir l'usine.
   descripteurs de la porte Argon2 hors ligne.
3. Utilisez le service de relais et de puzzle pour effectuer une analyse.
4. Surveillez le `soranet_handshake_pow_difficulty` pour savoir ce qui se passe.
   J'ai ouvert la session hashcash et j'ai vérifié qu'il s'agissait de `/v2/puzzle/config`.
   сообщает `puzzle = null`.

## Surveillance et alerte- **SLO de latence :** Remplacez le `soranet_handshake_latency_seconds` et le P95
  ou 300 ms. Le test de trempage compense les données d'étalonnage des manettes de protection.
  【docs/source/soranet/reports/pow_resilience.md:1】
- **Pression du quota :** Utilisez `soranet_guard_capacity_report.py` avec les métriques de relais
  pour les temps de recharge `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Alignement du puzzle :** `soranet_handshake_pow_difficulty` должен совпадать с
  difficulté из `/v2/puzzle/config`. Diversification de la configuration de relais obsolète
  ou un redémarrage imminent.
- **Préparation au jeton :** Alerte, si `/v2/token/config` n'est pas disponible
  `enabled = false` ou `revocation_source` utilisent des horodatages obsolètes. Opérateurs
  Vous devez faire pivoter le fichier de révocation Norito à partir de la CLI pour que vous puissiez le faire.
  le point final est défini.
- **Santé du service :** Vérifiez `/healthz` dans la cadence de vivacité et l'alerte,
  Si `/v2/puzzle/mint` utilise HTTP 500 (incompatibilité des paramètres Argon2
  ou échecs RNG). Les méthodes de création de jetons sont compatibles avec HTTP 4xx/5xx
  `/v2/token/mint` ; повторяющиеся сбои следует считать condition de pagination.

## Conformité et journalisation d'audit

Les relais publient les événements structurés `handshake`, ainsi que les raisons de l'accélérateur.
et durées de recharge. Убедитесь, что pipeline de conformité из
`docs/source/soranet/relay_audit_pipeline.md` contient ces journaux, pour l'analyse
la politique de puzzle оставались auditables. Когда puzzle gate включен, архивируйте
образцы tickets émis et instantané de configuration Norito pour le ticket de déploiement
для будущих audits. Jetons d'admission, выпущенные перед fenêtres de maintenance,
veuillez procéder à la révocation via `token_id_hex` et à la révocation ultérieure
истечения или отзыва.