---
lang: pt
direction: ltr
source: docs/source/crypto/attachments/sm_sales_usage_filing_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 14f32b40ff71fa4eef698eac80d8d7dd27104b46b84523d735d054dedea1c47a
source_last_modified: "2026-01-03T18:07:57.068055+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

Modelo de % de arquivamento de vendas e uso SM2/SM3/SM4 (销售/使用备案)
% Hyperledger Iroha Grupo de Trabalho de Conformidade
% 2026-05-06

# Instruções

Use este modelo ao registrar o uso de implantação em um escritório SCA para onshore
operadores. Forneça um envio por cluster de implementação ou espaço de dados. Atualizar
os espaços reservados com detalhes específicos do operador e anexe as evidências listadas
na lista de verificação.

Nº 1. Resumo do operador e implantação

| Campo | Valor |
|-------|-------|
| Nome do operador | {{OPERATOR_NAME }} |
| ID de registro comercial | {{REG_ID}} |
| Endereço registrado | {{ ENDEREÇO ​​}} |
| Contato principal (nome/cargo/e-mail/telefone) | {{ CONTATO }} |
| Identificador de implantação | {{ DEPLOYMENT_ID }} |
| Local(is) de implantação | {{ LOCAIS }} |
| Tipo de arquivamento | Vendas/Uso (销售/使用备案) |
| Data de depósito | {{AAAA-MM-DD }} |

# 2. Detalhes de implantação

- ID/hash de compilação de software: `{{ BUILD_HASH }}`
- Fonte de compilação: {{ BUILD_SOURCE }} (por exemplo, construído pelo operador a partir da fonte, binário fornecido pelo fornecedor).
- Data de ativação: {{ ACTIVATION_DATE }}
- Janelas de manutenção planejada: {{ MAINTENANCE_CADENCE }}
- Funções dos nós que participam da assinatura SM:
  | Nó | Função | Recursos SM habilitados | Localização do cofre de chaves |
  |------|------|---------------------|--------------------|
  | {{ NODE_ID }} | {{ PAPEL }} | {{ RECURSOS }} | {{ VAULT }} |

# 3. Controles criptográficos

- Algoritmos permitidos: {{ ALGORITMOS }} (garantir que o conjunto SM corresponda à configuração).
- Resumo principal do ciclo de vida:
  | Palco | Descrição |
  |-------|------------|
  | Geração | {{ KEY_GENERATION }} |
  | Armazenamento | {{KEY_STORAGE}} |
  | Rotação | {{KEY_ROTATION}} |
  | Revogação | {{ KEY_REVOCATION }} |
- Política de identidade distinta (`distid`): {{ DISTID_POLICY }}
- Trecho de configuração (seção `crypto`): fornece instantâneo Norito/JSON com hashes.

# 4. Telemetria e trilhas de auditoria

- Monitoramento de endpoints: {{ METRICS_ENDPOINTS }} (`/metrics`, painéis).
- Métricas registradas: `crypto.sm.verification_total`, `crypto.sm.sign_total`,
  histogramas de latência, contadores de erros.
- Política de retenção de logs: {{ LOG_RETENTION }} (≥ três anos recomendado).
- Local de armazenamento do registro de auditoria: {{ AUDIT_STORAGE }}

# 5. Resposta a incidentes e contatos

| Função | Nome | Telefone | E-mail | SLA |
|------|------|-------|-------|-----|
| Líder de operações de segurança | {{ NOME }} | {{ TELEFONE }} | {{ E-MAIL }} | {{SLA}} |
| Criptografia de plantão | {{ NOME }} | {{ TELEFONE }} | {{ E-MAIL }} | {{SLA}} |
| Jurídico/conformidade | {{ NOME }} | {{ TELEFONE }} | {{ E-MAIL }} | {{SLA}} |
| Suporte ao fornecedor (se aplicável) | {{ NOME }} | {{ TELEFONE }} | {{ E-MAIL }} | {{SLA}} |

# 6. Lista de verificação de anexos- [] Instantâneo de configuração (Norito + JSON) com hashes.
- [ ] Prova de construção determinística (hashes, SBOM, notas de reprodutibilidade).
- [] Exportações de painel de telemetria e definições de alerta.
- [ ] Plano de resposta a incidentes e documento de rotação de plantão.
- [ ] Confirmação de treinamento do operador ou recebimento do runbook.
- [] Instrução de controle de exportação espelhando artefatos entregues.
- [ ] Cópias de acordos contratuais relevantes ou isenções de apólices.

# 7. Declaração do Operador

> Confirmamos que a implantação listada acima está em conformidade com as normas comerciais da RPC
> regulamentos de criptografia, que os serviços habilitados para SM sigam o documentado
> políticas de resposta a incidentes e telemetria, e que os artefatos de auditoria serão
> retidos por pelo menos três anos.

- Signatário autorizado: ________________________
- Data: ________________________