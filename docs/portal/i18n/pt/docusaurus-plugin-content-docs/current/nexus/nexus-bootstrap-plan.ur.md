---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano nexus-bootstrap
título: Sora Nexus بوٹ اسٹریپ اور آبزرویبیلٹی
description: Nexus کے بنیادی cluster de validador کو آن لائن لانے سے پہلے SoraFS اور SoraNet خدمات شامل کرنے کا آپریشنل پلان۔
---

:::nota کینونیکل ماخذ
یہ صفحہ `docs/source/soranexus_bootstrap_plan.md` کی عکاسی کرتا ہے۔ لوکلائزڈ ورژنز پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Sora Nexus Plano de inicialização e observabilidade

##
- گورنس کیز, APIs Torii, e monitoramento de consenso کے ساتھ Validador/observador Sora Nexus نیٹ ورک کی بنیاد قائم کریں۔
- بنیادی سروسز (Torii, consenso, persistência) کو SoraFS/SoraNet piggyback implantações سے پہلے ویلیڈیٹ کریں۔
- Fluxos de trabalho de CI/CD e painéis/alertas de observabilidade.

## پیشگی شرائط
- Material chave de governança (multisig do conselho, chaves do comitê) HSM یا Vault میں دستیاب ہو۔
- بنیادی انفراسٹرکچر (clusters Kubernetes ou nós bare-metal).
- اپ ڈیٹ شدہ configuração de bootstrap (`configs/nexus/bootstrap/*.toml`) e parâmetros de consenso دکھائے۔

## نیٹ ورک ماحول
- Nos ambientes Nexus کو الگ نیٹ ورک prefixos کے ساتھ چلائیں:
- **Sora Nexus (mainnet)** - پروڈکشن نیٹ ورک prefixo `nexus` e governança canônica اور SoraFS/SoraNet piggyback services میزبان بناتا ہے (ID da cadeia `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - staging نیٹ ورک prefixo `testus` جو configuração mainnet کو teste de integração اور validação de pré-lançamento کے لئے espelho کرتا ہے (cadeia UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- ہر ambiente کے لئے الگ arquivos genesis, chaves de governança, e pegadas de infraestrutura رکھیں۔ Testus تمام SoraFS/SoraNet rollouts کے لئے campo de provas ہے, Nexus میں promover کرنے سے پہلے۔
- Pipelines CI/CD پہلے Testus پر implantar کریں, testes de fumaça automatizados چلائیں, e verificações پاس ہونے پر Nexus میں promoção manual درکار ہو۔
- Pacotes de configuração de referência `configs/soranexus/nexus/` (mainnet) e `configs/soranexus/testus/` (testnet) کے تحت ہیں, ہر ایک میں نمونہ `config.toml`, `genesis.json` Diretórios de admissão Torii شامل ہیں۔

## Passo 1 - Revisão de configuração
1. Qual documentação e auditoria کریں:
   - `docs/source/nexus/architecture.md` (consenso, layout Torii).
   - `docs/source/nexus/deployment_checklist.md` (requisitos de infra-estrutura).
   - `docs/source/nexus/governance_keys.md` (procedimentos de custódia de chaves).
2. Arquivos Genesis (`configs/nexus/genesis/*.json`) کو validar کریں کہ وہ موجودہ lista de validadores اور pesos de piquetagem سے alinhar ہیں۔
3. Quais são os parâmetros necessários para definir:
   - Tamanho do comitê de consenso e quórum.
   - Intervalo de bloqueio / limites de finalidade.
   - Portas de serviço Torii e certificados TLS.

## Passo 2 - Implantação de cluster Bootstrap
1. Provisão de nós validadores کریں:
   - Instâncias `irohad` (validadores) کو volumes persistentes کے ساتھ implantar کریں۔
   - نیٹ ورک regras de firewall کو یقینی بنائیں کہ consenso اور Torii nós de tráfego کے درمیان permitido ہو۔
2. Validador de serviços Torii (REST/WebSocket) e TLS کے ساتھ شروع کریں۔
3. اضافی resiliência کے لئے nós observadores (somente leitura) implantar کریں۔
4. Scripts de bootstrap (`scripts/nexus_bootstrap.sh`) چلائیں تاکہ genesis تقسیم ہو, consenso شروع ہو, اور nós رجسٹر ہوں۔
5. Testes de fumaça:
   - Torii کے ذریعے envio de transações de teste کریں (`iroha_cli tx submit`).
   - Telemetria کے ذریعے produção de blocos/verificação de finalidade کریں۔
   - Validadores/observadores کے درمیان replicação de razão چیک کریں۔

## Passo 3 - Governança e Gerenciamento de Chaves
1. Configuração multisig do conselho لوڈ کریں؛ تصدیق کریں کہ propostas de governança apresentadas اور ratificar ہو سکتی ہیں۔
2. Chaves de consenso/comitê کو محفوظ رکھیں؛ registro de acesso کے ساتھ backups automáticos configurar کریں۔
3. Procedimentos de rotação de chave de emergência (`docs/source/nexus/key_rotation.md`) سیٹ اپ کریں اور verificação de runbook کریں۔

## Passo 4 - Integração CI/CD
1. Configuração de pipelines:
   - Imagens Validator/Torii construídas e publicadas (GitHub Actions ou GitLab CI).
   - Validação automatizada de configuração (lint genesis, verificação de assinaturas).
   - Pipelines de implantação (Helm/Kustomize) para preparação e clusters de produção.
2. CI میں testes de fumaça شامل کریں (cluster efêmero اٹھائیں, conjunto de transações canônicas چلائیں).
3. Principais implantações para scripts de reversão e runbooks## Passo 5 - Observabilidade e Alertas
1. Pilha de monitoramento (Prometheus + Grafana + Alertmanager) e região میں implantar کریں۔
2. Métricas principais:
  -`nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Torii para serviços de consenso کے لئے Loki/ELK کے ذریعے logs۔
3. Painéis:
   - Saúde do consenso (altura do bloco, finalidade, status dos pares).
   - Taxas de latência/erro da API Torii.
   - Transações de governança e status de propostas.
4. Alertas:
   - Bloqueio de produção de blocos (>2 intervalos de blocos).
   - Quorum de contagem de pares سے نیچے گر جائے۔
   - Picos de taxa de erro Torii.
   - Atraso na fila de propostas de governança.

## Passo 6 - Validação e transferência
1. Validação ponta a ponta:
   - Enviar proposta de governança کریں (مثلاً alteração de parâmetro).
   - Aprovação do conselho کے ذریعے processo کریں تاکہ pipeline de governança درست ہو۔
   - Diferença de estado do razão چلائیں تاکہ consistência یقینی ہو۔
2. Runbook de plantão دستاویز کریں (resposta a incidentes, failover, escalonamento).
3. SoraFS/SoraNet ٹیموں کو prontidão بتائیں؛ تصدیق کریں کہ implantações nas costas Nexus nós کو ponto کر سکتے ہیں۔

## Lista de verificação de implementação
- [] Auditoria de gênese/configuração مکمل۔
- [] Validador ou nós observadores implantados e consenso صحت مند ہے۔
- [] Chaves de governança لوڈ ہوئے، teste de proposta ہوا۔
- [ ] Pipelines CI/CD چل رہے ہیں (construção + implantação + testes de fumaça).
- [] Painéis de observabilidade فعال ہیں اور alertando موجود ہے۔
- [ ] Transferência de documentação downstream ٹیموں کو دے دی گئی۔