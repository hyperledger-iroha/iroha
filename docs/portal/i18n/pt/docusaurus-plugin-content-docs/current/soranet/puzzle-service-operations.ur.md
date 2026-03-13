---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de serviço de quebra-cabeça
título: Guia de operações do serviço Puzzle
sidebar_label: Operações de serviço de quebra-cabeça
descrição: Bilhetes de admissão Argon2/ML-DSA کے لئے `soranet-puzzle-service` daemon کی آپریشنز۔
---

:::nota Fonte Canônica
`docs/source/soranet/puzzle_service_operations.md` کی عکاسی کرتا ہے۔ جب تک پرانا conjunto de documentação retirado نہ ہو، دونوں ورژنز sincronização رکھیں۔
:::

# Guia de operações de serviço de quebra-cabeça

Daemon `tools/soranet-puzzle-service/` e `soranet-puzzle-service`
Bilhetes de admissão apoiados por Argon2 جاری کرتا ہے جو relé کی Política `pow.puzzle.*`
کو espelho کرتے ہیں، اور جب configurar ہو تو relés de borda کی جانب سے ML-DSA
corretor de tokens de admissão کرتا ہے۔ Os endpoints HTTP expõem o seguinte:

- `GET /healthz` - sonda de vivacidade.
- `GET /v2/puzzle/config` - relé JSON (`handshake.descriptor_commit_hex`, `pow.*`) Preto
  اٹھائے گئے موثر Parâmetros PoW/puzzle واپس کرتا ہے۔
- `POST /v2/puzzle/mint` - Bilhete Argon2 mint کرتا ہے؛ corpo JSON opcional
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  کم TTL کی درخواست کرتا ہے (janela de política تک clamp), ticket کو transcrição hash
  سے bind کرتا ہے، اور chaves de assinatura configuradas ہوں تو ticket assinado por retransmissão +
  impressão digital de assinatura واپس کرتا ہے۔
- `GET /v2/token/config` - جب `pow.token.enabled = true` ہو تو token de admissão ativo
  política واپس کرتا ہے (impressão digital do emissor, limites TTL/inclinação do relógio, ID do relé, اور
  conjunto de revogação mesclado).
- `POST /v2/token/mint` - Token de admissão ML-DSA mint کرتا ہے جو hash de currículo fornecido
  سے vinculado ہوتا ہے؛ corpo da solicitação `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`
  قبول کرتا ہے۔

Serviço کے بنائے گئے tickets کو teste de integração
`volumetric_dos_soak_preserves_puzzle_and_latency_slo` verificação de verificação
cenários DoS volumétricos کے دوران aceleradores de relé بھی exercício کرتا ہے۔
【tools/soranet-relay/testes/adaptive_and_puzzle.rs:337】

## Configuração de emissão de token کرنا

`pow.token.*` کے تحت retransmitir campos JSON definidos کریں (مثال کے لئے
`tools/soranet-relay/deploy/config/relay.entry.json` دیکھیں) تاکہ tokens ML-DSA
ativar ہوں۔ کم از کم chave pública do emissor e lista de revogação opcional فراہم کریں:

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

Serviço de quebra-cabeça انہی valores کو reutilização کرتا ہے اور tempo de execução میں Norito Revogação JSON
فائل کو خودکار طور پر recarregar کرتا ہے۔ CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) Configuração de cartão de crédito
tokens offline cunhar/inspecionar ہوں, revogação فائل میں entradas `token_id_hex` anexar ہوں،
Para atualizações de produção ou auditoria de credenciais

Chave secreta do emissor کو sinalizadores CLI کے ذریعے serviço de quebra-cabeça میں پاس کریں:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` بھی دستیاب ہے جب pipeline secreto de ferramentas fora de banda کے ذریعے gerenciar ہو۔
Observador de arquivo de revogação `/v2/token/config` کو atual رکھتا ہے؛ atualizações
`soranet-admission-token revoke` کمانڈ کے ساتھ coordenada کریں تاکہ estado de revogação
atrasoRelé JSON میں `pow.signed_ticket_public_key_hex` set کریں تاکہ tickets PoW assinados
verificar کرنے کے لئے anúncio de chave pública ML-DSA-44 ہو؛ Chave `/v2/puzzle/config` یہ
A impressão digital BLAKE3 (`signed_ticket_public_key_fingerprint_hex`) echo کرتا ہے تاکہ
Pin do verificador de clientes کر سکیں۔ ID de retransmissão de tickets assinados e ligações de transcrição
validar ہوتے ہیں اور اسی armazenamento de revogação کو compartilhar کرتے ہیں؛ Tickets PoW brutos de 74 bytes
verificador de bilhete assinado configurado ہونے پر بھی válido رہتے ہیں۔ Segredo do signatário کو
`--signed-ticket-secret-hex` یا `--signed-ticket-secret-path` کے ذریعے lançamento do serviço
پر passar کریں؛ pares de chaves incompatíveis de inicialização rejeitam کرتا ہے اگر segredo
`pow.signed_ticket_public_key_hex` کے خلاف validar نہ ہو۔ `POST /v2/puzzle/mint`
`"signed": true` (opcional `"transcript_hash_hex"`) é um código com codificação Norito
bytes brutos do ticket assinado کے ساتھ واپس ہو؛ respostas میں `signed_ticket_b64`
اور `signed_ticket_fingerprint_hex` شامل ہوتے ہیں تاکہ reproduzir faixa de impressões digitais ہوں۔
` signed = true` والی solicitações rejeitadas ہوتی ہیں اگر segredo do signatário configurado نہ ہو۔

## Manual de rotação de chaves

1. **نیا descritor commit جمع کریں۔** Pacote de diretório de governança میں relé
   descritor commit publicar کرتی ہے۔ String hexadecimal e configuração JSON de retransmissão
   `handshake.descriptor_commit_hex` میں cópia کریں جو serviço de quebra-cabeça کے ساتھ compartilhado ہے۔
2. **Revisão dos limites da política do quebra-cabeça کریں۔** تصدیق کریں کہ atualizado
   Plano de liberação de valores `pow.puzzle.{memory_kib,time_cost,lanes}` کے مطابق ہیں۔ Operadores کو
   Relés de configuração Argon2 são determinísticos رکھنا چاہئے (کم از کم 4 MiB de memória،
   1 <= pistas <= 16).
3. **Estágio de reinicialização کریں۔** Anúncio de transição de rotação de governança کرے تو unidade systemd یا
   recarga de contêiner کریں۔ Serviço میں hot-reload نہیں ہے؛ نیا descritor commit لینے کے لئے
   reiniciar
4. **Validar کریں۔** `POST /v2/puzzle/mint` کے ذریعے emissão de bilhete کریں اور تصدیق کریں کہ
   `difficulty` e `expires_at` Política de correspondência ہوں۔ Relatório de imersão
   (`docs/source/soranet/reports/pow_resilience.md`) referência کے لئے limites de latência esperados
   capturar کرتا ہے۔ Tokens permitem ہوں تو `/v2/token/config` buscar کریں تاکہ emissor anunciado
   impressão digital اور contagem de revogação valores esperados سے correspondência ہوں۔

## Procedimento de desativação de emergência

1. Configuração de relé compartilhado میں `pow.puzzle.enabled = false` set کریں۔
   `pow.required = true` رکھیں اگر hashcash fallback tickets لازمی رہنے چاہئیں۔
2. Entradas opcionais طور پر `pow.emergency` impõem کریں تاکہ Argon2 gate offline ہونے پر
   descritores obsoletos rejeitam ہوں۔
3. Retransmissão ou serviço de quebra-cabeça دونوں reiniciar کریں تاکہ تبدیلی aplicar ہو۔
4. Monitor `soranet_handshake_pow_difficulty` کریں تاکہ dificuldade valor de hashcash esperado
   تک drop ہو، اور verificar کریں کہ `/v2/puzzle/config` `puzzle = null` رپورٹ کرے۔

## Monitoramento e alertas- **SLO de latência:** Faixa `soranet_handshake_latency_seconds` کریں اور P95 کو 300 ms سے نیچے رکھیں۔
  Teste de absorção de compensação de aceleradores de proteção کے لئے dados de calibração فراہم کرتے ہیں۔
  【docs/source/soranet/reports/pow_resilience.md:1】
- **Pressão de cota:** `soranet_guard_capacity_report.py` کو métricas de relé کے ساتھ استعمال کریں تاکہ
  Cooldowns `pow.quotas` (`soranet_abuse_remote_cooldowns`, `soranet_handshake_throttled_remote_quota_total`) ajuste ہوں۔
  【docs/source/soranet/relay_audit_pipeline.md:68】
- **Alinhamento do quebra-cabeça:** `soranet_handshake_pow_difficulty` کو `/v2/puzzle/config` سے واپس ہونے والی
  dificuldade کے ساتھ correspondência ہونا چاہئے۔ Configuração de relé obsoleta de divergência یا falha na reinicialização کی نشاندہی ہے۔
- **Prontidão do token:** اگر `/v2/token/config` غیر متوقع طور پر `enabled = false` ہو جائے یا
  `revocation_source` timestamps obsoletos رپورٹ کرے تو alerta کریں۔ Operadores کو CLI کے ذریعے Norito
  rotação do arquivo de revogação کرنا چاہئے جب کوئی token retirar ہو تاکہ یہ endpoint درست رہے۔
- **Saúde do serviço:** `/healthz` کو معمول کی cadência de atividade پر sonda کریں اور alerta کریں اگر
  `/v2/puzzle/mint` HTTP 500 respostas دے (incompatibilidade de parâmetro Argon2 یا falhas RNG کی نشاندہی).
  Erros de cunhagem de token `/v2/token/mint` em respostas HTTP 4xx/5xx کے ذریعے نظر آتے ہیں؛ falhas repetidas
  کو condição de paginação سمجھیں۔

## Conformidade e registro de auditoria

Os eventos `handshake` estruturados dos relés emitem کرتے ہیں جن میں motivos de aceleração e durações de resfriamento شامل ہوتے ہیں۔
یقینی بنائیں کہ `docs/source/soranet/relay_audit_pipeline.md` میں بیان کردہ pipeline de conformidade e logs کو ingerir کرے
تاکہ mudanças na política de quebra-cabeças auditáveis رہیں۔ Ativação do portão do quebra-cabeça e amostras de ingressos cunhados e configuração Norito
instantâneo کو ticket de lançamento کے ساتھ arquivo کریں تاکہ مستقبل کے auditorias کے لئے دستیاب ہوں۔ Janelas de manutenção
mint کئے گئے tokens de admissão کو ان کے `token_id_hex` valores کے ساتھ rastrear کیا جانا چاہئے اور expirar یا revogar ہونے
پر arquivo de revogação میں inserir کیا جانا چاہئے۔