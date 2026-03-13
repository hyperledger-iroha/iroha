---
lang: pt
direction: ltr
source: docs/source/crypto/sm_audit_success_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ef9305dc14d477a616923c80445094c692bc6a38d69465f679b54ccd52e92
source_last_modified: "2026-01-03T18:07:57.081283+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Critérios de sucesso de auditoria SM2/SM3/SM4
Grupo de trabalho de criptografia % Iroha
% 30/01/2026

# Objetivo

Esta lista de verificação captura os critérios concretos necessários para um sucesso
conclusão da auditoria externa SM2/SM3/SM4. Deve ser revisto durante
início, revisitado em cada ponto de verificação de status e usado para confirmar a saída
condições antes de ativar a assinatura SM para validadores de produção.

# Preparação pré-engajamento

- [ ] Contrato assinado, incluindo escopo, resultados, confidencialidade e
      idioma de suporte à correção.
- [] A equipe de auditoria recebe acesso ao espelho do repositório, bucket de artefato CI e
      pacote de documentação listado em `docs/source/crypto/sm_audit_brief.md`.
- [] Pontos de contato confirmados com backups para cada função
      (cripto, IVM, operações de plataforma, segurança, documentos).
- [ ] As partes interessadas internas se alinham na data de lançamento prevista e congelam as janelas.
- [ ] Exportação SBOM (`cargo auditable` + CycloneDX) gerada e compartilhada.
- [] Pacote de origem de construção OpenSSL/Tongsuo preparado
      (hash de tarball de origem, script de construção, notas de reprodutibilidade).
- [] Últimos resultados de testes determinísticos capturados:
      `scripts/sm_openssl_smoke.sh`, `cargo test -p iroha_crypto sm` e
      Norito luminárias de ida e volta.
- [ ] Anúncio Torii `/v2/node/capabilities` (via `iroha runtime capabilities`) registrado, verificando os campos de manifesto `crypto.sm` e o instantâneo da política de aceleração.

# Execução de Engajamento

- [ ] Workshop inicial concluído com compreensão compartilhada dos objetivos,
      cronogramas e cadência de comunicação.
- [ ] Relatórios semanais de status recebidos e triados; cadastro de risco atualizado.
- [] Descobertas comunicadas dentro de um dia útil após a descoberta quando a gravidade
      é Alto ou Crítico.
- [ ] A equipe de auditoria valida caminhos de determinismo em ≥2 arquiteturas de CPU (x86_64,
      aarch64) com saídas correspondentes.
- [] A revisão do canal lateral inclui provas em tempo constante ou testes empíricos
      evidência para os caminhos Rust e FFI.
- [] A revisão da conformidade e da documentação confirma as correspondências das orientações do operador
      obrigações regulatórias.
- [] Testes diferenciais contra implementações de referência (RustCrypto,
      OpenSSL/Tongsuo) executado com supervisão de auditor.
- [ ] Chicotes Fuzz avaliados; novos corpora de sementes fornecidos onde existirem lacunas.

# Correção e saída

- [] Todas as descobertas categorizadas com gravidade, impacto, explorabilidade e
      etapas de correção recomendadas.
- [] Problemas altos/críticos recebem patches ou mitigações com aprovação do auditor
      verificação; riscos residuais documentados.
- [ ] Auditor fornece validação de reteste evidenciando problemas corrigidos (diff, teste
      execuções ou atestado assinado).
- [ ] Relatório final entregue: resumo executivo, resultados detalhados, metodologia,
      veredicto de determinismo, veredicto de conformidade.
- [ ] Reunião interna de aprovação conclui próximas etapas, libera ajustes,
      e atualizações de documentação.
- [] `status.md` atualizado com resultado de auditoria e remediação pendente
      acompanhamentos.
- [] Post-mortem capturado em `docs/source/crypto/sm_program.md` (lições
      aprendidas, futuras tarefas de endurecimento).