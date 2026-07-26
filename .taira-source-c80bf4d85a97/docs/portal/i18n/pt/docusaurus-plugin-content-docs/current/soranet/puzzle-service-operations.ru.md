---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de serviço de quebra-cabeça
título: Руководство по эксплуатации Puzzle Service
sidebar_label: Operações de serviço de quebra-cabeça
description: Daemon de expansão `soranet-puzzle-service` para ingressos de admissão Argon2/ML-DSA.
---

:::nota História Canônica
:::

# Руководство по эксплуатации Puzzle Service

Daemon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) instalado
Bilhetes de admissão apoiados por Argon2, política de которые отражают `pow.puzzle.*` у relé
e, portanto, corretagem de tokens de admissão ML-DSA em relés de borda existentes.
Para definir os endpoints HTTP:

- `GET /healthz` - sonda de vivacidade.
-`GET /v1/puzzle/config` - возвращает эффективные параметры PoW/puzzle,
  Selecione o relé JSON (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` - выпускает bilhete Argon2; corpo JSON opcional
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  запрашивает более короткий TTL (fixado к janela de política), привязывает ticket
  к hash de transcrição e возвращает bilhete assinado por retransmissão + impressão digital de assinatura
  при наличии chaves de assinatura.
- `GET /v1/token/config` - когда `pow.token.enabled = true`, ativado
  política de token de admissão (impressão digital do emissor, limites de TTL/inclinação do relógio, ID de retransmissão,
  e conjunto de revogação mesclado).
- `POST /v1/token/mint` - token de admissão выпускает ML-DSA, связанный с fornecido
  retomar hash; corpo принимает `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

Ingressos, serviço de compra, teste integrado
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, também
упражняет aceleradores de relé no cenário DoS volumétrico.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Настройка выпуска токенов

Selecione o relé JSON para `pow.token.*` (см.
`tools/soranet-relay/deploy/config/relay.entry.json` como exemplo), чтобы
usar tokens ML-DSA. Chave pública do emissor mínima e opcional
lista de revogação:

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

Serviço de quebra-cabeças pode ser usado com segurança e personalização automática
Norito JSON é revogação em tempo de execução. Utilize CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) isso é feito e
testar tokens offline, adicionar entradas `token_id_hex` na página de revogação e
fornecer credenciais de verificação de auditoria para atualizações públicas em produção.

Transfira a chave secreta do emissor para o serviço de quebra-cabeça através dos sinalizadores CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` доступен, когда secret управляется ferramentas fora de banda
gasoduto. Observador de arquivo de revogação держит `/v1/token/config` актуальным;
coordenação de operação com comando `soranet-admission-token revoke`, чтобы
избежать отставания estado de revogação.Definindo `pow.signed_ticket_public_key_hex` no relé JSON, isso é obtido
Chave pública ML-DSA-44 para tíquetes PoW assinados; `/v1/puzzle/config` é
chave возвращает e impressão digital его BLAKE3 (`signed_ticket_public_key_fingerprint_hex`),
чтобы clientes могли pin-ить verificador. Bilhetes assinados são fornecidos pelo ID de retransmissão e
ligações de transcrição e usadas para armazenamento de revogação; Tickets PoW brutos de 74 bytes
остаются валидными при настроенном verificador de bilhetes assinados. Segredo do signatário Передайте
verifique `--signed-ticket-secret-hex` ou `--signed-ticket-secret-path` por conta própria
serviço de quebra-cabeças; старт отклонит несоответствующие pares de chaves, exceto segredo não validado
protив `pow.signed_ticket_public_key_hex`. `POST /v1/puzzle/mint` teste
`"signed": true` (e opcional `"transcript_hash_hex"`)
Ticket assinado codificado em Norito contém bytes brutos de ticket; ответы включают
`signed_ticket_b64` e `signed_ticket_fingerprint_hex` para reproduzir impressões digitais.
Depois de `signed = true` aberto, o segredo do signatário não foi encontrado.

## Playbook ротации ключей

1. **Comprometer o novo descritor de governança.** Descritor de retransmissão pública de governança
   commit no diretório bundle. Escreva uma string hexadecimal em `handshake.descriptor_commit_hex`
   внутри relé JSON конфигурации, которой пользуется serviço de quebra-cabeça.
2. **Proverьте quebra-cabeça de política de limites.** Подтвердите, что обновленные значения
   `pow.puzzle.{memory_kib,time_cost,lanes}` соответствуют plano de lançamento. Operadores
   должны держать Argon2 конфигурацию детерминированной между relés (минимум 4 MiB
   памяти, 1 <= pistas <= 16).
3. **Reiniciar.** Instale a unidade systemd ou o contêiner também, como
   governança объявит transição de rotação. O serviço não pode ser recarregado a quente; para
   Primeiro, o novo descritor commit deve ser reiniciado.
4. **Provalidirуйте.** Выпустите ticket через `POST /v1/puzzle/mint` e подтвердите,
   что `difficulty` e `expires_at` definimos uma nova política. Relatório de imersão
   (`docs/source/soranet/reports/pow_resilience.md`) содержит ожидаемые limites de latência
   para справки. Os tokens de segurança são adquiridos, `/v1/token/config`, usados,
   что impressão digital do emissor anunciada e contagem de revogação соответствуют ожидаемым значениям.

## Processo de desativação de emergência

1. Instale `pow.puzzle.enabled = false` na configuração correta do relé. Hospedagem
   `pow.required = true`, exceto tickets de fallback de hashcash que foram criados.
2. Selecione as entradas `pow.emergency`, чтобы отклонять устаревшие
   descritores пока Argon2 gate offline.
3. Instale o serviço de relé e quebra-cabeça, esta é uma avaliação preliminar.
4. Monitor `soranet_handshake_pow_difficulty`, isso é instalado, este é o problema
   упала ожидаемого hashcash значения, e prover, что `/v1/puzzle/config`
   use `puzzle = null`.

## Monitoramento e alertas- **SLO de latência:** Selecione `soranet_handshake_latency_seconds` e defina P95
  menos de 300 ms. Os deslocamentos de teste de imersão fornecem dados de calibração para aceleradores de proteção.
  【docs/source/soranet/reports/pow_resilience.md:1】
- **Pressão de cota:** Use `soranet_guard_capacity_report.py` com métricas de relé
  para cooldowns `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Alinhamento do quebra-cabeça:** `soranet_handshake_pow_difficulty` должен совпадать с
  dificuldade de `/v1/puzzle/config`. Diversidade usada na configuração de relé obsoleta
  ou não será necessário reiniciar.
- **Prontidão do token:** Alerta, если `/v1/token/config` неожиданно падает до
  `enabled = false` ou `revocation_source` contém carimbos de data/hora obsoletos. Operadores
  должны ротировать Norito arquivo de revogação через CLI при выводе токена, чтобы
  endpoint é definido como.
- **Saúde do serviço:** Verifique `/healthz` em cadência e alerta de atividade ativos,
  если `/v1/puzzle/mint` возвращает HTTP 500 (указывает на Argon2 parâmetro incompatibilidade
  ou falhas de RNG). A cunhagem de tokens é executada como HTTP 4xx/5xx em
  `/v1/token/mint`; повторяющиеся сбои следует считать condição de paginação.

## Conformidade e registro de auditoria

Relés публикуют структурированные `handshake` eventos, motivos de aceleração
e durações de resfriamento. Considere qual pipeline de conformidade está
`docs/source/soranet/relay_audit_pipeline.md` armazena seus logs, sua configuração
política de quebra-cabeça оставались auditáveis. Когда puzzle gate включен, архивируйте
образцы cunhados tickets e instantâneo de configuração Norito вместе с rollout ticket
para auditorias будущих. Tokens de admissão, janelas de manutenção выпущенные перед,
следует отслеживать по `token_id_hex` e добавлять в revogação файл после
истечения или отзыва.