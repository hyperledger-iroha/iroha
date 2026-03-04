---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/transport.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: transporte
título: Transporte SoraNet
sidebar_label: Transporte de transporte
descrição: Aperto de mão, sais de rotação e руководство по возможностям para anonimato sobre SoraNet.
---

:::nota História Canônica
Esta página está configurada para o transporte específico SNNet-1 em `docs/source/soranet/spec.md`. Selecione uma cópia da sincronização, mas a documentação não será exibida na tela.
:::

SoraNet - isso é anônimo, o intervalo de buscas SoraFS, Norito RPC streaming e faixas de dados Nexus. Programa de transporte (itens de roteiro **SNNet-1**, **SNNet-1a** e **SNNet-1b**) determina o aperto de mão, operação pós-quântica (PQ) возможностям и план ротации sais, чтобы каждый relé, cliente e gateway наблюдал одинаковую postura de segurança.

## O que é e o modelo

- Строить треххоповые circuitos (entrada -> meio -> saída) поверх QUIC v1, чтобы злоупотребляющие peers никогда не достигали Torii por favor.
- Наложить handshake Noise XX *hybrid* (Curve25519 + Kyber768) поверх QUIC/TLS, чтобы привязать chaves de sessão para transcrição TLS.
- Capacidade de Требовать TLVs, которые объявляют поддержку PQ KEM/подписи, роль relé e версию протокола; GREASE é um tipo de graxa que pode ser usado para garantir a lubrificação.
- Ежедневно ротировать blinded-content salts e фиксировать guard relays em 30 de dezembro, чтобы churn no diretório não мог деанонимизировать клиентов.
- Держать células фиксированными на 1024 B, вводить preenchimento/células fictícias e экспортировать детерминированную telemetria, чтобы быстро ловить downgrade.

## Aperto de mão do pipeline (SNNet-1a)

1. **Envelope QUIC/TLS** - clientes enviam relés para QUIC v1 e usam handshake TLS 1.3, используя сертификаты Ed25519, подписанные governança CA. Exportador TLS (`tls-exporter("soranet handshake", 64)`) засевает слой Noise, чтобы transcrições были неразделимы.
2. **Noise XX híbrido** - строка протокола `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` com prólogo = exportador TLS. Qual é a solução:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   Use Curve25519 DH e seus encapsulamentos Kyber são configurados em uma classe simmétrica final. Провал переговоров по PQ материалу полностью обрывает handshake - fallback только на классике запрещен.

3. **Tíquetes de quebra-cabeça e tokens** - os relés podem receber o tíquete de prova de trabalho Argon2id para `ClientHello`. Tickets - esses quadros com prefixo de comprimento, que são necessários para definir a configuração do Argon2 e истекают na política de segurança:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Tokens de admissão com o padrão `SNTK` quebra-cabeças de обходят, когда подпись ML-DSA-44 от эмитента валидируется относительно активной politicagem e espionagem.

4. **Capacidade de troca de TLV** - финальный Carga útil de ruído переносит capacidade TLVs, описанные ниже. Os clientes desejam uma solução, exceto a capacidade de aquisição de capacidade (PQ KEM/подпись, роль или versa) отсутствует ou não совпадает с diretório записью.

5. **Registro de transcrição** - retransmite transcrição de hash de registro, impressão digital TLS e conteúdo TLV, detecta detectores de downgrade e pipelines de conformidade.## TLVs de capacidade (SNNet-1c)

Capacidades используют фиксированную TLV-оболочку `typ/length/value`:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Tipos, seção útil:

- `snnet.pqkem` - уровень Kyber (`kyber768` para implementação do teclado).
- `snnet.pqsig` - suite PQ подписей (`ml-dsa-44`).
- `snnet.role` - relé de função (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - identificador da versão do protocolo.
- `snnet.grease` - elementos de enchimento de vedação em um dia de folga, que precisa de um termóstato Existem TLVs.

Os clientes podem usar a lista de permissões para configurar TLVs e validar o handshake antes da solicitação ou do downgrade. Os relés são publicados no microdescritor de diretório, que é validado e determinado.

## Sais de rotação e cegueira CID (SNNet-1b)

- Governança publica o nome `SaltRotationScheduleV1` com o nome `(epoch_id, salt, valid_after, valid_until)`. Relés e gateways são gráficos criados por editores de diretórios.
- Clientes compram novo sal em `valid_after`, fornecem sal em 12 horas, período de graça e história de 7 épocas por dia переносимости задержанных обновлений.
- Identificadores cegos canônicos usados:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Gateways usam chave cega como `Sora-Req-Blinded-CID` e são usados ​​em `Sora-Content-CID`. Cegamento de circuito/solicitação (`CircuitBlindingKey::derive`) instalado em `iroha_crypto::soranet::blinding`.
- Sua época de projeto de relé, em novos circuitos instalados para fazer gráficos e выпускает `SaltRecoveryEventV1`, который painéis de controle de plantão трактуют como sinal de paginação.

## Dados do diretório e política de proteção

- Os microdescritores não incluem relé de identidade (Ed25519 + ML-DSA-65), chaves PQ, TLVs de capacidade, tags de região, elegibilidade de proteção e época do sal anunciada.
- Os clientes фиксируют guard sets em 30 de junho e сохраняют `guard_set` caches вместе с подписанным snapshot do diretório. Os wrappers CLI e SDK usam impressão digital do cache, a implementação de evidências pode ser usada para revisão de alterações.

## Telemetria e lista de verificação de implementação

- Métricas para transporte durante a produção:
  -`soranet_handshake_success_total{role}`
  -`soranet_handshake_failure_total{reason}`
  -`soranet_handshake_latency_seconds`
  -`soranet_capability_mismatch_total`
  -`soranet_salt_rotation_lag_seconds`
- Пороговые значения алертов живут рядом с матрицей SLO para SOP ротации sais (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) e должны быть отражены в O Alertmanager é uma solução de problemas.
- Alertas: taxa de falha >5% em 5 minutos, atraso de sal >15 minutos ou incompatibilidades de capacidade na produção.
- Implementação de Шаги:
  1. Programar testes de interoperabilidade de relé/cliente no teste com handshake híbrido e pilha PQ.
  2. Отрепетировать SOP ротации sais (`docs/source/soranet_salt_plan.md`) e приложить artefatos de perfuração para alterar o registro.
  3. Ative a negociação de capacidade no diretório, conecte-se a relés de entrada, relés intermediários, relés de saída e em clientes de conexão.
  4. Зафиксировать guarda impressões digitais do cache, programações de sal e painéis de telemetria para cada operação; приложить pacote de evidências к `status.md`.Esta lista de verificação fornece comandos para operadores, clientes e SDK usando SoraNet transporta sincronização e antes disso você precisa de treinamento детерминизма и аудита, отраженные no roteiro SNNet.