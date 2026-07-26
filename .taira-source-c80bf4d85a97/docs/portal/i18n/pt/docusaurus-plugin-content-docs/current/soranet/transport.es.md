---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/transport.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: transporte
título: Resumo do transporte de SoraNet
sidebar_label: Resumo do transporte
descrição: Aperto de mão, rotação de sais e guia de capacidades para a sobreposição de anonimato de SoraNet.
---

:::nota Fonte canônica
Esta página reflete a especificação de transporte SNNet-1 em `docs/source/soranet/spec.md`. Mantenha ambas as cópias sincronizadas.
:::

SoraNet é a sobreposição de anonimato que respalda o intervalo de buscas de SoraFS, o streaming de Norito RPC e as futuras faixas de dados de Nexus. O programa de transporte (itens do roteiro **SNNet-1**, **SNNet-1a** e **SNNet-1b**) define um handshake determinista, negociação de capacidades pós-quânticas (PQ) e um plano de rotação de sais para que cada relé, cliente e gateway observem a mesma postura de segurança.

## Objetivos e modelo de red

- Construir circuitos de três saltos (entrada -> meio -> saída) sobre QUIC v1 para que os pares abusivos nunca entrem em Torii diretamente.
- Superpone um handshake Noise XX *híbrido* (Curve25519 + Kyber768) sobre QUIC/TLS para ligar as chaves de sessão à transcrição TLS.
- Solicitar TLVs de capacidades que anunciem suporte PQ KEM/firma, rolo de relé e versão de protocolo; Faça GREASE de tipos desconhecidos para que futuras extensões sigam desplegables.
- Rotar sais de conteúdo ciego a diario e fijar guard relays por 30 dias para que a rotatividade do diretório não possa desanonimizar clientes.
- Manter células fixas de 1024 B, inserir preenchimento/celdas dummy e exportar telemetria determinista para que as intenções de downgrade sejam detectadas rapidamente.

## Pipeline de handshake (SNNet-1a)

1. **Envelope QUIC/TLS** - os clientes se conectam a relés no QUIC v1 e completam um handshake TLS 1.3 usando certificados Ed25519 firmados pela CA de governança. O exportador TLS (`tls-exporter("soranet handshake", 64)`) alimenta a capa Noise para que as transcrições sejam inseparáveis.
2. **Noise XX hybrid** - string de protocolo `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` com prólogo = exportador TLS. Fluxo de mensagens:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   A saída DH de Curve25519 e ambos os encapsulamentos Kyber se misturam nas chaves finais simétricas. Se não houver negociação de material PQ, o aperto de mão será abortado por completo: não se permitirá fallback solo clássico.

3. **Tíquetes e tokens de quebra-cabeça** - os relés podem exigir um tíquete de prova de trabalho Argon2id antes de `ClientHello`. Os ingressos são quadros com prefijo de longitude que leva a solução Argon2 hasheada e expira dentro dos limites da política:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Os tokens de admissão prefixados com `SNTK` evitam os quebra-cabeças quando uma firma ML-DSA-44 do emissor é válida contra a política ativa e a lista de revogação.

4. **Intercâmbio de capacidade TLV** - a carga útil final de Noise transporta os TLVs de capacidades descritas abaixo. Os clientes abortam a conexão se alguma capacidade obrigatória (PQ KEM/firma, rolo ou versão) estiver faltando ou não coincidir com a entrada do diretório.5. **Registro da transcrição** - os relés registram o hash da transcrição, o huella TLS e o conteúdo TLV para alimentar detectores de downgrade e pipelines de cumprimento.

## TLVs de capacidade (SNNet-1c)

As capacidades reutilizam um arquivo TLV de `typ/length/value`:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Tipos definidos hoje:

- `snnet.pqkem` - nível Kyber (`kyber768` para o lançamento real).
- `snnet.pqsig` - pacote de firma PQ (`ml-dsa-44`).
- `snnet.role` - papel do relé (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - identificador de versão do protocolo.
- `snnet.grease` - entradas de relleno aleatorias no rango reservado para garantir que futuros TLVs sejam tolerados.

Os clientes mantêm uma lista de permissões de TLVs exigidas e perdem o handshake se forem omitidos ou degradados. Os relés publicam o mesmo conjunto em seu microdescritor do diretório para que a validação seja determinista.

## Rotação de sais e cegamento CID (SNNet-1b)

- Governança publica um registro `SaltRotationScheduleV1` com valores `(epoch_id, salt, valid_after, valid_until)`. Relés e gateways obtêm o calendário firmado do editor do diretório.
- Os clientes aplicam o novo sal em `valid_after`, mantêm o sal anterior durante um período de graça de 12 horas e retienen um histórico de 7 épocas para tolerar atualizações retroativas.
- Los identificadores ciegos canonicos usam:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Os gateways aceitam a chave ciega via `Sora-Req-Blinded-CID` e fazem eco em `Sora-Content-CID`. O cegamento do circuito/solicitação (`CircuitBlindingKey::derive`) vive em `iroha_crypto::soranet::blinding`.
- Se um relé falhar uma época, detenha novos circuitos até que o calendário seja descarregado e emita um `SaltRecoveryEventV1`, que os painéis de proteção tratam como um sinal de paginação.

## Dados de diretório e política de guardas

- Os microdescritores de identidade do relé (Ed25519 + ML-DSA-65), chaves PQ, TLVs de capacidades, etiquetas de região, elegibilidade de guarda e época de sal anunciada.
- Os clientes mantêm conjuntos de guarda por 30 dias e persistem caches `guard_set` junto com o snapshot firmado no diretório. CLI e SDK expõem a camada de cache para que a evidência de implementação seja anexada às revisões de mudança.

## Telemetria e checklist de implementação- Métricas a exportar antes da produção:
  -`soranet_handshake_success_total{role}`
  -`soranet_handshake_failure_total{reason}`
  -`soranet_handshake_latency_seconds`
  -`soranet_capability_mismatch_total`
  -`soranet_salt_rotation_lag_seconds`
- Os guarda-chuvas de alerta vivem junto com a matriz SLO de SOP de rotação de sais (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) e devem ser refletidos no Alertmanager antes de promover a rede.
- Alertas: >5% de tasa de fallo em 5 minutos, salt lag >15 minutos ou incompatibilidades de capacidades distribuídas na produção.
- Passos de lançamento:
  1. Execute testes de interoperabilidade relé/cliente em teste com o handshake híbrido e a pilha PQ habilitada.
  2. Ensayar o SOP de rotação de sais (`docs/source/soranet_salt_plan.md`) e adicionar os artefatos do simulacro ao registro de mudanças.
  3. Habilitar a negociação de capacidades no diretório, depois desplegar os relés de entrada, relés intermediários, relés de saída e finalmente clientes.
  4. Registrador de impressões digitais de cache de guarda, calendários de sal e painéis de telemetria para cada fase; adicione o pacote de evidência a `status.md`.

Seguir esta lista de verificação permite que equipes de operadores, clientes e SDK adotem os transportes de SoraNet no mesmo ritmo enquanto cumprem a determinação e os requisitos de auditoria capturados no roteiro SNNet.