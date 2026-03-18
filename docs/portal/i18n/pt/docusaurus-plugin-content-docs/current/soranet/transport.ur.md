---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/transport.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: transporte
título: Visão geral do transporte SoraNet
sidebar_label: Visão geral do transporte
descrição: Sobreposição de anonimato SoraNet کے لئے aperto de mão, rotação de sal, orientação de capacidade اور۔
---

:::nota Fonte Canônica
یہ صفحہ `docs/source/soranet/spec.md` میں Especificação de transporte SNNet-1 کی عکاسی کرتا ہے۔ جب تک پرانا conjunto de documentação retirar نہ ہو، دونوں کاپیاں sincronização رکھیں۔
:::

SoraNet e sobreposição de anonimato ہے جو SoraFS کے buscas de intervalo, Norito streaming RPC, اور مستقبل کے Nexus faixas de dados کو سپورٹ کرتا ہے۔ programa de transporte (itens de roteiro **SNNet-1**, **SNNet-1a**, e **SNNet-1b**) نے handshake determinístico, negociação de capacidade pós-quântica (PQ), e plano de rotação de sal متعین کیا تاکہ ہر relé, cliente, e gateway ایک ہی segurança postura observar کرے۔

## Metas e modelo de rede

- QUIC v1 tem circuitos de três saltos (entrada -> meio -> saída) بنانا تاکہ pares abusivos کبھی Torii تک براہ راست نہ پہنچیں۔
- QUIC/TLS کے اوپر Noise XX *hybrid* handshake (Curve25519 + Kyber768) camadas تاکہ chaves de sessão Transcrição TLS سے bind ہوں۔
- capacidade TLVs لازمی کریں جو PQ KEM/suporte de assinatura, função de retransmissão, e versão do protocolo anunciada کریں؛ tipos desconhecidos کو GREASE کریں تاکہ extensões futuras implementáveis ​​رہیں۔
- sais de conteúdo cego کو روزانہ girar کریں اور guarda relés کو 30 دن pin کریں تاکہ diretório churn clientes کو deanonimizar نہ کر سکے۔
- células کو 1024 B پر fixo رکھیں, preenchimento/células fictícias injetadas کریں, اور exportação de telemetria determinística کریں تاکہ tentativas de downgrade جلد پکڑی جائیں۔

## Pipeline de handshake (SNNet-1a)

1. **Envelope QUIC/TLS** - clientes QUIC v1 پر relés سے conectar کرتے ہیں اور certificados Ed25519 کے ساتھ TLS 1.3 handshake completo کرتے ہیں جو governança CA نے sinal کئے ہوتے ہیں۔ Exportador TLS (`tls-exporter("soranet handshake", 64)`) Camada de ruído کو semente کرتا ہے تاکہ transcrições inseparáveis ​​رہیں۔
2. **Noise XX hybrid** - string de protocolo `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` com prólogo = exportador TLS۔ Fluxo de mensagens:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   Curve25519 Saída DH اور دونوں Chaves simétricas finais de encapsulamentos Kyber میں mix ہوتے ہیں۔ Negociação de material PQ نہ ہو تو aperto de mão مکمل طور پر abortar ہوتا ہے - fallback somente clássico کی اجازت نہیں۔

3. **Tickets e tokens de quebra-cabeça** - relés `ClientHello` سے پہلے Ticket de prova de trabalho Argon2id مانگ سکتے ہیں۔ Quadros com prefixo de comprimento de tickets ہیں جو solução Argon2 com hash لے جاتے ہیں اور limites de política کے اندر expirar ہوتے ہیں:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Prefixo `SNTK` والے tokens de admissão quebra-cabeças ignorar کرتے ہیں جب emissor کی Política ativa de assinatura ML-DSA-44 اور lista de revogação کے خلاف validar ہو جائے۔

4. **Capacidade de troca de TLV** - carga útil de ruído final نیچے بیان کردہ capacidade TLVs لے جاتا ہے۔ اگر کوئی لازمی capacidade (PQ KEM/assinatura, função, یا versão) ausente ہو یا entrada de diretório سے incompatibilidade کرے تو conexão de clientes abortada کرتے ہیں۔

5. **Registro de transcrição** - retransmite hash de transcrição, impressão digital TLS, e log de conteúdo TLV کرتے ہیں تاکہ detectores de downgrade e pipelines de conformidade کو feed کیا جا سکے۔

## TLVs de capacidade (SNNet-1c)Capacidades ایک مقررہ `typ/length/value` Reutilização de envelope TLV کرتی ہیں:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Quais são os tipos definidos:

- `snnet.pqkem` - Nível Kyber (`kyber768` موجودہ implementação کے لئے).
- `snnet.pqsig` - Conjunto de assinaturas PQ (`ml-dsa-44`).
- `snnet.role` - função de relé (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` – identificador da versão do protocolo.
- `snnet.grease` - intervalo reservado میں entradas de preenchimento aleatório تاکہ TLVs futuros toleram ہوں۔

Os clientes exigiram TLVs کی lista de permissões رکھتے ہیں اور ausente یا downgrade ہونے پر falha de handshake کرتے ہیں۔ Relés یہی set اپنی microdescriptor de diretório میں publicar کرتے ہیں تاکہ validação determinística رہے۔

## Rotação de sal e cegamento CID (SNNet-1b)

- Publicação de registro de governança `SaltRotationScheduleV1` کرتی ہے جس میں Valores `(epoch_id, salt, valid_after, valid_until)` ہوتے ہیں۔ Relés اور gateways cronograma assinado کو editor de diretório سے fetch کرتے ہیں۔
- Clientes `valid_after` پر نیا salt apply کرتے ہیں, پچھلا salt 12 h período de carência کے لئے رکھتے ہیں, اور atualizações atrasadas کے لئے 7 épocas de histórico reter کرتے ہیں۔
- Identificadores cegos canônicos یوں بنتے ہیں:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Gateways `Sora-Req-Blinded-CID` کے ذریعے chave cega قبول کرتے ہیں اور `Sora-Content-CID` میں echo کرتے ہیں۔ Cegamento de circuito/solicitação (`CircuitBlindingKey::derive`) `iroha_crypto::soranet::blinding` میں موجود ہے۔
- اگر relé کوئی epoch miss کرے تو وہ نئے circuitos روک دیتا ہے جب تک agendar download نہ کر لے اور `SaltRecoveryEventV1` emitir کرتا ہے، جسے sinal de paginação de painéis de plantão کے طور پر tratar کرتے ہیں۔

## Dados do diretório e política de proteção

- Identidade de retransmissão de microdescritores (Ed25519 + ML-DSA-65), chaves PQ, capacidade TLVs, tags de região, elegibilidade de proteção, اور موجودہ época do sal anunciada لے جاتے ہیں۔
- Conjuntos de proteção de clientes کو 30 pinos کرتے ہیں اور `guard_set` caches کو instantâneo de diretório assinado کے ساتھ persistir کرتے ہیں۔ CLI e SDK wrappers armazenam superfície de impressão digital em cache

## Telemetria e lista de verificação de implementação

- Métricas de produção e exportação:
  -`soranet_handshake_success_total{role}`
  -`soranet_handshake_failure_total{reason}`
  -`soranet_handshake_latency_seconds`
  -`soranet_capability_mismatch_total`
  -`soranet_salt_rotation_lag_seconds`
- Limites de alerta rotação de sal matriz SOP SLO (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) کے ساتھ رہتی ہیں اور promoção de rede کرنے سے پہلے Alertmanager میں espelho ہونی چاہئیں۔
- Alertas: 5 meses de taxa de falha >5%, atraso de sal >15 minutos, produção ou incompatibilidades de capacidade.
- Etapas de implementação:
  1. Preparação: handshake híbrido, habilitação de pilha PQ, testes de interoperabilidade de relé/cliente, testes de interoperabilidade de relé/cliente.
  2. SOP de rotação de sal (`docs/source/soranet_salt_plan.md`) ensaio کریں اور artefatos de perfuração alteram registro کے ساتھ anexar کریں۔
  3. Diretório میں capacidade de negociação habilitada کریں, پھر relés de entrada, relés intermediários, relés de saída, اور آخر میں clientes تک rollout کریں۔
  4. Fase کے لئے impressões digitais do cache de proteção, programações de sal, e registro de painéis de telemetria کریں؛ pacote de evidências کو `status.md` کے ساتھ anexar کریں۔یہ lista de verificação seguir کرنے سے operador, cliente, اور Equipes SDK Transportes SoraNet کو یکساں رفتار سے adotar کر سکتے ہیں اور SNNet roadmap میں موجود determinismo اور auditoria requisitos