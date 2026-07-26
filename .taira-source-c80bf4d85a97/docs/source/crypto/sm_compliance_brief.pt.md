---
lang: pt
direction: ltr
source: docs/source/crypto/sm_compliance_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 73f5ca7a7484a26e901102dd6950b7110a18e7fa215a46540c7189c919e0958f
source_last_modified: "2026-01-03T18:07:57.080817+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Resumo de conformidade e exportação de SM2/SM3/SM4

Este resumo complementa as notas de arquitetura em `docs/source/crypto/sm_program.md`
e fornece orientação prática para equipes de engenharia, operações e jurídicas como o
A família de algoritmos GM/T passa da visualização somente de verificação para uma habilitação mais ampla.

## Resumo
- **Base regulatória:** *Lei de criptografia* da China (2019), *Lei de segurança cibernética* e
  *Lei de Segurança de Dados* classifica SM2/SM3/SM4 como “criptografia comercial” quando implantada
  em terra. Os operadores devem arquivar relatórios de uso, e certos setores exigem credenciados
  testes antes do uso em produção.
- **Controles internacionais:** Fora da China, os algoritmos se enquadram na categoria EAR dos EUA
  5 Parte 2, Anexo 1 da UE 2021/821 (5D002) e regimes nacionais semelhantes. Código aberto
  publicação normalmente se qualifica para exceções de licença (ENC/TSU), mas binários
  enviados para regiões embargadas continuam sendo exportações controladas.
- **Política do projeto:** os recursos do SM permanecem desativados por padrão. Funcionalidade de assinatura
  só será habilitado após encerramento da auditoria externa, desempenho determinístico/telemetria
  portão e documentação do operador (este resumo) terreno.

## Ações necessárias por função
| Equipe | Responsabilidades | Artefatos | Proprietários |
|------|------------------|-----------|--------|
| GT de criptografia | Rastreie atualizações de especificações GM/T, coordene auditorias de terceiros, mantenha políticas determinísticas (derivação de nonce, r∥s canônicos). | `sm_program.md`, relatórios de auditoria, pacotes de acessórios. | Líder do Crypto WG |
| Engenharia de Liberação | Recursos do Gate SM por trás da configuração explícita, mantêm o padrão somente verificação e gerenciam a lista de verificação de implementação de recursos. | `release_dual_track_runbook.md`, manifestos de lançamento, ticket de lançamento. | Liberar TL |
| Operações/SRE | Fornece lista de verificação de ativação de SM, painéis de telemetria (uso, taxas de erro), plano de resposta a incidentes. | Runbooks, painéis Grafana, tickets de integração. | Operações/SRE |
| Contato Jurídico | Arquivar relatórios de desenvolvimento/uso da RPC quando os nós são executados na China continental; revise a postura de exportação para cada pacote. | Modelos de arquivamento, extratos de exportação. | Contacto jurídico |
| Programa SDK | O algoritmo Surface SM oferece suporte consistente, impõe comportamento determinístico e propaga notas de conformidade para documentos do SDK. | Notas de versão do SDK, documentos, controle de CI. | Leads do SDK |## Requisitos de documentação e arquivamento (China)
1. **Arquivo do produto (开发备案):** Para desenvolvimento onshore, envie a descrição do produto,
   declaração de disponibilidade de origem, lista de dependências e etapas de construção determinísticas para
   a administração provincial de criptografia antes do lançamento.
2. **Arquivo de vendas/uso (销售/使用备案):** Os operadores que executam nós habilitados para SM devem
   registrar escopo de uso, gerenciamento de chaves e coleta de telemetria com o mesmo
   autoridade. Forneça informações de contato e SLAs de resposta a incidentes.
3. **Certificação (检测/认证):** Operadores de infraestrutura crítica podem exigir
   testes credenciados. Forneça scripts de construção reproduzíveis, SBOM e relatórios de teste
   para que os integradores downstream possam concluir a certificação sem alterar o código.
4. **Manutenção de registros:** Arquive registros e aprovações no rastreador de conformidade.
   Atualize `status.md` quando novas regiões ou operadoras concluírem o processo.

## Lista de verificação de conformidade

### Antes de ativar os recursos do SM
- [ ] Confirme que o consultor jurídico revisou as regiões de implantação alvo.
- [] Capture instruções determinísticas de construção, manifestos de dependência e SBOM
      exportações para inclusão em limalhas.
- [] Alinhar `crypto.allowed_signing`, `crypto.default_hash` e admissão
      a política se manifesta com o ticket de implementação.
- [ ] Produzir comunicações do operador descrevendo o escopo dos recursos SM,
      pré-requisitos de habilitação e planos alternativos para desabilitação.
- [] Exportar painéis de telemetria cobrindo contadores de verificação/assinatura SM,
      taxas de erro e métricas de desempenho (`sm3`, `sm4`, tempo de syscall).
- [] Preparar contatos de resposta a incidentes e caminhos de escalonamento para onshore
      operadoras e o Crypto WG.

### Preparação para arquivamento e auditoria
- [ ] Selecione o modelo de arquivamento apropriado (produto x vendas/uso) e preencha
      nos metadados da versão antes do envio.
- [] Anexe arquivos SBOM, transcrições de testes determinísticos e hashes de manifesto.
- [ ] Garantir que a declaração de controle de exportação reflita os artefatos exatos que estão sendo
      entregue e cita exceções de licença nas quais se baseia (ENC/TSU).
- [ ] Verifique se os relatórios de auditoria, o rastreamento de remediação e os runbooks do operador
      estão vinculados a partir do pacote de arquivamento.
- [] Armazenar registros assinados, aprovações e correspondência no compliance
      rastreador com referências versionadas.

### Operações pós-aprovação
- [ ] Atualize `status.md` e o ticket de lançamento assim que o arquivamento for aceito.
- [] Execute novamente a validação de telemetria para confirmar correspondências de cobertura de observabilidade
      as entradas de arquivamento.
- [ ] Programar revisão periódica (pelo menos anualmente) de arquivamentos, relatórios de auditoria,
      e exportar instruções para capturar atualizações de especificações/regulamentações.
- [] Acionar adendos de arquivamento sempre que houver configuração, escopo de recurso ou hospedagem
      a pegada muda materialmente.## Orientação de exportação e distribuição
- Incluir uma breve declaração de exportação nas notas de lançamento/manifestos referenciando a confiança
  em ENC/TSU. Exemplo:
  > "Esta versão contém implementações SM2/SM3/SM4. A distribuição segue ENC
  > (15 CFR Parte 742) / UE 2021/821 Anexo 1 5D002. Os operadores devem garantir a conformidade
  > com as leis locais de exportação/importação.”
- Para compilações hospedadas na China, coordene com as operações para publicar artefatos de
  infraestrutura terrestre; evitar a transferência transfronteiriça de binários habilitados para SM, a menos que
  as licenças apropriadas estão em vigor.
- Ao espelhar para repositórios de pacotes, registre quais artefatos incluem recursos SM
  para simplificar os relatórios de conformidade.

## Lista de verificação do operador
- [] Confirmar perfil de lançamento (`scripts/select_release_profile.py`) + sinalizador de recurso SM.
- [ ] Revise `sm_program.md` e este resumo; garantir que os registros legais sejam registrados.
- [ ] Habilite os recursos SM compilando com `sm`, atualizando `crypto.allowed_signing` para incluir `sm2` e alternando `crypto.default_hash` para `sm3-256` somente depois que as proteções de determinismo estiverem em vigor e o status de auditoria estiver verde.
- [] Atualizar painéis/alertas de telemetria para incluir contadores SM (falhas de verificação,
      solicitações de assinatura, métricas de desempenho).
- [] Mantenha manifestos, provas de hash/assinatura e confirmações de arquivamento anexadas a
      o tíquete de lançamento.

## Exemplos de modelos de arquivamento

Os modelos estão sob `docs/source/crypto/attachments/` para fácil inclusão em
arquivando pacotes. Copie o modelo Markdown relevante na alteração do operador
registre ou exporte-o para PDF conforme exigido pelas autoridades locais.

- [`sm_product_filing_template.md`](attachments/sm_product_filing_template.md) —
  formulário de arquivamento de produto provincial (开发备案), capturando metadados de lançamento, algoritmos,
  Referências SBOM e contatos de suporte.
- [`sm_sales_usage_filing_template.md`](attachments/sm_sales_usage_filing_template.md) —
  registro de vendas/uso do operador (销售/使用备案) descrevendo a pegada de implantação,
  procedimentos de gerenciamento de chaves, telemetria e resposta a incidentes.
- [`sm_export_statement_template.md`](attachments/sm_export_statement_template.md) —
  declaração de controle de exportação adequada para notas de lançamento, manifestos ou informações legais
  correspondência baseada em exceções de licença ENC/TSU.## Padrões e citações
- **GM/T 0002-2012 / GB/T 32907-2016** — Cifra de bloco SM4 e parâmetros AEAD (ECB/GCM/CCM). Corresponde aos vetores capturados em `docs/source/crypto/sm_vectors.md`.
- **GM/T 0003-2012 / GB/T 32918.x-2016** — Criptografia de chave pública SM2, parâmetros de curva, processo de assinatura/verificação e testes de resposta conhecida do Anexo D.
- **GM/T 0004-2012 / GB/T 32905-2016** — Especificação da função hash SM3 e vetores de conformidade.
- **RFC 8998** — Troca de chaves SM2 e uso de assinatura em TLS; citar ao documentar a interoperabilidade com OpenSSL/Tongsuo.
- **Lei de Criptografia da República Popular da China (2019)**, **Lei de Segurança Cibernética (2017)**, **Lei de Segurança de Dados (2021)** — Base jurídica para o fluxo de trabalho de arquivamento mencionado acima.
- **EAR dos EUA Categoria 5 Parte 2** e **Regulamento UE 2021/821 Anexo 1 (5D002)** — Regimes de controle de exportação que regem binários habilitados para SM.
- **Artefatos Iroha:** `scripts/sm_interop_matrix.sh` e `scripts/sm_openssl_smoke.sh` fornecem transcrições determinísticas de interoperabilidade que os auditores podem reproduzir antes de assinar relatórios de conformidade.

## Referências
- `docs/source/crypto/sm_program.md` — arquitetura técnica e política.
- `docs/source/release_dual_track_runbook.md` — liberação de controle e processo de implementação.
- `docs/source/sora_nexus_operator_onboarding.md` — exemplo de fluxo de integração do operador.
- GM/T 0002-2012, GM/T 0003-2012, GM/T 0004-2012, série GB/T 32918, RFC 8998.

Perguntas? Entre em contato com o Crypto WG ou com o contato jurídico por meio do rastreador de implementação SM.