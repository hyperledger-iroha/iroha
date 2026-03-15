---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de serviço de quebra-cabeça
título: Guia de operações do serviço de quebra-cabeças
sidebar_label: Operações de serviço de quebra-cabeças
descrição: Exploração do daemon `soranet-puzzle-service` para ingressos de admissão Argon2/ML-DSA.
---

:::nota Fonte canônica
:::

# Guia de operações do serviço de quebra-cabeças

O daemon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) foi enviado
ingressos baseados em Argon2 que refletem a política `pow.puzzle.*` du
retransmitir e, ao configurar, orquestrar tokens de admissão ML-DSA para os
borda dos relés. Ele expõe os endpoints cinq HTTP:

- `GET /healthz` - sonda de vivacidade.
- `GET /v1/puzzle/config` - retorna os parâmetros de efeito do PoW/quebra-cabeça
  do relé JSON (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` - emite um ticket Argon2; uma opção JSON de corpo
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  exigir um TTL mais tribunal (clamp au window de policy), mentir le ticket a un
  transcrição hash e reenvio de um bilhete assinado pelo relé + a impressão de
  assinatura quando os itens de assinatura são configurados.
- `GET /v1/token/config` - quando `pow.token.enabled = true`, retorne à política
  d'admission-token ativo (impressão digital do emissor, limites TTL/inclinação do relógio, ID do relé,
  e o conjunto de revogação fusionne).
- `POST /v1/token/mint` - emite um token de admissão ML-DSA no hash de currículo
  quatroni; o corpo aceita `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

Os tickets produzidos pelo serviço são verificados no teste de integração
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, que exerce também
aceleradores do relé para cenários DoS volumetriques.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Configurando a emissão de tokens

Defina os campos JSON do relé sob `pow.token.*` (ver
`tools/soranet-relay/deploy/config/relay.entry.json` para um exemplo) afin
ativa os tokens ML-DSA. No mínimo, forneça o documento público do emissor
e uma lista de opções de revogação:

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
Norito JSON de revogação em tempo de execução. Utilize o CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) para emitir e
inspecionar tokens offline, adicionar entradas `token_id_hex` no arquivo de
revogação, e audite as credenciais existentes antes de receber atualizações
em produção.

Passe o segredo do emissor para o serviço de quebra-cabeça por meio da CLI de sinalizadores:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` também está disponível quando o segredo é gerado por um
pipeline fora da banda. O observador do arquivo de revogação guarda `/v1/token/config`
um dia; coordonnez les mises a jour com o comando `soranet-admission-token revoke`
para evitar um estado de revogação retardado.Defina `pow.signed_ticket_public_key_hex` no relé JSON para anunciar
o arquivo público ML-DSA-44 é usado para verificar as assinaturas de tickets PoW; `/v1/puzzle/config`
repete la cle et son empreinte BLAKE3 (`signed_ticket_public_key_fingerprint_hex`) afin
que os clientes possam fixar o verificador. Os ingressos assinados são válidos
contre o ID de retransmissão e as ligações de transcrição e compartilhe o armazenamento de meme de
revogação; os tickets PoW brutos de 74 octetos permanecem válidos quando o verificador
O ticket assinado está configurado. Passe o segredo da assinatura via `--signed-ticket-secret-hex`
ou `--signed-ticket-secret-path` no lançamento do serviço de quebra-cabeça; a desmarcação
rejeite os pares de chaves incoerentes se o segredo não for válido
`pow.signed_ticket_public_key_hex`. `POST /v1/puzzle/mint` aceita `"signed": true`
(e opcional `"transcript_hash_hex"`) para enviar um bilhete assinado Norito en
mais os bytes do ticket bruto; as respostas incluem `signed_ticket_b64` e
`signed_ticket_fingerprint_hex` para reproduzir impressões digitais. Les
solicitações com `signed = true` são rejeitadas se o segredo da assinatura não for permitido
configurar.

## Manual de rotação dos cles

1. **Colecione o novo descritor commit.** Governança pública do relé
   descritor commit no diretório bundle. Copie a cadeia hexadecimal em
   `handshake.descriptor_commit_hex` na configuração do relé JSON compartilhado
   com o serviço de quebra-cabeça.
2. **Verifique os limites do quebra-cabeça da política.** Confirme os valores
   `pow.puzzle.{memory_kib,time_cost,lanes}` perde o alinhamento com o plano
   de lançamento. Os operadores devem monitorar a configuração determinada do Argon2
   entre relés (mínimo 4 MiB de memória, 1 <= pistas <= 16).
3. **Prepare o redemarrage.** Recarregue o sistema unitário ou o contêiner de um
   foi que a governança anunciou a mudança de rotação. O serviço não é suportado
   não recarregar a quente; uma redemarrage é necessária para obter o novo descritor
   cometer.
4. **Validador.** Emita um ticket via `POST /v1/puzzle/mint` e confirme que
   `difficulty` e `expires_at` correspondentes à nova política. O relacionamento
   imersão (`docs/source/soranet/reports/pow_resilience.md`) captura des bornes de
   presenças de latência para referência. Quando os tokens estão ativos, lisez
   `/v1/token/config` para verificar se a impressão digital do emissor anuncia e le
   compte de revogation correspondente aux valeurs atendentes.

## Procedimento de desativação de urgência

1. Defina `pow.puzzle.enabled = false` na configuração do relé compartilhado.
   Gardez `pow.required = true` se os tickets hashcash fallback devem ser restaurados
   obrigatórios.
2. Opcionalmente, imponha as entradas `pow.emergency` para rejeitar os
   descritores obsoletos enquanto a porta Argon2 está offline.
3. Redemarrez a la fois le relay et le puzzle service for applique le
   mudança.
4. Verifique `soranet_handshake_pow_difficulty` para verificar se há dificuldade
   tombe o valor hashcash comparecimento, e valide que `/v1/puzzle/config`
   relatório `puzzle = null`.

## Monitoramento e alertas- **Latency SLO:** Suivez `soranet_handshake_latency_seconds` e gardez le P95
  sous 300 ms. As compensações do teste de imersão fornecem dados de calibração
  para os aceleradores de proteção.【docs/source/soranet/reports/pow_resilience.md:1】
- **Pressão de cota:** Utilize `soranet_guard_capacity_report.py` com arquivos
  relé de métricas para ajustar os cooldowns `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Alinhamento do quebra-cabeça:** `soranet_handshake_pow_difficulty` faça a correspondência
  difícil retorno par `/v1/puzzle/config`. Uma divergência indica uma configuração
  retransmitir obsoleto ou uma taxa de redemarrage.
- **Prontidão do token:** Alerte si `/v1/token/config` chute a `enabled = false`
  de ser desatendido ou se `revocation_source` relatar carimbos de data / hora obsoletos.
  Os operadores devem fazer o tour do arquivo de revogação Norito via CLI
  aquele token foi retirado para fornecer informações precisas sobre o endpoint.
- **Saúde do serviço:** Sonda `/healthz` com a cadência de vida habitual e
  alertar se `/v1/puzzle/mint` enviar respostas HTTP 500 (indicar uma incompatibilidade
  parâmetros Argon2 ou echecs RNG). Erros de cunhagem de token se
  manifesto via respostas HTTP 4xx/5xx em `/v1/token/mint`; trai-los
  echecs repete como uma condição de paginação.

## Registro de conformidade e auditoria

Os relés emitem eventos `handshake` com estruturas que incluem as razões
acelerador e durações de resfriamento. Certifique-se de que o pipeline está em conformidade
descrito em `docs/source/soranet/relay_audit_pipeline.md` inserir esses logs até
que as mudanças no quebra-cabeça da política permanecem auditáveis. Quebra-cabeça Quand la porte
está ativo, arquiva os echantillons de tickets minutos e o instantâneo de
configuração Norito com ticket de implementação para auditorias futuras. Les
tokens de admissão minutos antes das janelas de manutenção doivent etre suivis
com seus valores `token_id_hex` e inseridos no arquivo de revogação de um
foi expirado ou revogado.