---
lang: pt
direction: ltr
source: docs/source/iroha_3_whitepaper.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 07e149429887b0dfc38cf0619552cbefcbae4dd1ec9fe9e9d47a05371ed08f29
source_last_modified: "2026-01-03T18:07:57.204376+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v3.0 (Visualização Nexus)

Este documento captura a arquitetura Hyperledger Iroha v3 voltada para o futuro, com foco no multi-lane
pipeline, espaços de dados Nexus e o Asset Exchange Toolkit (AXT). Ele complementa o white paper Iroha v2 por
descrevendo capacidades futuras que estão ativamente em desenvolvimento.

---

## 1. Visão geral

Iroha v3 estende a base determinística da v2 com escalabilidade horizontal e domínio cruzado mais rico
fluxos de trabalho. O lançamento, codinome **Nexus**, apresenta:

- Uma rede única compartilhada globalmente chamada **SORA Nexus**. Todos os pares Iroha v3 participam deste universal
  razão em vez de operar implantações isoladas. As organizações ingressam registrando seus próprios espaços de dados,
  que permanecem isolados em termos de política e privacidade, ao mesmo tempo que se ancoram no livro-razão comum.
- Uma base de código compartilhada: o mesmo repositório cria Iroha v2 (redes auto-hospedadas) e Iroha v3 (SORA Nexus).
  A configuração seleciona o modo de destino para que os operadores possam adotar os recursos Nexus sem trocar de software
  pilhas. A máquina virtual Iroha (IVM) é idêntica em ambas as versões, portanto, contratos e bytecodes Kotodama
  os artefatos são executados perfeitamente em redes auto-hospedadas e no livro-razão global Nexus.
- Produção de blocos multi-lane para processar cargas de trabalho independentes em paralelo.
- Espaços de dados (DS) que isolam ambientes de execução enquanto permanecem combináveis ​​através de âncoras na cadeia.
- O Asset Exchange Toolkit (AXT) para transferências atômicas de valores entre espaços e swaps controlados por contrato.
- Maior confiabilidade por meio de pistas Reliable Broadcast Commit (RBC), prazos determinísticos e provas
  orçamentos amostrais.

Esses recursos permanecem em desenvolvimento ativo; APIs e layouts podem evoluir antes da v3 geral
marco de disponibilidade. Consulte `nexus.md`, `nexus_transition_notes.md` e `new_pipeline.md` para
detalhes em nível de engenharia.

## 2. Arquitetura multi-pista

- **Agendador:** As partições do agendador Nexus funcionam em pistas com base em identificadores de espaço de dados e
  grupos de composição. As pistas são executadas em paralelo, preservando as garantias de ordenação determinísticas dentro
  cada pista.
- **Grupos de pistas:** os espaços de dados relacionados compartilham um `LaneGroupId`, permitindo a execução coordenada para fluxos de trabalho que
  abrangem vários componentes (por exemplo, um CBDC DS e seu dApp DS de pagamento).
- **Prazos:** Cada pista rastreia prazos determinísticos (bloqueio, prova, disponibilidade de dados) para garantir
  progresso e uso limitado de recursos.
- **Telemetria:** métricas em nível de pista expõem a taxa de transferência, a profundidade da fila, as violações de prazo e o uso de largura de banda.
  Os scripts de CI afirmam a presença desses contadores para manter os painéis alinhados com o agendador.

## 3. Espaços de dados (Nexus)- **Isolamento:** Cada espaço de dados mantém sua própria via de consenso, segmento de estado mundial e armazenamento Kura. Isto
  suporta domínios de privacidade enquanto mantém o livro-razão global SORA Nexus coerente por meio de âncoras.
- **Âncoras:** Confirmações regulares produzem artefatos de âncora que resumem o estado do DS (raízes Merkle, provas,
  compromissos) e publicá-los na via global para auditabilidade.
- **Grupos de pistas e capacidade de composição:** Os espaços de dados podem declarar grupos de composição que permitem AXT atômico
  transações entre participantes aprovados. A governança controla as mudanças de membros e os períodos de ativação.
**Armazenamento codificado para eliminação:** Os instantâneos Kura e WSV adotam os parâmetros de codificação de eliminação `(k, m)` para dimensionar os dados
  disponibilidade sem sacrificar o determinismo. As rotinas de recuperação restauram fragmentos ausentes de forma determinística.

## 4. Kit de ferramentas de troca de ativos (AXT)

- **Descritor e vinculação:** Os clientes constroem descritores AXT determinísticos. As âncoras de hash `axt_binding`
  descritores para envelopes individuais, evitando a repetição e garantindo que os participantes do consenso validem byte por
  cargas úteis de byte Norito.
- **Syscalls:** O IVM expõe syscalls `AXT_BEGIN`, `AXT_TOUCH` e `AXT_COMMIT`. Os contratos declaram sua
  conjuntos de leitura/gravação por espaço de dados, permitindo que o host imponha a atomicidade entre as pistas.
- **Identificadores e épocas:** As carteiras obtêm identificadores de capacidade vinculados a `(dataspace_id, epoch_id, sub_nonce)`.
  Concorrente usa conflito deterministicamente, retornando códigos canônicos `AxtTrap` quando as restrições são
  violado.
- **Aplicação de políticas:** Os hosts principais agora derivam instantâneos de políticas AXT de manifestos do Space Directory no WSV,
  aplicação de verificações de raiz de manifesto, faixa de destino, era de ativação, sub-nonce e expiração (`current_slot >= expiry_slot`
  aborta) mesmo em hosts de teste mínimos. As políticas são codificadas pelo ID do espaço de dados e construídas a partir do catálogo de rotas para que
  os identificadores não podem escapar de sua via de emissão ou usar manifestos obsoletos.
  - Os motivos de rejeição são determinísticos: espaço de dados desconhecido, incompatibilidade de raiz de manifesto, incompatibilidade de faixa de destino,
    handle_era abaixo da ativação do manifesto, sub_nonce abaixo do limite mínimo da política, identificador expirado, falta de toque para
    o espaço de dados do identificador ou falta de provas quando necessário.
- **Provas e prazos:** Durante uma janela ativa Δ, os validadores coletam provas, amostras de disponibilidade de dados,
  e se manifesta. O não cumprimento dos prazos aborta o AXT de forma determinística com orientação para novas tentativas do cliente.
- **Integração de governança:** Os módulos de política definem quais espaços de dados podem participar do AXT, limite de taxa
  manipula e publica manifestos amigáveis ao auditor, capturando compromissos, anuladores e logs de eventos.

## 5. Faixas confiáveis ​​de Broadcast Commit (RBC)- **DA específico de pista:** pistas RBC espelham grupos de pistas, garantindo que cada pipeline de múltiplas pistas tenha dados dedicados
  garantias de disponibilidade.
- **Orçamentos de amostragem:** Os validadores seguem regras de amostragem determinísticas (`q_in_slot_per_ds`) para validar as provas
  e material de testemunhas sem coordenação central.
- **Informações sobre contrapressão:** Os eventos do marcapasso Sumeragi se correlacionam com as estatísticas de RBC para diagnosticar pistas paralisadas
  (ver `scripts/sumeragi_backpressure_log_scraper.py`).

## 6. Operações e migração

- **Plano de transição:** `nexus_transition_notes.md` descreve a migração em fases de faixa única (Iroha v2) para
  multi-lane (Iroha v3), incluindo preparação de telemetria, controle de configuração e atualizações de genesis.
- **Rede universal:** Os peers SORA Nexus executam uma pilha comum de gênese e governança. Novos operadores a bordo por
  criando um espaço de dados (DS) e satisfazendo as políticas de admissão Nexus em vez de lançar redes autônomas.
- **Configuração:** novos botões de configuração cobrem orçamentos de pista, prazos de prova, cotas AXT e metadados de espaço de dados.
  Os padrões permanecem conservadores até que os operadores optem pelo modo Nexus.
- **Testes:** Os testes Golden capturam descritores AXT, manifestos de pista e listas de syscall. Testes de integração
  (`integration_tests/tests/repo.rs`, `crates/ivm/tests/axt_host_flow.rs`) exercem fluxos ponta a ponta.
- **Ferramentas:** `kagami` ganha geração de gênese com reconhecimento de Nexus e scripts de painel validam o rendimento da pista,
  orçamentos de prova e saúde RBC.

## 7. Roteiro

- **Fase 1:** Habilite a execução de múltiplas pistas de domínio único com suporte e auditoria AXT local.
- **Fase 2:** ativar grupos de composição para AXT entre domínios autorizados e expandir a cobertura de telemetria.
- **Fase 3:** Implementação da federação de espaço de dados Nexus completa, armazenamento com código de eliminação e compartilhamento de provas avançado.

As atualizações de status estão disponíveis em `roadmap.md` e `status.md`. As contribuições alinhadas com o design Nexus devem seguir
as políticas determinísticas de execução e governança estabelecidas para v3.