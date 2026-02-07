---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/transport.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: transporte
título: Vista do conjunto de transporte SoraNet
sidebar_label: Visual do conjunto de transporte
descrição: Aperto de mão, rotação de sais e guia de capacidades para sobreposição de anonimato SoraNet.
---

:::nota Fonte canônica
Esta página reflete a especificação de transporte SNNet-1 em `docs/source/soranet/spec.md`. Gardez les duas cópias alinhadas jusqu'a la retraite do antigo conjunto de documentação.
:::

SoraNet é a sobreposição anônima que contém o intervalo de buscas de SoraFS, o streaming Norito RPC e as futuras faixas de dados de Nexus. O programa de transporte (itens de roadmap **SNNet-1**, **SNNet-1a** e **SNNet-1b**) define um handshake determinado, uma negociação de capacidade pós-quântica (PQ) e um plano de rotação de sais para que cada relé, cliente e gateway observem a mesma postura de segurança.

## Objetivos e modelo de patrimônio

- Construa circuitos com três sauts (entrada -> meio -> saída) no QUIC v1 para que os pares abusivos não sejam informados jamais na direção Torii.
- Superposer um handshake Noise XX *hybride* (Curve25519 + Kyber768) par-dessus QUIC/TLS para colocar arquivos de sessão na transcrição TLS.
- Exige des TLVs de capacidade que anunciam o suporte PQ KEM/assinatura, a função do relé e a versão do protocolo; GREASE os tipos desconhecidos para que as extensões futuras permaneçam implantáveis.
- Rotação diária dos sais de conteúdo aveugle e epinglage des guard relays por 30 dias para que a rotatividade do diretório não possa desanonimizar os clientes.
- Garder des cell fixes a 1024 B, injecter du padding/des cell dummy, et exporter une telemetrie determinista pour detecte rapidement les tentatives de downgrade.

## Pipeline de handshake (SNNet-1a)

1. **Envelope QUIC/TLS** - os clientes se conectam a relés via QUIC v1 e terminam um handshake TLS 1.3 usando os certificados Ed25519 assinados pela CA de governança. O exportador TLS (`tls-exporter("soranet handshake", 64)`) alimenta o sofá Noise para que as transcrições sejam inseparáveis.
2. **Noise XX hybrid** - cadeia de protocolo `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` com prólogo = exportador TLS. Fluxo de mensagens:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   A saída DH Curve25519 e os dois encapsulamentos Kyber são mesclados nas extremidades simétricas. Uma verificação de negociação PQ anula a conclusão do handshake: nenhum substituto clássico não é autorizado.

3. **Tíquetes de quebra-cabeça e tokens** - os relés podem exigir um tíquete de prova de trabalho Argon2id antes de `ClientHello`. Os ingressos são prefixos de quadros por longo tempo que transportam a solução Argon2 hachee e expiram nos bornes da política:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Os tokens de prefixos de admissão de `SNTK` contornam os quebra-cabeças ao receber uma assinatura ML-DSA-44 do emissor válido contra a política ativa e a lista de revogação.4. **Echange decapacite TLV** - a carga útil final de Noise transporta os TLVs de capacite decrits ci-dessous. Os clientes abandonam a conexão se uma capacidade obrigatória (PQ KEM/assinatura, função ou versão) estiver limitada ou em conflito com a entrada do diretório.

5. **Jornalização da transcrição** - os relés registram o hash da transcrição, a impressão de TLS e o conteúdo TLV para alimentar os detectores de downgrade e os pipelines de conformidade.

## TLVs de capacidade (SNNet-1c)

As capacidades reutilizam um envelope TLV fixo `typ/length/value`:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Tipos definidos aujourd'hui:

- `snnet.pqkem` - nível Kyber (`kyber768` para implementação atual).
- `snnet.pqsig` - conjunto de assinatura PQ (`ml-dsa-44`).
- `snnet.role` - função do relé (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - identificador da versão do protocolo.
- `snnet.grease` - entradas de remplissage aleatórias na praia reservada para garantir a tolerância de futuros TLVs.

Os clientes mantêm uma lista de permissões de requisitos de TLVs e ecoam o handshake quando eles são omitidos ou degradados. Os relés publicam o meme definido em seu microdescritor de diretório para que a validação seja determinada.

## Rotação de sais e cegamento CID (SNNet-1b)

- A governança publica um registro `SaltRotationScheduleV1` com os valores `(epoch_id, salt, valid_after, valid_until)`. Os relés e gateways recuperam o calendário assinado pelo editor do diretório.
- Os clientes aplicam o sal novo a `valid_after`, conservam o sal precedente para um período de graça de 12 h e cultivam uma história de 7 épocas para tolerar as mises a jour retardadees.
- Les identificants aveugles canonices utilisent:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Os gateways aceitam a chamada via `Sora-Req-Blinded-CID` e o reenvio em `Sora-Content-CID`. O circuito/solicitação de cegamento (`CircuitBlindingKey::derive`) está disponível em `iroha_crypto::soranet::blinding`.
- Se um relé já existe há algum tempo, os novos circuitos são bloqueados até que ele carregue o calendário e emita um `SaltRecoveryEventV1`, para que os painéis de controle sejam exibidos como um sinal de paginação.

## Donnees de diretório e política de guarda

- Os microdescritores indicam a identidade do relé (Ed25519 + ML-DSA-65), os arquivos PQ, os TLVs de capacidade, as tags de região, a guarda elegível e a época de sal anunciada.
- Os clientes mantêm os conjuntos de guarda pendentes por 30 dias e os caches persistentes `guard_set` nas costas do instantâneo assinam o diretório. Os wrappers CLI e SDK expõem a ocupação do cache para que as prévias de implementação sejam anexadas às revisões de alterações.

## Telemetria e lista de verificação de implementação- Métricas exportadoras de vanguarda na produção:
  -`soranet_handshake_success_total{role}`
  -`soranet_handshake_failure_total{reason}`
  -`soranet_handshake_latency_seconds`
  -`soranet_capability_mismatch_total`
  -`soranet_salt_rotation_lag_seconds`
- Os seus alertas vivem nas costas da matriz SLO do SOP de rotação dos sais (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) e devem ser refletidos no Alertmanager antes da promoção do reseau.
- Alertas: >5% de taux d'echec em 5 minutos, salt lag >15 minutos, ou incompatibilidades de capacidade observadas na produção.
- Etapas de lançamento:
  1. Execute os testes de interoperabilidade do relé/cliente em teste com o handshake híbrido e a pilha PQ ativa.
  2. Repita o SOP de rotação dos sais (`docs/source/soranet_salt_plan.md`) e junte os artefatos da broca ao dossiê de troca.
  3. Ative a negociação de capacidades no diretório, depois implante os relés de entrada, os relés intermediários, os relés de saída e, finalmente, os clientes.
  4. Registre as impressões de cache de guarda, os calendários de sal e os painéis de telemetria para cada fase; junte o pacote de testes a `status.md`.

Seguir esta lista de verificação permite que equipes de operadores, clientes e SDK adotem os transportes SoraNet de acordo com o determinismo e as exigências de auditoria capturadas no roteiro SNNet.