<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/transport.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3d384345dcbe8c537c5be3e6b1877f3df6aa779096fc53db714b45e7124dd636
source_last_modified: "2025-11-11T12:56:25.187116+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: transport
title: Visao geral do transporte SoraNet
sidebar_label: Visao geral do transporte
description: Handshake, rotacao de salts e guia de capacidades para o overlay de anonimato do SoraNet.
---

:::note Fonte canonica
Esta pagina espelha a especificacao de transporte SNNet-1 em `docs/source/soranet/spec.md`. Mantenha ambas as copias sincronizadas.
:::

SoraNet e o overlay de anonimato que sustenta range fetches do SoraFS, streaming de Norito RPC e futuros data lanes do Nexus. O programa de transporte (itens do roadmap **SNNet-1**, **SNNet-1a** e **SNNet-1b**) definiu um handshake deterministico, negociacao de capacidades post-quantum (PQ) e um plano de rotacao de salts para que cada relay, client e gateway observe a mesma postura de seguranca.

## Metas e modelo de rede

- Construir circuitos de tres hops (entry -> middle -> exit) sobre QUIC v1 para que peers abusivos nunca alcancem Torii diretamente.
- Sobrepor um handshake Noise XX *hybrid* (Curve25519 + Kyber768) ao QUIC/TLS para ligar chaves de sessao ao transcript TLS.
- Exigir TLVs de capacidades que anunciem suporte PQ KEM/assinatura, papel do relay e versao de protocolo; aplicar GREASE em tipos desconhecidos para manter futuras extensoes implantaveis.
- Rotacionar salts de conteudo cego diariamente e fixar guard relays por 30 dias para que o churn do directory nao possa desanonimizar clients.
- Manter cells fixas em 1024 B, injetar padding/cells dummy e exportar telemetry deterministica para capturar tentativas de downgrade rapidamente.

## Pipeline de handshake (SNNet-1a)

1. **QUIC/TLS envelope** - clients se conectam a relays via QUIC v1 e completam um handshake TLS 1.3 usando certificados Ed25519 assinados pela governance CA. O TLS exporter (`tls-exporter("soranet handshake", 64)`) semeia a camada Noise para que os transcripts sejam inseparaveis.
2. **Noise XX hybrid** - string de protocolo `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` com prologue = TLS exporter. Fluxo de mensagens:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   O resultado DH do Curve25519 e ambas encapsulations do Kyber sao misturados nas chaves simetricas finais. Falhar na negociacao do material PQ aborta o handshake por completo - nao ha fallback apenas classico.

3. **Puzzle tickets e tokens** - relays podem exigir um ticket de proof-of-work Argon2id antes do `ClientHello`. Os tickets sao frames com prefixo de comprimento que carregam a solucao Argon2 hasheada e expiram dentro dos limites da politica:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Tokens de admissao com prefixo `SNTK` pulam os puzzles quando uma assinatura ML-DSA-44 do emissor valida contra a politica ativa e a lista de revogacao.

4. **Troca de capability TLV** - o payload final do Noise transporta os TLVs de capacidades descritos abaixo. Clients abortam a conexao se qualquer capacidade obrigatoria (PQ KEM/assinatura, papel ou versao) estiver ausente ou divergente da entrada do directory.

5. **Registro do transcript** - relays registram o hash do transcript, a impressao digital TLS e o conteudo TLV para alimentar detectores de downgrade e pipelines de compliance.

## Capability TLVs (SNNet-1c)

As capacidades reutilizam um envelope TLV fixo de `typ/length/value`:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Tipos definidos hoje:

- `snnet.pqkem` - nivel Kyber (`kyber768` para o rollout atual).
- `snnet.pqsig` - suite de assinatura PQ (`ml-dsa-44`).
- `snnet.role` - papel do relay (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - identificador de versao do protocolo.
- `snnet.grease` - entradas de preenchimento aleatorias na faixa reservada para garantir tolerancia a futuros TLVs.

Clients mantem uma allow-list de TLVs requeridos e falham o handshake quando sao omitidos ou downgraded. Relays publicam o mesmo set em seu microdescriptor do directory para que a validacao seja deterministica.

## Rotacao de salts e CID blinding (SNNet-1b)

- A governance publica um registro `SaltRotationScheduleV1` com valores `(epoch_id, salt, valid_after, valid_until)`. Relays e gateways buscam o calendario assinado no directory publisher.
- Clients aplicam o novo salt em `valid_after`, mantem o salt anterior por um periodo de gracia de 12 h e retem um historico de 7 epochs para tolerar atualizacoes atrasadas.
- Identificadores cegos canonicos usam:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" || salt || cid)
  ```

  Gateways aceitam a chave cega via `Sora-Req-Blinded-CID` e fazem echo em `Sora-Content-CID`. O blinding de circuit/request (`CircuitBlindingKey::derive`) fica em `iroha_crypto::soranet::blinding`.
- Se um relay perder uma epoch, ele interrompe novos circuits ate baixar o calendario e emite um `SaltRecoveryEventV1`, que os dashboards de on-call tratam como sinal de paging.

## Dados do directory e politica de guard

- Microdescriptors carregam identidade do relay (Ed25519 + ML-DSA-65), chaves PQ, TLVs de capacidades, tags de regiao, elegibilidade de guard e a epoch de salt anunciada.
- Clients fixam guard sets por 30 dias e persistem caches `guard_set` junto com o snapshot assinado do directory. Wrappers CLI e SDK exibem o fingerprint do cache para que a evidencia de rollout seja anexada a revisoes de mudanca.

## Telemetry e checklist de rollout

- Metricas a exportar antes da producao:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- Os limites de alerta vivem ao lado da matriz SLO do SOP de rotacao de salts (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) e devem ser refletidos no Alertmanager antes da promocao da rede.
- Alertas: taxa de falha >5% em 5 minutos, salt lag >15 minutos, ou mismatches de capacidades observados em producao.
- Passos de rollout:
  1. Executar testes de interoperabilidade relay/client em staging com o handshake hybrid e o stack PQ habilitados.
  2. Ensaiar o SOP de rotacao de salts (`docs/source/soranet_salt_plan.md`) e anexar os artefatos do drill ao change record.
  3. Habilitar a negociacao de capacidades no directory, depois rolar para entry relays, middle relays, exit relays e por fim clients.
  4. Registrar guard cache fingerprints, salt schedules e telemetry dashboards para cada fase; anexar o evidence bundle a `status.md`.

Seguir este checklist permite que times de operadores, clients e SDK adotem os transports de SoraNet em conjunto enquanto atendem aos requisitos de determinismo e auditoria capturados no roadmap SNNet.
