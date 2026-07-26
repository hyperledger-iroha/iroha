---
lang: pt
direction: ltr
source: docs/source/kaigi_privacy_design.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b7ffca7e960376a2959357cd865d8dab5afa1dfcb959adbc688b6db60977c8f
source_last_modified: "2026-01-04T10:50:53.617088+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Privacidade Kaigi e design de retransmissão

Este documento captura a evolução focada na privacidade que introduz conhecimento zero
provas de participação e retransmissões do tipo cebola sem sacrificar o determinismo ou
auditabilidade do razão.

# Visão geral

O design abrange três camadas:

- **Privacidade da lista** – oculte as identidades dos participantes na rede enquanto mantém as permissões do anfitrião e o faturamento consistentes.
- **Opacidade de uso** – permite que os hosts registrem o uso medido sem divulgar publicamente os detalhes por segmento.
- **Relés de sobreposição** – roteia pacotes de transporte através de peers multi-hop para que os observadores da rede não possam saber quais participantes se comunicam.

Todas as adições permanecem Norito-first, são executadas na versão 1 da ABI e devem ser executadas de forma determinística em hardware heterogêneo.

# Metas

1. Admitir/expulsar participantes usando provas de conhecimento zero para que o razão nunca exponha IDs brutos de contas.
2. Mantenha fortes garantias contábeis: cada evento de ingresso, saída e uso ainda deve ser reconciliado de forma determinística.
3. Fornece manifestos de retransmissão opcionais que descrevem rotas cebola para canais de controle/dados e podem ser auditados na cadeia.
4. Mantenha o substituto (lista totalmente transparente) operacional para implantações que não exigem privacidade.

# Resumo do modelo de ameaças

- **Adversários:** Observadores de rede (ISPs), validadores curiosos, operadores de retransmissão maliciosos e hosts semi-honestos.
- **Ativos protegidos:** Identidade do participante, tempo de participação, detalhes de uso/faturamento por segmento e metadados de roteamento de rede.
- **Suposições:** Os anfitriões ainda aprendem o verdadeiro conjunto de participantes fora da cadeia; os pares do razão verificam as provas de forma determinística; os relés de sobreposição não são confiáveis, mas têm taxa limitada; As primitivas HPKE e SNARK já existem na base de código.

# Mudanças no modelo de dados

Todos os tipos vivem em `iroha_data_model::kaigi`.

```rust
/// Commitment to a participant identity (Poseidon hash of account + domain salt).
pub struct KaigiParticipantCommitment {
    pub commitment: FixedBinary<32>,
    pub alias_tag: Option<String>,
}

/// Nullifier unique to each join action, prevents double-use of proofs.
pub struct KaigiParticipantNullifier {
    pub digest: FixedBinary<32>,
    pub issued_at_ms: u64,
}

/// Relay path description used by clients to set up onion routing.
pub struct KaigiRelayManifest {
    pub hops: Vec<KaigiRelayHop>,
    pub expiry_ms: u64,
}

pub struct KaigiRelayHop {
    pub relay_id: AccountId,
    pub hpke_public_key: FixedBinary<32>,
    pub weight: u8,
}
```

`KaigiRecord` ganha os seguintes campos:

- `roster_commitments: Vec<KaigiParticipantCommitment>` – substitui a lista `participants` exposta quando o modo de privacidade é ativado. As implantações clássicas podem manter ambos preenchidos durante a migração.
- `nullifier_log: Vec<KaigiParticipantNullifier>` – estritamente somente para acréscimos, limitado por uma janela contínua para manter os metadados limitados.
- `room_policy: KaigiRoomPolicy` – seleciona a posição de autenticação do visualizador para a sessão (salas `Public` espelham relés somente leitura; salas `Authenticated` exigem tickets do visualizador antes que uma saída encaminhe pacotes).
- `relay_manifest: Option<KaigiRelayManifest>` – manifesto estruturado codificado com Norito para que saltos, chaves HPKE e pesos permaneçam canônicos sem correções JSON.
- Enum `privacy_mode: KaigiPrivacyMode` (veja abaixo).

```rust
pub enum KaigiPrivacyMode {
    Transparent,
    ZkRosterV1,
}
```

`NewKaigi` recebe campos opcionais correspondentes para que os hosts possam optar pela privacidade no momento da criação.


- Os campos usam auxiliares `#[norito(with = "...")]` para impor a codificação canônica (little-endian para números inteiros, saltos classificados por posição).
- `KaigiRecord::from_new` semeia os novos vetores vazios e copia qualquer manifesto de retransmissão fornecido.

# Mudanças na superfície de instrução

## Ajudante de início rápido de demonstração

Para demonstrações ad hoc e testes de interoperabilidade, a CLI agora expõe
`iroha kaigi quickstart`. Isto:- Reutiliza a configuração CLI (domínio `wonderland` + conta), a menos que seja substituído por `--domain`/`--host`.
- Gera um nome de chamada baseado em carimbo de data/hora quando `--call-name` é omitido e envia `CreateKaigi` no endpoint Torii ativo.
- Opcionalmente, ingressa automaticamente no host (`--auto-join-host`) para que os visualizadores possam se conectar imediatamente.
- Emite um resumo JSON contendo URL Torii, identificadores de chamada, política de privacidade/sala, um comando de junção pronto para copiar e os testadores de caminho de spool devem monitorar (por exemplo, `storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/*.norito`). Use `--summary-out path/to/file.json` para persistir o blob.

Este auxiliar **não** substitui a necessidade de um nó `irohad --sora` em execução: rotas de privacidade, arquivos em spool e manifestos de retransmissão permanecem apoiados pelo razão. Ele simplesmente corta o padrão ao criar salas temporárias para festas externas.

### Script de demonstração de um comando

Para um caminho ainda mais rápido, existe um script complementar: `scripts/kaigi_demo.sh`.
Ele executa o seguinte para você:

1. Assina o pacote `defaults/nexus/genesis.json` em `target/kaigi-demo/genesis.nrt`.
2. Inicia `irohad --sora` com o bloco assinado (logs em `target/kaigi-demo/irohad.log`) e aguarda que Torii exponha `http://127.0.0.1:8080/status`.
3. Executa `iroha kaigi quickstart --auto-join-host --summary-out target/kaigi-demo/kaigi_summary.json`.
4. Imprime o caminho para o resumo JSON mais o diretório de spool (`storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/`) para que você possa compartilhá-lo com testadores externos.

Variáveis de ambiente:

- `TORII_URL` — substitui o endpoint Torii para sondar (padrão `http://127.0.0.1:8080`).
- `RUN_DIR` — substitui o diretório de trabalho (padrão `target/kaigi-demo`).

Pare a demonstração pressionando `Ctrl+C`; a armadilha no script termina `irohad` automaticamente. Os arquivos em spool e o resumo permanecem no disco para que você possa entregar artefatos após o encerramento do processo.

##`CreateKaigi`

- Valida `privacy_mode` em relação às permissões do host.
- Se um `relay_manifest` for fornecido, aplique ≥3 saltos, pesos diferentes de zero, presença de chave HPKE e exclusividade para que os manifestos na cadeia permaneçam auditáveis.
- Valide a entrada `room_policy` de SDKs/CLI (`public` vs `authenticated`) e propague-a para o provisionamento SoraNet para que os caches de retransmissão exponham as categorias GAR corretas (`stream.kaigi.public` vs `stream.kaigi.authenticated`). Os hosts conectam isso por meio de `iroha kaigi create --room-policy …`, o campo `roomPolicy` do JS SDK ou configurando `room_policy` quando os clientes Swift montam a carga útil Norito antes do envio.
- Armazena logs de compromisso/anulador vazios.

##`JoinKaigi`

Parâmetros:

- `proof: ZkProof` (wrapper de bytes Norito) – Prova Groth16 atestando que o chamador conhece `(account_id, domain_salt)` cujo hash Poseidon é igual ao `commitment` fornecido.
-`commitment: FixedBinary<32>`
-`nullifier: FixedBinary<32>`
- `relay_hint: Option<KaigiRelayHop>` – substituição opcional por participante para o próximo salto.

Etapas de execução:

1. Se `record.privacy_mode == Transparent`, retorne ao comportamento atual.
2. Verifique a prova Groth16 em relação à entrada de registro do circuito `KAIGI_ROSTER_V1`.
3. Certifique-se de que `nullifier` não tenha aparecido em `record.nullifier_log`.
4. Anexe entradas de compromisso/anulador; se `relay_hint` for fornecido, corrija a visualização do manifesto de retransmissão para este participante (armazenado apenas no estado de sessão na memória, não na cadeia).##`LeaveKaigi`

O modo transparente corresponde à lógica atual.

O modo privado requer:

1. Prova de que o chamador conhece um compromisso em `record.roster_commitments`.
2. Atualização do anulador comprovando licença de uso único.
3. Remova entradas de compromisso/anulador. A auditoria preserva lápides para janelas de retenção fixas para evitar vazamentos estruturais.

##`RecordKaigiUsage`

Estende a carga útil com:

- `usage_commitment: FixedBinary<32>` – compromisso com a tupla de uso bruto (duração, gás, ID do segmento).
- Prova ZK opcional para verificar se o delta corresponde aos logs criptografados fornecidos fora do livro-razão.

Os hosts ainda podem enviar totais transparentes; o modo de privacidade apenas torna o campo de compromisso obrigatório.

# Verificação e Circuitos

- `iroha_core::smartcontracts::isi::kaigi::privacy` agora executa escalação completa
  verificação por padrão. Ele resolve `zk.kaigi_roster_join_vk` (junta) e
  `zk.kaigi_roster_leave_vk` (sai) da configuração,
  procura o `VerifyingKeyRef` correspondente no WSV (garantindo que o registro seja
  `Active`, correspondência de identificadores de back-end/circuito e alinhamento de compromissos), cobranças
  contabilidade de bytes e despachos para o back-end ZK configurado.
- O recurso `kaigi_privacy_mocks` mantém o verificador de stub determinístico para
  testes de unidade/integração e trabalhos de CI restritos podem ser executados sem um backend Halo2.
  As compilações de produção devem manter o recurso desabilitado para impor provas reais.
- A caixa emite um erro em tempo de compilação se `kaigi_privacy_mocks` estiver habilitado em um
  compilação sem teste e sem `debug_assertions`, evitando binários de lançamento acidental
  do envio com o esboço.
- Os operadores precisam (1) registrar o verificador de escala definido por meio da governança, e
  (2) conjunto `zk.kaigi_roster_join_vk`, `zk.kaigi_roster_leave_vk` e
  `zk.kaigi_usage_vk` em `iroha_config` para que os hosts possam resolvê-los em tempo de execução.
  Até que as chaves estejam presentes, as entradas, saídas e chamadas de uso de privacidade falharão
  determinísticamente.
- `crates/kaigi_zk` agora envia circuitos Halo2 para entradas/saídas de escalação e uso
  compromissos juntamente com os compressores reutilizáveis (`commitment`, `nullifier`,
  `usage`). Os circuitos de lista expõem a raiz Merkle (quatro little-endian
  membros de 64 bits) como entradas públicas adicionais para que o host possa verificar a prova
  contra a raiz da lista armazenada antes da verificação. Os compromissos de uso são
  aplicado por `KaigiUsageCommitmentCircuit`, que vincula `(duração, gás,
  segment)` para o hash do livro-razão.
- Entradas do circuito `Join`: `(commitment, nullifier, domain_salt)` e privado
  `(account_id)`. As entradas públicas incluem `commitment`, `nullifier` e
  quatro membros da raiz Merkle para a árvore de compromisso da lista (a lista
  permanece fora da cadeia, mas a raiz está ligada à transcrição).
- Determinismo: fixamos parâmetros de Poseidon, versões de circuitos e índices no
  registro. Qualquer alteração aumenta `KaigiPrivacyMode` para `ZkRosterV2` com correspondência
  testes/arquivos dourados.

# Sobreposição de roteamento Onion

## Registro de retransmissão- Os relés se auto-registram como entradas de metadados de domínio `kaigi_relay::<relay_id>`, incluindo material de chave HPKE e classe de largura de banda.
- A instrução `RegisterKaigiRelay` persiste o descritor nos metadados do domínio, emite um resumo `KaigiRelayRegistered` (com impressão digital HPKE e classe de largura de banda) e pode ser invocada novamente para girar chaves deterministicamente.
- A governança seleciona listas de permissões por meio de metadados de domínio (`kaigi_relay_allowlist`) e retransmite atualizações de registro/manifesto que impõem a adesão antes de aceitar novos caminhos.

## Criação de Manifesto

- Os hosts constroem caminhos multi-hop (comprimento mínimo 3) a partir dos relés disponíveis. O manifesto codifica a sequência de AccountIds e as chaves públicas HPKE necessárias para criptografar o envelope em camadas.
- `relay_manifest` armazenado na cadeia contém descritores de salto e expiração (`KaigiRelayManifest` codificado em Norito); chaves efêmeras reais e compensações por sessão são trocadas fora do razão usando HPKE.

## Sinalização e Mídia

- A troca SDP/ICE continua via metadados Kaigi, mas criptografada por salto. Os validadores veem apenas o texto cifrado HPKE mais os índices de cabeçalho.
- Pacotes de mídia viajam através de relés usando QUIC com cargas seladas. Cada salto descriptografa uma camada para aprender o endereço do próximo salto; o destinatário final obtém o fluxo de mídia após remover todas as camadas.

## Failover

- Os clientes monitoram a integridade do relé por meio da instrução `ReportKaigiRelayHealth`, que persiste o feedback assinado nos metadados do domínio (`kaigi_relay_feedback::<relay_id>`), transmite `KaigiRelayHealthUpdated` e permite que a governança/hosts raciocinem sobre a disponibilidade atual. Quando uma retransmissão falha, o host emite um manifesto atualizado e registra um evento `KaigiRelayManifestUpdated` (veja abaixo).
- Os hosts aplicam alterações de manifesto no livro-razão por meio da instrução `SetKaigiRelayManifest`, que substitui o caminho armazenado ou o limpa completamente. A compensação emite um resumo com `hop_count = 0` para que os operadores possam observar a transição de volta ao roteamento direto.
- Métricas Prometheus (`kaigi_relay_registered_total`, `kaigi_relay_registration_bandwidth_class`, `kaigi_relay_manifest_updates_total`, `kaigi_relay_manifest_hop_count`, `kaigi_relay_health_reports_total`, `kaigi_relay_health_state`, `kaigi_relay_failover_total`, `kaigi_relay_failover_hop_count`) agora apresenta rotatividade de relé, status de integridade e cadência de failover para painéis do operador.

# Eventos

Estenda as variantes `DomainEvent`:

- `KaigiRosterSummary` – emitido com contagens anônimas e a escalação atual
  root sempre que a lista muda (root é `None` em modo transparente).
- `KaigiRelayRegistered` – emitido sempre que um registro de relé é criado ou atualizado.
- `KaigiRelayManifestUpdated` – emitido quando o manifesto do relé é alterado.
- `KaigiRelayHealthUpdated` – emitido quando os hosts enviam um relatório de integridade do relé via `ReportKaigiRelayHealth`.
- `KaigiUsageSummary` – emitido após cada segmento de utilização, expondo apenas os totais agregados.

Os eventos são serializados com Norito, expondo apenas hashes e contagens de compromisso.As ferramentas CLI (`iroha kaigi …`) agrupam cada ISI para que os operadores possam registrar sessões,
envie atualizações de lista, relate a integridade do relé e registre o uso sem criar transações manualmente.
Manifestos de retransmissão e provas de privacidade são carregados de arquivos JSON/hex transmitidos
o caminho normal de envio da CLI, facilitando o script do contrato
admissão em ambientes de teste.

# Contabilidade de Gás

- Novas constantes em `crates/iroha_core/src/gas.rs`:
  -`BASE_KAIGI_JOIN_ZK`, `BASE_KAIGI_LEAVE_ZK` e `BASE_KAIGI_USAGE_ZK`
    calibrado em relação aos tempos de verificação do Halo2 (≈1,6ms para escalação
    entra/sai, ≈1,2 ms para uso no Apple M2 Ultra). As sobretaxas continuam
    dimensionar com tamanho de byte de prova via `PER_KAIGI_PROOF_BYTE`.
- Os compromissos `RecordKaigiUsage` pagam uma taxa extra com base no tamanho do compromisso e na verificação do comprovante.
- O equipamento de calibração reutilizará a infraestrutura de ativos confidenciais com sementes fixas.

# Estratégia de teste

- Testes unitários verificando codificação/decodificação Norito para `KaigiParticipantCommitment`, `KaigiRelayManifest`.
- Testes de ouro para visualização JSON garantindo ordenação canônica.
- Testes de integração girando uma mini-rede com (veja
  `crates/iroha_core/tests/kaigi_privacy.rs` para a cobertura atual):
  - Ciclos privados de ingresso/saída usando provas simuladas (sinalizador de recurso `kaigi_privacy_mocks`).
  - Atualizações de manifesto de retransmissão propagadas por meio de eventos de metadados.
- Testes de UI Trybuild cobrindo configuração incorreta do host (por exemplo, falta de manifesto de retransmissão no modo de privacidade).
- Ao executar testes de unidade/integração em ambientes restritos (por exemplo, o Codex
  sandbox), exporte `NORITO_SKIP_BINDINGS_SYNC=1` para ignorar a ligação Norito
  verificação de sincronização aplicada por `crates/norito/build.rs`.

# Plano de Migração

1. ✅ Envie adições ao modelo de dados atrás dos padrões `KaigiPrivacyMode::Transparent`.
2. ✅ Verificação de caminho duplo do fio: a produção desativa `kaigi_privacy_mocks`,
   resolve `zk.kaigi_roster_vk` e executa verificação de envelope real; testes podem
   ainda habilite o recurso para stubs determinísticos.
3. ✅ Introduziu a caixa Halo2 `kaigi_zk` dedicada, gás calibrado e com fio
   cobertura de integração para executar provas reais de ponta a ponta (as simulações agora são apenas para teste).
4. ⬜ Descontinuar o vetor `participants` transparente assim que todos os consumidores compreenderem os compromissos.

# Perguntas abertas

- Definir a estratégia de persistência da árvore Merkle: on-chain vs off-chain (tendência atual: árvore off-chain com compromissos de raiz on-chain). *(Rastreado em KPG-201.)*
- Determine se os manifestos de retransmissão devem suportar vários caminhos (caminhos redundantes simultâneos). *(Rastreado em KPG-202.)*
- Esclarecer a governança para reputações de retransmissão – precisamos de cortes ou apenas proibições suaves? *(Rastreado em KPG-203.)*

Esses itens devem ser resolvidos antes de ativar o `KaigiPrivacyMode::ZkRosterV1` em produção.