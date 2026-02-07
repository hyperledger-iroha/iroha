---
lang: pt
direction: ltr
source: docs/source/crypto/sm_audit_vendor_landscape.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f39199767280be0fdd582301cdc3e8929497cf372a96f9f300e718f827000a7
source_last_modified: "2026-01-03T18:07:57.038484+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Cenário do Fornecedor de Auditoria SM
Grupo de trabalho de criptografia % Iroha
% 12/02/2026

# Visão geral

O Crypto Working Group precisa de uma bancada permanente de revisores independentes que
compreender a criptografia Rust e os padrões chineses GM/T (SM2/SM3/SM4).
Esta nota cataloga empresas com referências relevantes e resume a auditoria
escopo que normalmente solicitamos para que os ciclos de solicitação de proposta (RFP) permaneçam rápidos e
consistente.

# Empresas Candidatas

## Trilha de Bits (Prática de Criptografia CN)

- Compromissos documentados: revisão de segurança de 2023 do Tongsuo do Ant Group
  (distribuição OpenSSL habilitada para SM) e auditorias repetidas de servidores baseados em Rust
  blockchains como Diem/Libra, Sui e Aptos.
- Pontos fortes: equipe dedicada de criptografia Rust, automação em tempo constante
  ferramentas de análise, experiência na validação de execução determinística e hardware
  políticas de despacho.
- Adequado para Iroha: pode estender o SOW de auditoria SM atual ou realizar tarefas independentes
  retestes; operação confortável com equipamentos Norito e syscall IVM
  superfícies.

## Grupo NCC (serviços de criptografia APAC)

- Compromissos documentados: exames de código GM/T (SM) para pagamento regional
  fornecedores de redes e HSM; análises anteriores de Rust para Parity Substrate, Polkadot,
  e componentes de Libra.
- Pontos fortes: grande bancada APAC com reportagem bilíngue, capacidade de combinar
  verificações de processos de conformidade com revisão profunda do código.
- Adequado para Iroha: ideal para avaliações de segunda opinião ou orientadas para governança
  validação junto com as descobertas da Trail of Bits.

## Laboratórios SECBIT (Pequim)

- Compromissos documentados: mantenedores da caixa Rust `libsm` de código aberto
  usado pela Nervos CKB e CITA; auditou a capacitação de Guomi para Nervos, Muta e
  Componentes FISCO BCOS Rust com resultados bilíngues.
- Pontos fortes: engenheiros que enviam ativamente primitivos SM em Rust, fortes
  capacidades de teste de propriedade, profunda familiaridade com conformidade nacional
  requisitos.
- Adequado para Iroha: valioso quando precisamos de revisores que possam fornecer comparativos
  vetores de teste e orientações de implementação junto com as descobertas.

## Segurança SlowMist (Chengdu)

- Compromissos documentados: análises de segurança do Substrate/Polkadot Rust, incluindo
  Garfos Guomi para operadoras chinesas; avaliações de rotina da carteira SM2/SM3/SM4
  e código de ponte usado pelas exchanges.
- Pontos fortes: prática de auditoria focada em blockchain, resposta integrada a incidentes,
  orientação que abrange o código do protocolo principal e as ferramentas do operador.
- Adequado para Iroha: útil para validar a paridade do SDK e pontos de contato operacionais
  além de caixas centrais.

## Chaitin Tech (Laboratório de Segurança QAX 404)- Engajamentos documentados: contribuidores para a proteção GmSSL/Tongsuo e SM2/SM3/
  Orientação de implementação do SM4 para instituições financeiras nacionais; estabelecido
  Prática de auditoria de ferrugem cobrindo pilhas TLS e bibliotecas criptográficas.
- Pontos fortes: experiência profunda em criptoanálise, capacidade de emparelhar verificação formal
  artefatos com revisão manual, relacionamentos reguladores de longa data.
Adequado para Iroha: adequado para aprovação regulatória ou artefatos de prova formal
  precisa acompanhar o relatório de revisão de código padrão.

# Escopo e resultados típicos de auditoria

- **Conformidade das especificações:** validar cálculo e assinatura do SM2 ZA
  canonização, preenchimento/compactação SM3 e programação de chave SM4 e manipulação IV
  contra GM/T 0003-2012, GM/T 0004-2012 e GM/T 0002-2012.
- **Determinismo e comportamento em tempo constante:** examine ramificação, pesquisa
  tabelas e portas de recursos de hardware (por exemplo, instruções NEON, SM4) para garantir
  O despacho Rust e FFI permanece determinístico em todo o hardware suportado.
- **Integração de FFI e provedor:** revise as ligações OpenSSL/Tongsuo,
  Adaptadores PKCS#11/HSM e caminhos de propagação de erros para segurança de consenso.
**Cobertura de testes e acessórios:** avaliar chicotes fuzz, viagens de ida e volta Norito,
  testes determinísticos de fumaça e recomendam testes diferenciais onde lacunas
  aparecer.
- **Dependência e análise da cadeia de suprimentos:** confirme a origem da construção, o fornecedor
  políticas de patch, precisão do SBOM e instruções de construção reproduzíveis.
- **Documentação e operações:** validar runbooks do operador, conformidade
  resumos, padrões de configuração e procedimentos de reversão.
- **Expectativas de relatório:** resumo executivo com classificação de risco, detalhado
  descobertas com referências de código e orientações de correção, plano de novo teste e
  atestados que cobrem garantias de determinismo.

# Próximas etapas

- Use esta lista de fornecedores durante os ciclos de RFQ; ajuste a lista de verificação de escopo acima para
  corresponder ao marco SM ativo antes de emitir uma RFP.
- Registrar resultados de engajamento em `docs/source/crypto/sm_audit_brief.md` e
  atualizações de status de superfície em `status.md` assim que os contratos são executados.