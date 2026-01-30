---
lang: es
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: puzzle-service-operations
title: Guia de operacoes do Puzzle Service
sidebar_label: Ops do Puzzle Service
description: Operacao do daemon `soranet-puzzle-service` para admission tickets Argon2/ML-DSA.
---

:::note Fonte canonica
Espelha `docs/source/soranet/puzzle_service_operations.md`. Mantenha ambas as copias sincronizadas.
:::

# Guia de operacoes do Puzzle Service

O daemon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) emite
admission tickets respaldados por Argon2 que refletem a policy `pow.puzzle.*`
do relay e, quando configurado, faz broker de ML-DSA admission tokens em nome
dos edge relays. Ele expoe cinco endpoints HTTP:

- `GET /healthz` - liveness probe.
- `GET /v1/puzzle/config` - retorna os parametros efetivos de PoW/puzzle extraidos
do JSON do relay (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` - emite um ticket Argon2; um body JSON opcional
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  pede um TTL menor (clamp na janela de policy), vincula o ticket a um transcript
  hash e retorna um ticket assinado pelo relay + fingerprint da assinatura quando
  as chaves de assinatura estao configuradas.
- `GET /v1/token/config` - quando `pow.token.enabled = true`, retorna a policy
  ativa de admission-token (issuer fingerprint, limites de TTL/clock-skew, relay ID
  e o revocation set mesclado).
- `POST /v1/token/mint` - emite um ML-DSA admission token vinculado ao resume hash
  fornecido; o body aceita `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

Os tickets produzidos pelo servico sao verificados no teste de integracao
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, que tambem exercita os
throttles do relay durante cenarios de DoS volumetrico.
(tools/soranet-relay/tests/adaptive_and_puzzle.rs:337)

## Configurar emissao de tokens

Defina os campos JSON do relay em `pow.token.*` (veja
`tools/soranet-relay/deploy/config/relay.entry.json` como exemplo) para habilitar
ML-DSA tokens. No minimo, forneca a chave publica do issuer e uma revocation list
opcional:

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

O puzzle service reutiliza esses valores e recarrega automaticamente o arquivo
Norito JSON de revogacao em runtime. Use o CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) para emitir e
inspecionar tokens offline, anexar entradas `token_id_hex` ao arquivo de revogacao
e auditar credenciais existentes antes de publicar updates em producao.

Passe a issuer secret key para o puzzle service via flags CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` tambem esta disponivel quando o secret e gerenciado por um
pipeline de tooling fora de banda. O watcher do arquivo de revogacao mantem
`/v1/token/config` atualizado; coordene updates com o comando
`soranet-admission-token revoke` para evitar estado de revogacao defasado.

Defina `pow.signed_ticket_public_key_hex` no JSON do relay para anunciar a chave
publica ML-DSA-44 usada para verificar PoW tickets assinados; `/v1/puzzle/config`
reproduz a chave e seu fingerprint BLAKE3 (`signed_ticket_public_key_fingerprint_hex`)
para que clients possam fixar o verifier. Tickets assinados sao validados contra
o relay ID e transcript bindings e compartilham o mesmo revocation store; PoW
tickets brutos de 74 bytes permanecem validos quando o signed-ticket verifier
ainda esta configurado. Passe o signer secret via `--signed-ticket-secret-hex` ou
`--signed-ticket-secret-path` ao iniciar o puzzle service; o startup rejeita
keypairs divergentes se o secret nao valida contra `pow.signed_ticket_public_key_hex`.
`POST /v1/puzzle/mint` aceita `"signed": true` (e opcional `"transcript_hash_hex"`) para
retornar um signed ticket Norito junto com os bytes do ticket bruto; as respostas
incluem `signed_ticket_b64` e `signed_ticket_fingerprint_hex` para ajudar a rastrear
fingerprints de replay. Requests com `signed = true` sao rejeitadas se o signer secret
nao estiver configurado.

## Playbook de rotacao de chaves

1. **Coletar o novo descriptor commit.** A governance publica o relay
   descriptor commit no directory bundle. Copie a string hex para
   `handshake.descriptor_commit_hex` dentro da configuracao JSON do relay compartilhada
   com o puzzle service.
2. **Revisar bounds da policy de puzzle.** Confirme que os valores atualizados
   `pow.puzzle.{memory_kib,time_cost,lanes}` estejam alinhados ao plano de release.
   Operadores devem manter a configuracao Argon2 deterministica entre relays
   (minimo 4 MiB de memoria, 1 <= lanes <= 16).
3. **Preparar o restart.** Recarregue a unidade systemd ou o container quando a
   governance anunciar o cutover de rotacao. O servico nao suporta hot-reload;
   um restart e necessario para pegar o novo descriptor commit.
4. **Validar.** Emita um ticket via `POST /v1/puzzle/mint` e confirme que
   `difficulty` e `expires_at` correspondem a nova policy. O soak report
   (`docs/source/soranet/reports/pow_resilience.md`) captura limites de latencia
   esperados para referencia. Quando tokens estiverem habilitados, busque
   `/v1/token/config` para garantir que o issuer fingerprint anunciado e a
   contagem de revogacoes correspondam aos valores esperados.

## Procedimento de desativacao de emergencia

1. Defina `pow.puzzle.enabled = false` na configuracao compartilhada do relay.
   Mantenha `pow.required = true` se os tickets hashcash fallback precisarem
   permanecer obrigatorios.
2. Opcionalmente, force entradas `pow.emergency` para rejeitar descriptors
   antigos enquanto o gate Argon2 estiver offline.
3. Reinicie o relay e o puzzle service para aplicar a mudanca.
4. Monitore `soranet_handshake_pow_difficulty` para garantir que a dificuldade
   caia para o valor hashcash esperado e verifique `/v1/puzzle/config` reportando
   `puzzle = null`.

## Monitoramento e alertas

- **Latency SLO:** Acompanhe `soranet_handshake_latency_seconds` e mantenha o P95
  abaixo de 300 ms. Os offsets do soak test fornecem dados de calibracao para
  guard throttles. (docs/source/soranet/reports/pow_resilience.md:1)
- **Quota pressure:** Use `soranet_guard_capacity_report.py` com relay metrics
  para ajustar cooldowns de `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`). (docs/source/soranet/relay_audit_pipeline.md:68)
- **Puzzle alignment:** `soranet_handshake_pow_difficulty` deve corresponder a
  dificuldade retornada por `/v1/puzzle/config`. Divergencia indica relay config
  stale ou restart falho.
- **Token readiness:** Alerta se `/v1/token/config` cair para `enabled = false`
  inesperadamente ou se `revocation_source` reportar timestamps stale. Operadores
  devem rotacionar o arquivo Norito de revogacao via CLI quando um token for
  retirado para manter esse endpoint preciso.
- **Service health:** Probing `/healthz` na cadencia usual de liveness e alertar
  se `/v1/puzzle/mint` retornar HTTP 500 (indica mismatch de parametros Argon2 ou
  falhas de RNG). Erros de token minting aparecem como respostas HTTP 4xx/5xx em
  `/v1/token/mint`; trate falhas repetidas como condicao de paging.

## Compliance e audit logging

Relays emitem eventos `handshake` estruturados que incluem motivos de throttle e
cooldown durations. Garanta que o pipeline de compliance descrito em
`docs/source/soranet/relay_audit_pipeline.md` ingira esses logs para que mudancas
da policy de puzzle fiquem auditaveis. Quando o puzzle gate estiver habilitado,
arquive amostras de tickets emitidos e o snapshot de configuracao Norito com o
rollout ticket para futuras auditorias. Admission tokens emitidos antes de janelas
de manutencao devem ser rastreados pelos valores `token_id_hex` e inseridos no
arquivo de revogacao quando expirarem ou forem revogados.
