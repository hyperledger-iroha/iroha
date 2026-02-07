---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/transport.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: transporte
título: Visão geral do transporte SoraNet
sidebar_label: Visão geral do transporte
description: Handshake, rotação de sais e guia de capacidades para a sobreposição de anonimato do SoraNet.
---

:::nota Fonte canônica
Esta página reflete a especificação de transporte SNNet-1 em `docs/source/soranet/spec.md`. Mantenha ambas as cópias sincronizadas.
:::

SoraNet e o overlay de anonimato que sustenta range fetches do SoraFS, streaming de Norito RPC e futuras data lanes do Nexus. O programa de transporte (itens do roadmap **SNNet-1**, **SNNet-1a** e **SNNet-1b**) define um handshake determinístico, negociação de capacidades pós-quânticas (PQ) e um plano de rotação de sais para que cada relé, cliente e gateway observem a mesma postura de segurança.

## Metas e modelo de rede

- Construir circuitos de três saltos (entrada -> meio -> saída) sobre QUIC v1 para que peers abusivos nunca alcancem Torii diretamente.
- Sobrepor um handshake Noise XX *hybrid* (Curve25519 + Kyber768) ao QUIC/TLS para ligar chaves de sessão ao transcrição TLS.
- Exigir TLVs de capacidades que anunciem suporte PQ KEM/assinatura, papel do relé e versão de protocolo; aplique GRAXA em tipos desconhecidos para manter futuras extensões implantáveis.
- Rotacionar sais de conteúdo cego diariamente e fixar guarda relés por 30 dias para que o churn do diretório não possa desanonimizar clientes.
- Manter células fixas em 1024 B, injetar padding/cells dummy e exportar telemetria determinística para capturar tentativas de downgrade rapidamente.

## Pipeline de handshake (SNNet-1a)

1. **Envelope QUIC/TLS** - os clientes se conectam a relés via QUIC v1 e completam um handshake TLS 1.3 usando certificados Ed25519 assinados pela governança CA. O exportador TLS (`tls-exporter("soranet handshake", 64)`) semeia a camada Ruído para que as transcrições sejam inseparáveis.
2. **Noise XX hybrid** - string de protocolo `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` com prólogo = exportador TLS. Fluxo de mensagens:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   O resultado DH do Curve25519 e ambos os encapsulamentos do Kyber são misturados nas chaves simétricas finais. Falhar na negociação do material PQ aborta o handshake por completo - não há fallback apenas clássico.

3. **Puzzle tickets e tokens** - os relés podem exigir um ticket de prova de trabalho Argon2id antes do `ClientHello`. Os tickets são frames com prefixo de comprimento que carregam a solução Argon2 hasheada e expiram dentro dos limites da política:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Tokens de admissão com prefixo `SNTK` pulam os quebra-cabeças quando uma assinatura ML-DSA-44 do emissor é válida contra a política ativa e a lista de revogação.

4. **Troca de capacidade TLV** - o payload final do Noise transporta os TLVs de capacidades descritos abaixo. Os clientes abortam a conexão se qualquer capacidade obrigatória (PQ KEM/assinatura, papel ou versão) estiver ausente ou divergente da entrada do diretório.5. **Registro do transcrito** - retransmite o registro o hash do transcrito, a impressão digital TLS e o conteúdo TLV para detectores alimentares de downgrade e pipelines de compliance.

## TLVs de capacidade (SNNet-1c)

As capacidades reutilizam um envelope TLV fixo de `typ/length/value`:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Tipos definidos hoje:

- `snnet.pqkem` - nível Kyber (`kyber768` para o lançamento atual).
- `snnet.pqsig` - conjunto de assinatura PQ (`ml-dsa-44`).
- `snnet.role` - papel do relé (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - identificador de versão do protocolo.
- `snnet.grease` - entradas de preenchimento aleatório na faixa reservada para garantir tolerância a futuros TLVs.

Os clientes mantêm uma lista de permissões de TLVs exigidas e falham no handshake quando são omitidos ou rebaixados. Relays publicam o mesmo set em seu microdescritor do diretório para que a validação seja determinística.

## Rotação de sais e cegamento CID (SNNet-1b)

- A governança publica um registro `SaltRotationScheduleV1` com valores `(epoch_id, salt, valid_after, valid_until)`. Relés e gateways buscam o calendário assinado no diretório editor.
- Clientes aplicam o novo sal em `valid_after`, mantem o sal anterior por um período de graça de 12 h e retém um histórico de 7 épocas para tolerar atualizações atrasadas.
- Identificadores cegos canônicos usam:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" || salt || cid)
  ```

  Gateways aceitam a chave cega via `Sora-Req-Blinded-CID` e fazem echo em `Sora-Content-CID`. O blinding de circuito/solicitação (`CircuitBlindingKey::derive`) fica em `iroha_crypto::soranet::blinding`.
- Se um relé perder uma época, ele interrompeu novos circuitos até baixar o calendário e emite um `SaltRecoveryEventV1`, que os dashboards de on-call tratam como sinal de paging.

## Dados do diretório e política de guarda

- Microdescritores carregam identidade do relé (Ed25519 + ML-DSA-65), chaves PQ, TLVs de capacidades, tags de regiao, elegibilidade de guarda e época de sal anunciada.
- Clientes fixam guard sets por 30 dias e persistem caches `guard_set` junto com o snapshot contratado do diretório. Wrappers CLI e SDK exibem a impressão digital do cache para que uma evidência de implementação seja anexada a revisões de mudança.

## Telemetria e checklist de implementação- Métricas a exportar antes da produção:
  -`soranet_handshake_success_total{role}`
  -`soranet_handshake_failure_total{reason}`
  -`soranet_handshake_latency_seconds`
  -`soranet_capability_mismatch_total`
  -`soranet_salt_rotation_lag_seconds`
- Os limites de alerta vivem ao lado da matriz SLO do SOP de rotação de sais (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) e devem ser refletidos no Alertmanager antes da promoção da rede.
- Alertas: taxas de falha >5% em 5 minutos, salt lag >15 minutos, ou incompatibilidades de capacidades distribuídas em produção.
- Passos de implementação:
  1. Executar testes de interoperabilidade relé/cliente em staging com o handshake híbrido e o stack PQ habilitados.
  2. Ensaiar o SOP de rotação de sais (`docs/source/soranet_salt_plan.md`) e anexar os artefatos da perfuração ao registro de mudança.
  3. Habilitar a negociação de capacidades no diretório, depois rolar para relés de entrada, relés intermediários, relés de saída e por fim clientes.
  4. Registrador guarda cache de impressões digitais, programações de salt e painéis de telemetria para cada fase; fixação do pacote de evidências a `status.md`.

Seguir esta lista de verificação permite que os operadores, clientes e SDK adotem os transportes do SoraNet em conjunto enquanto atendem aos requisitos de determinismo e auditorias capturados no roadmap SNNet.