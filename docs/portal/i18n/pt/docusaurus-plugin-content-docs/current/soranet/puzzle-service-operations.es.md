---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de serviço de quebra-cabeça
título: Guia de operações de serviço de quebra-cabeças
sidebar_label: Operações de serviço de quebra-cabeças
description: Operação do daemon `soranet-puzzle-service` para tickets de admissão Argon2/ML-DSA.
---

:::nota Fonte canônica
Reflexo `docs/source/soranet/puzzle_service_operations.md`. Mantenha ambas as versões sincronizadas até que os documentos herdados sejam retirados.
:::

# Guia de operações de serviço de quebra-cabeças

O daemon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) emite
ingressos respaldados por Argon2 que refletem a política `pow.puzzle.*`
del relé e, quando estiver configurado, tokens intermediários de admissão ML-DSA en
nome da borda do relé. Expor cinco endpoints HTTP:

- `GET /healthz` - sonda de vivacidade.
- `GET /v1/puzzle/config` - devolve os parâmetros efetivos de PoW/puzzle
  feitos do JSON do relé (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` - abre um ticket Argon2; um corpo JSON opcional
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  solicitar um TTL mas curto (fixar na janela da política), no ticket para um
  transcrição hash e devolve um ticket firmado pelo relé + la huella de
  la firma quando hay claves de firmado ajustadas.
- `GET /v1/token/config` - quando `pow.token.enabled = true`, devolve la
  política de ativação de token de admissão (impressão digital do emissor, limites de TTL/inclinação do relógio,
  relé ID e o conjunto de revogação combinado).
- `POST /v1/token/mint` - contém um token de admissão ML-DSA vinculado ao currículo
  hash fornecido; o corpo aceita `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

Os ingressos produzidos pelo serviço são verificados na tentativa de integração
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, que também é emitido
os aceleradores do relé durante cenários de DoS volumetrico.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Configurar a emissão de tokens

Configure os campos JSON do relé abaixo `pow.token.*` (ver
`tools/soranet-relay/deploy/config/relay.entry.json` como exemplo) para habilitar
os tokens ML-DSA. Como mínimo, prove a chave pública do emissor e uma lista
opcional de revogação:

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

O serviço de quebra-cabeça reutiliza esses valores e recarrega automaticamente o arquivo
Norito JSON de revogações em tempo de execução. Use o CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) para descobrir e
inspecionar tokens offline, anexar entradas `token_id_hex` ao arquivo de
revogações e auditar credenciais existentes antes de publicar atualizações a
produção.

Passe a chave secreta do emissor para o serviço de quebra-cabeça via flags CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` também está disponível quando o segredo é gerenciado por
um pipeline de ferramentas fora de banda. O observador do arquivo de revogação
mantenedor `/v1/token/config` atualizado; coordenação de atualizações com o comando
`soranet-admission-token revoke` para evitar desfases no estado de revogação.Configure `pow.signed_ticket_public_key_hex` no JSON do relé para anunciar
a chave pública ML-DSA-44 usada para verificar tickets PoW firmados; `/v1/puzzle/config`
réplica da chave e da huella BLAKE3 (`signed_ticket_public_key_fingerprint_hex`)
para que os clientes possam fixar o selecionador. Os ingressos firmados são válidos
contra o ID do relé e as ligações de transcrição e compara o mesmo armazenamento de revogação;
os tickets PoW brutos de 74 bytes continuam sendo válidos quando o selecionador de
O ticket assinado está configurado. Passe o segredo do firmante via `--signed-ticket-secret-hex`
o `--signed-ticket-secret-path` para iniciar o serviço de quebra-cabeças; el arranque rechaza
pares de chaves que não coincidem se o segredo não for válido contra `pow.signed_ticket_public_key_hex`.
`POST /v1/puzzle/mint` aceita `"signed": true` (e opcional `"transcript_hash_hex"`) para
devolver um ticket firmado Norito junto com os bytes do ticket bruto; las
respostas incluem `signed_ticket_b64` e `signed_ticket_fingerprint_hex` para ajudar
a rastrear impressões digitais de repetição. Las solicitudes con `signed = true` se rechazan
se o segredo do firmante não estiver configurado.

## Manual de rotação de chaves

1. **Recolha do novo descritor commit.** Governança pública do relé
   descritor commit no pacote do diretório. Copiar a cadeia hexadecimal em
   `handshake.descriptor_commit_hex` dentro do JSON de configuração do relé
   compartilhado com o serviço de quebra-cabeça.
2. **Revisão dos limites da política do quebra-cabeça.** Confirma que os valores
   `pow.puzzle.{memory_kib,time_cost,lanes}` atualizado se alinhado com o plano
   de lançamento. Os operadores devem manter a configuração do determinista Argon2
   entre relés (mínimo 4 MiB de memória, 1 <= pistas <= 16).
3. **Preparar o reinício.** Recarregue a unidade do sistema ou o conteúdo uma vez
   que a governança anuncie a mudança de rotação. O serviço não suporta hot-reload;
   é necessário reiniciar para executar o novo descritor commit.
4. **Validar.** Emita um ticket via `POST /v1/puzzle/mint` e confirme que los
   Os valores `difficulty` e `expires_at` coincidem com a nova política. O relatório
   de imersão (`docs/source/soranet/reports/pow_resilience.md`) captura de limites
   de latência esperada como referência. Quando os tokens estão habilitados,
   consulte `/v1/token/config` para garantir que a impressão digital do emissor foi anunciada
   e o conteúdo de revogações coincide com os valores esperados.

## Procedimento de desativação de emergência

1. Configure `pow.puzzle.enabled = false` na configuração compartilhada do relé.
   Mantenha `pow.required = true` se os tickets hashcash fallback devem seguir
   sendo obrigatório.
2. Opcionalmente aplica entradas `pow.emergency` para recuperar descritores
   obsoletos enquanto a porta Argon2 está offline.
3. Reinicie o serviço de relé e quebra-cabeça para aplicar a mudança.
4. Monitore `soranet_handshake_pow_difficulty` para garantir que a dificuldade
   cae al valor hashcash esperado, e verifica que `/v1/puzzle/config` reporte
   `puzzle = null`.

## Monitoramento e alertas- **Latency SLO:** Rastrea `soranet_handshake_latency_seconds` e mantém o P95
  por baixo de 300 ms. As compensações do teste de imersão provaram dados de calibração
  para aceleradores de guarda.【docs/source/soranet/reports/pow_resilience.md:1】
- **Pressão de cota:** Usa `soranet_guard_capacity_report.py` com métricas de relé
  para ajustar cooldowns de `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Alinhamento do quebra-cabeça:** `soranet_handshake_pow_difficulty` deve coincidir com la
  dificultado devido a `/v1/puzzle/config`. A divergência indica configuração
  O relé está parado ou um reinício falhado.
- **Prontidão do token:** Alerta si `/v1/token/config` cae a `enabled = false`
  inesperadamente ou se `revocation_source` relata timestamps obsoletos. Os operadores
  deve girar o arquivo de revogações Norito via CLI quando um token for retirado
  para manter este endpoint preciso.
- **Saúde do serviço:** Sondea `/healthz` na cadência habitual de vivacidade e alerta
  si `/v1/puzzle/mint` deve responder HTTP 500 (indica incompatibilidade de parâmetros
  Argon2 ou falhas de RNG). Erros de cunhagem de token aparecem como respostas
  HTTP 4xx/5xx em `/v1/token/mint`; trata-se de falhas repetidas como condição de paginação.

## Registro de conformidade e auditoria

Os relés emitem eventos `handshake` estruturados que incluem razões de
acelerador e durações de resfriamento. Certifique-se de que o pipeline de conformidade está descrito
em `docs/source/soranet/relay_audit_pipeline.md` ingeste esses logs para que eles
mudanças de política do quebra-cabeça seguem sendo auditáveis. Quando a porta do quebra-cabeça
esta habilitada, arquiva mostras de tickets emitidos e o snapshot de configuração
Norito com o ticket de rollout para futuras auditorias. Los tokens de admissão
emitidos antes de vendas de manutenção devem ser rastreados com seus valores
`token_id_hex` e insira no arquivo de revogações uma vez que expire ou
sean revogados.