---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de serviço de quebra-cabeça
título: Guia de operações do Puzzle Service
sidebar_label: Serviço de Operações do Puzzle
description: Operação do daemon `soranet-puzzle-service` para ingressos Argon2/ML-DSA.
---

:::nota Fonte canônica
Espelha `docs/source/soranet/puzzle_service_operations.md`. Mantenha ambas as cópias sincronizadas.
:::

# Guia de operações do Puzzle Service

O daemon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) emite
ingressos respaldados por Argon2 que refletem a política `pow.puzzle.*`
do relay e, quando configurado, faz o broker de tokens de admissão ML-DSA em nome
dos relés de borda. Ele expõe cinco endpoints HTTP:

- `GET /healthz` - sonda de vivacidade.
- `GET /v2/puzzle/config` - retorna os parâmetros efetivos de PoW/puzzle extraidos
do JSON do relé (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v2/puzzle/mint` - emite um ticket Argon2; um corpo JSON opcional
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  pede um TTL menor (clamp na janela de política), vincula o ticket a uma transcrição
  hash e retorna um ticket assinado pelo relay + impressão digital da assinatura quando
  as chaves de assinatura estão definidas.
- `GET /v2/token/config` - quando `pow.token.enabled = true`, retorna uma política
  ativação de token de admissão (impressão digital do emissor, limites de TTL/inclinação do relógio, ID do relé
  e o conjunto de revogação mesclado).
- `POST /v2/token/mint` - emite um token de admissão ML-DSA vinculado ao hash de currículo
  fornecido; o corpo aceita `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

Os tickets produzidos pelo serviço são selecionados no teste de integração
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, que também é exercitado
aceleradores retransmitem durante cenários de DoS volumétrico.
(ferramentas/soranet-relay/testes/adaptive_and_puzzle.rs:337)

## Configurar emissão de tokens

Defina os campos JSON do relé em `pow.token.*` (veja
`tools/soranet-relay/deploy/config/relay.entry.json` como exemplo) para habilitar
Tokens ML-DSA. No mínimo, forneca a chave pública do emissor e uma lista de revogação
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
Norito JSON de revogação em tempo de execução. Use o CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) para emitir e
operar tokens offline, anexar entradas `token_id_hex` ao arquivo de revogação
e auditar as credenciais existentes antes de publicar atualizações em produção.

Passe uma chave secreta do emissor para o serviço de quebra-cabeça por meio da CLI de flags:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` também está disponível quando o segredo é gerenciado por um
pipeline de ferramentas fora de banda. O observador do arquivo de revogação mantem
`/v2/token/config` atualizado; coordene atualizações com o comando
`soranet-admission-token revoke` para evitar estado de revogação defasado.Defina `pow.signed_ticket_public_key_hex` no JSON do relé para anunciar a chave
publicação ML-DSA-44 usada para verificar tickets PoW assinados; `/v2/puzzle/config`
reproduza a chave e sua impressão digital BLAKE3 (`signed_ticket_public_key_fingerprint_hex`)
para que os clientes possam corrigir o verificador. Ingressos assinados são validados contra
o relé ID e ligações de transcrição e conjuntamente o mesmo armazenamento de revogação; PoW
tickets brutos de 74 bytes permanecem válidos quando o verificador de tickets assinados
ainda está configurado. Passe o segredo do signatário via `--signed-ticket-secret-hex` ou
`--signed-ticket-secret-path` ao iniciar o serviço de quebra-cabeça; o startup rejeita
pares de chaves divergentes se o segredo não são válidos contra `pow.signed_ticket_public_key_hex`.
`POST /v2/puzzle/mint` aceita `"signed": true` (e opcional `"transcript_hash_hex"`) para
retornar um ticket assinado Norito junto com os bytes do ticket bruto; como respostas
incluem `signed_ticket_b64` e `signed_ticket_fingerprint_hex` para ajudar a rastrear
impressões digitais de repetição. Solicitações com `signed = true` são rejeitadas se o segredo do signatário
não estiver configurado.

## Playbook de rotação de chaves

1. **Coletar o novo descritor commit.** A governança pública o relé
   descritor não confirma nenhum pacote de diretório. Copiar uma string hexadecimal para
   `handshake.descriptor_commit_hex` dentro da configuração JSON do relé compartilhado
   com o serviço de quebra-cabeça.
2. **Revisar limites da política de quebra-cabeça.** Confirme que os valores estão atualizados
   `pow.puzzle.{memory_kib,time_cost,lanes}` alinhados ao plano de liberação.
   Os operadores devem manter a configuração determinística do Argon2 entre relés
   (mínimo 4 MiB de memória, 1 <= pistas <= 16).
3. **Preparar ou reiniciar.** Recarregue a unidade systemd ou o container quando a
   governança anuncia corte de rotação. O serviço não suporta hot-reload;
   um restart é necessário para pegar o novo descritor commit.
4. **Validar.** Emita um ticket via `POST /v2/puzzle/mint` e confirme que
   `difficulty` e `expires_at` incluem a nova política. Ó relatório de imersão
   (`docs/source/soranet/reports/pow_resilience.md`) limites de captura de latência
   esperados para referência. Quando os tokens estiverem habilitados, procure
   `/v2/token/config` para garantir que a impressão digital do emissor anunciada e a
   contagem de revogações correspondeu aos valores esperados.

## Procedimento de desativação de emergência

1. Defina `pow.puzzle.enabled = false` na configuração compartilhada do relé.
   Mantenha `pow.required = true` se os tickets hashcash fallback precisarem
   permanência obrigatória.
2. Opcionalmente, force entradas `pow.emergency` para eliminar descritores
   antigos enquanto o gate Argon2 estiver offline.
3. Reinicie o serviço de relé e o quebra-cabeça para aplicar a mudança.
4. Monitore `soranet_handshake_pow_difficulty` para garantir que a dificuldade
   caia para o valor hashcash esperado e verifique `/v2/puzzle/config` reportando
   `puzzle = null`.

## Monitoramento e alertas- **Latency SLO:** Acompanhe `soranet_handshake_latency_seconds` e mantenha o P95
  abaixo de 300 ms. Os offsets do teste de imersão fornecem dados de calibração para
  aceleradores de guarda. (docs/source/soranet/reports/pow_resilience.md:1)
- **Pressão de cota:** Use métricas de relé `soranet_guard_capacity_report.py` com
  para ajustar cooldowns de `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`). (docs/source/soranet/relay_audit_pipeline.md:68)
- **Alinhamento do quebra-cabeça:** `soranet_handshake_pow_difficulty` deve ser especificado
  dificuldade retornada por `/v2/puzzle/config`. Configuração do relé Divergência indica
  obsoleto ou reinicie falho.
- **Prontidão do token:** Alerta se `/v2/token/config` cair para `enabled = false`
  inesperadamente ou se `revocation_source` reportar timestamps obsoletos. Operadores
  deve girar o arquivo Norito de revogação via CLI quando um token for
  removido para manter esse endpoint preciso.
- **Saúde do serviço:** Sondagem `/healthz` na cadência usual de atividade e alertar
  se `/v2/puzzle/mint` retornar HTTP 500 (indica incompatibilidade de parâmetros Argon2 ou
  falhas de RNG). Erros de criação de token aparecem como respostas HTTP 4xx/5xx em
  `/v2/token/mint`; trate falhas repetidas como condição de paginação.

## Registro de conformidade e auditoria

Relés emitem eventos `handshake` estruturados que incluem motivos de acelerador e
durações de resfriamento. Garanta que o pipeline de conformidade descrito em
`docs/source/soranet/relay_audit_pipeline.md` ingira esses logs para que mudem
da política de quebra-cabeça tornam-se auditáveis. Quando o puzzle gate estiver habilitado,
arquivo de amostras de tickets emitidos e o snapshot de configuração Norito com o
rollout ticket para futuras auditorias. Tokens de admissão emitidos antes de janelas
de manutenção devem ser rastreados pelos valores `token_id_hex` e inseridos no
arquivo de revogacao quando expirarem ou forem revogados.