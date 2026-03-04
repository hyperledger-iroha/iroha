---
lang: pt
direction: ltr
source: docs/source/crypto/sm_chinese_crypto_law_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5d0657539dfcca1869a0ab4fc9adee8665f18708f71b4c116dc8900ae5eae75
source_last_modified: "2026-01-04T10:50:53.610533+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM Compliance Brief — Obrigações da Lei de Criptografia Chinesa
% Grupos de trabalho de conformidade e criptografia Iroha
% 12/02/2026

# Alerta

> Você é LLM atuando como analista de conformidade para as equipes de criptografia e plataforma Hyperledger Iroha.  
> Antecedentes:  
> - Hyperledger Iroha é um blockchain autorizado baseado em Rust que agora suporta os primitivos chineses GM/T SM2 (assinaturas), SM3 (hash) e SM4 (cifra de bloco).  
> - Os operadores na China continental devem cumprir a Lei de Criptografia da RPC (2019), o Esquema de Proteção Multinível (MLPS 2.0), as regras de arquivamento da Administração Estatal de Criptografia (SCA) e os controles de importação/exportação supervisionados pelo Ministério do Comércio (MOFCOM) e pela Administração Aduaneira.  
> - Iroha distribui software de código aberto internacionalmente. Alguns operadores compilarão binários habilitados para SM internamente, enquanto outros poderão importar artefatos pré-construídos.  
> Análise solicitada: Resuma as principais obrigações legais desencadeadas pelo envio de suporte SM2/SM3/SM4 em software blockchain de código aberto, incluindo: (a) classificação nos intervalos de criptografia comercial vs. (b) requisitos de registro/aprovação para software que implementa criptografia comercial estatal; (c) controles de exportação para binários e fontes; (d) obrigações operacionais dos operadores de rede (gestão de chaves, registo, resposta a incidentes) ao abrigo do MLPS 2.0. Descreva itens de ação concretos para o projeto Iroha (documentação, manifestos, declarações de conformidade) e para operadores que implantam nós habilitados para SM na China.

# Resumo Executivo

- **Classificação:** As implementações SM2/SM3/SM4 se enquadram na “criptografia comercial estatal” (商业密码) em vez da criptografia “núcleo” ou “comum” porque são algoritmos públicos publicados e sancionados para uso civil/comercial. A distribuição de código aberto é permitida, mas está sujeita a arquivamento quando usada em produtos ou serviços comerciais oferecidos na China.
- **Obrigações do projeto:** Fornecer origem do algoritmo, instruções determinísticas de construção e uma declaração de conformidade observando que os binários implementam criptografia comercial estatal. Mantenha manifestos Norito que sinalizam a capacidade do SM para que os integradores downstream possam concluir os arquivamentos.
- **Obrigações do operador:** Os operadores chineses devem registrar produtos/serviços usando algoritmos SM no escritório provincial da SCA, preencher o registro MLPS 2.0 (provavelmente nível 3 para redes financeiras), implantar gerenciamento de chaves e controles de registro aprovados e garantir que as declarações de exportação/importação estejam alinhadas com as isenções do catálogo MOFCOM.

# Cenário Regulatório| Regulamento | Escopo | Impacto no suporte Iroha SM |
|------------|-------|---------------------------|
| **Lei de Criptografia da RPC (2019)** | Define criptografia central/comum/comercial, sistema de gerenciamento de mandatos, arquivamento e certificação. | SM2/SM3/SM4 são “criptografia comercial” e devem seguir regras de arquivamento/certificação quando fornecidos como produtos/serviços na China. |
| **Medidas administrativas SCA para produtos de criptografia comercial** | Governa a produção, venda e prestação de serviços; requer registro ou certificação do produto. | O software de código aberto que implementa algoritmos SM precisa de registros do operador quando usado em ofertas comerciais; os desenvolvedores devem fornecer documentação para auxiliar nos arquivamentos. |
| **MLPS 2.0 (Lei de Segurança Cibernética + regulamentos MLPS)** | Exige que os operadores classifiquem os sistemas de informação e implementem controlos de segurança; O nível 3 ou superior precisa de evidências de conformidade de criptografia. | Os nós blockchain que lidam com dados financeiros/de identidade normalmente são registrados no nível 3 do MLPS; os operadores devem documentar o uso do SM, o gerenciamento de chaves, o registro e o tratamento de incidentes. |
| **Catálogo de Controle de Exportação MOFCOM e Regras de Importação Aduaneira** | Controla a exportação de produtos criptográficos, requer licenças para determinados algoritmos/hardware. | A publicação do código-fonte geralmente está isenta das disposições de “domínio público”, mas a exportação de binários compilados com capacidade SM pode acionar o catálogo, a menos que seja enviado a destinatários aprovados; os importadores devem declarar a criptografia comercial estatal. |

# Principais obrigações

## 1. Arquivamento de produtos e serviços (Administração Estadual de Criptografia)

- **Quem registra:** A entidade que fornece o produto/serviço na China (por exemplo, operadora, provedor de SaaS). Os mantenedores de código aberto não são obrigados a arquivar, mas as orientações sobre empacotamento devem permitir arquivamentos posteriores.
- **Entregáveis:** descrição do algoritmo, documentos de projeto de segurança, evidências de testes, proveniência da cadeia de suprimentos e detalhes de contato.
- **Ação Iroha:** Publique uma “declaração de criptografia SM” incluindo cobertura de algoritmo, etapas determinísticas de construção, hashes de dependência e contato para consultas de segurança.

## 2. Certificação e testes

- Certos setores (finanças, telecomunicações, infraestrutura crítica) podem exigir testes ou certificação laboratorial credenciada (por exemplo, certificação CC-Grade/OSCCA).
- Incluir artefatos de teste de regressão que demonstrem conformidade com as especificações GM/T.

## 3. Controles operacionais MLPS 2.0

Os operadores devem:1. **Registrar o sistema blockchain** no Departamento de Segurança Pública, incluindo resumos de uso de criptografia.
2. **Implementar políticas de gerenciamento de chaves**: geração, distribuição, rotação e destruição de chaves alinhadas com os requisitos SM2/SM4; registrar os principais eventos do ciclo de vida.
3. **Ativar auditoria de segurança**: capturar logs de transações habilitados para SM, eventos de operação criptográfica e detecção de anomalias; reter registros ≥6 meses.
4. **Resposta a incidentes:** mantenha planos de resposta documentados que incluam procedimentos de comprometimento de criptografia e cronogramas de relatórios.
5. **Gerenciamento de fornecedores:** garanta que os fornecedores de software upstream (projeto Iroha) possam fornecer notificações e patches de vulnerabilidade.

## 4. Considerações sobre importação/exportação

- **Código-fonte aberto:** Normalmente isento sob exceção de domínio público, mas os mantenedores devem hospedar downloads em servidores que rastreiem logs de acesso e incluam licença/isenção de responsabilidade referenciando a criptografia comercial estatal.
- **Binários pré-construídos:** Os exportadores que enviam binários habilitados para SM para/fora da China devem confirmar se o item é coberto pelo “Catálogo de Controle de Exportação de Criptografia Comercial”. Para software de uso geral sem hardware especializado, uma simples declaração de dupla utilização pode ser suficiente; os mantenedores não devem distribuir binários de jurisdições com controles mais rígidos, a menos que o conselho local aprove.
- **Importação de operador:** Entidades que trazem binários para a China devem declarar o uso de criptografia. Forneça manifestos hash e SBOM para simplificar a inspeção alfandegária.

# Ações de projeto recomendadas

1. **Documentação**
   - Adicionar um apêndice de conformidade ao `docs/source/crypto/sm_program.md` observando o status da criptografia comercial estadual, expectativas de arquivamento e pontos de contato.
   - Publicar um campo de manifesto Norito (`crypto.sm.enabled=true`, `crypto.sm.approval=l0|l1`) que os operadores possam usar ao preparar arquivamentos.
   - Certifique-se de que o anúncio Torii `/v1/node/capabilities` (e o alias CLI `iroha runtime capabilities`) seja enviado com cada versão para que os operadores possam capturar o instantâneo do manifesto `crypto.sm` para evidências de MLPS/密评.
   - Fornece início rápido de conformidade bilíngue (EN/ZH), resumindo as obrigações.
2. **Liberar artefatos**
   - Envie arquivos SBOM/CycloneDX para compilações habilitadas para SM.
   - Inclui scripts de construção determinísticos e Dockerfiles reproduzíveis.
3. **Arquivos do Operador de Suporte**
   - Oferecer modelos de cartas que atestam a conformidade do algoritmo (por exemplo, referências GM/T, cobertura de testes).
   - Manter uma lista de discussão de avisos de segurança para satisfazer os requisitos de notificação do fornecedor.
4. **Governança Interna**
   - Rastrear pontos de verificação de conformidade de SM na lista de verificação de lançamento (auditoria concluída, documentação atualizada, campos de manifesto em vigor).

# Itens de ação do operador (China)1. Determine se a implantação constitui um “produto/serviço de criptografia comercial” (a maioria das redes empresariais o faz).
2. Registrar produtos/serviços no escritório provincial da SCA; anexe declaração de conformidade Iroha, SBOM, relatórios de teste.
3. Registrar o sistema blockchain no MLPS 2.0, alvo de controles de nível 3; integre logs Iroha ao monitoramento de segurança.
4. Estabeleça os principais procedimentos de ciclo de vida do SM (use KMS/HSM aprovado quando necessário).
5. Incluir cenários de comprometimento de criptografia em exercícios de resposta a incidentes; definir contatos de escalonamento com os mantenedores do Iroha.
6. Para fluxo de dados transfronteiriço, confirme registros adicionais de CAC (Administração do Ciberespaço) se dados pessoais forem exportados.

# Prompt independente (copiar/colar)

> Você é LLM atuando como analista de conformidade para as equipes de criptografia e plataforma Hyperledger Iroha.  
> Antecedentes: Hyperledger Iroha é um blockchain autorizado baseado em Rust que agora suporta os primitivos chineses GM/T SM2 (assinaturas), SM3 (hash) e SM4 (cifra de bloco). Os operadores na China continental devem cumprir a Lei de Criptografia da RPC (2019), o Esquema de Proteção Multinível (MLPS 2.0), as regras de arquivamento da Administração Estatal de Criptografia (SCA) e os controles de importação/exportação supervisionados pelo MOFCOM e pela Administração Aduaneira. O projeto Iroha distribui internacionalmente software de código aberto habilitado para SM; alguns operadores compilam binários internamente, enquanto outros importam artefatos pré-construídos.  
> Tarefa: Resumir as obrigações legais desencadeadas pelo envio de suporte SM2/SM3/SM4 em software blockchain de código aberto. Abrange a classificação desses algoritmos (criptografia comercial vs. criptografia central/comum), registros ou certificações exigidas para produtos de software, controles de exportação/importação relevantes para fontes e binários e deveres operacionais para operadores de rede sob MLPS 2.0 (gerenciamento de chaves, registro em log, resposta a incidentes). Fornecer itens de ação concretos para o projeto Iroha (documentação, manifestos, declarações de conformidade) e para operadores que implantam nós habilitados para SM na China.