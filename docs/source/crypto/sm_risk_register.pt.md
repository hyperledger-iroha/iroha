---
lang: pt
direction: ltr
source: docs/source/crypto/sm_risk_register.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ba5f4fdc9221210a793fd0c2120d8cfb68487d7ddcbe67c208976798446ca5db
source_last_modified: "2026-01-03T18:07:57.078074+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Registro de risco do programa SM para ativação de SM2/SM3/SM4.

# Registro de Risco do Programa SM

Última atualização: 12/03/2025.

Este registro expande o resumo em `sm_program.md`, emparelhando cada risco com
propriedade, gatilhos de monitoramento e estado atual de mitigação. O Grupo de Criptomoedas
e os líderes da Plataforma Central revisam esse registro na cadência SM semanal; mudanças
estão refletidos aqui e no roteiro público.

## Resumo de risco

| ID | Risco | Categoria | Probabilidade | Impacto | Gravidade | Proprietário | Mitigação | Estado | Gatilhos |
|----|------|----------|------------|--------|----------|-------|------------|--------|----------|
| R1 | Auditoria externa para caixas RustCrypto SM não executadas antes da assinatura do validador GA | Cadeia de abastecimento | Médio | Alto | Alto | GT de criptografia | Trilha de contrato do Grupo Bits/NCC, manter postura de verificação até que o relatório seja aceito | Mitigação em andamento | SOW de auditoria não assinado até 15/04/2025 ou relatório de auditoria adiado após 01/06/2025 |
| R2 | Regressões nonce determinísticas entre SDKs | Implementação | Médio | Alto | Alto | Líderes do programa SDK | Compartilhe equipamentos no SDK CI, aplique a codificação canônica r∥s, adicione testes de violação entre SDKs | Monitoramento | Desvio de luminária detectado na versão CI ou SDK sem luminárias SM |
| R3 | Bugs específicos do ISA em intrínsecos (NEON/SIMD) | Desempenho | Baixo | Médio | Médio | GT Desempenho | Bloqueie intrínsecos por trás dos sinalizadores de recursos, exija cobertura de CI no ARM, mantenha substituto escalar | Mitigação em andamento | Bancos NEON falham ou regressão de hardware descoberta na matriz de desempenho SM |
| R4 | Ambiguidade de conformidade atrasa adoção de SM | Governança | Médio | Médio | Médio | Documentos e contato jurídico | Publicar resumo de conformidade, lista de verificação do operador e contato com aconselhamento jurídico antes da GA | Mitigação em andamento | Revisão jurídica pendente após 01/05/2025 ou atualizações da lista de verificação ausentes |
| R5 | Desvio de back-end FFI com atualizações do provedor | Integração | Médio | Médio | Médio | Operações de plataforma | Fixar versões do provedor, adicionar testes de paridade, manter a opção de visualização do OpenSSL/Tongsuo | Monitoramento | Atualização de pacote mesclada sem execução de paridade ou visualização habilitada fora do escopo piloto |

## Revise a cadência

- Sincronização semanal do Crypto WG (item permanente da agenda).
- Revisão conjunta mensal com Platform Ops e Docs para confirmar a postura de conformidade.
- Ponto de verificação de pré-lançamento: congelamento de registro de risco e atestado junto com GA
  artefatos.

## Assinatura

| Função | Representante | Data | Notas |
|------|----------------|------|-------|
| Líder do Crypto WG | (assinatura em arquivo) | 12/03/2025 | Aprovado para publicação e compartilhado com o backlog do WG. |
| Líder da plataforma principal | (assinatura em arquivo) | 12/03/2025 | Mitigações aceitas e cadência de monitoramento. |

Para histórico de aprovações e atas de reuniões, consulte `docs/source/crypto/sm_program.md`
(`Communication Plan`) e o arquivo da agenda SM vinculado ao Crypto WG
espaço de trabalho.