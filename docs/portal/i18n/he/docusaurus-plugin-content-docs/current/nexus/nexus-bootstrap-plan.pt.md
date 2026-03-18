---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-bootstrap-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-bootstrap-plan
כותרת: Bootstrap e observabilidade do Sora Nexus
תיאור: Plano operational para colocar o Cluster Central de validadores Nexus מקוון anten de addicionar services SoraFS e SoraNet.
---

:::שים לב Fonte canonica
Esta pagina reflete `docs/source/soranexus_bootstrap_plan.md`. Mantenha as duas copias alinhadas ate que as versoes localizadas cheguem ao portal.
:::

# Plano de bootstrap e observabilidade do Sora Nexus

## אובייקטיביות
- בסיס לבנטאר של validadores/observadores Sora Nexus com chaves de governanca, APIs Torii e monitoramento de consenso.
- Validar servicos centrais (Torii, קונצנזו, מתמשך) antes de habilitar מפעיל את SoraFS/SoraNet.
- תהליכי עבודה של CI/CD e לוחות מחוונים/אזהרות של observabilidade para garantir a saude da rede.

## דרישות מוקדמות
- Material de chaves de governanca (multisig do conselho, chaves de comite) disponivel em HSM ou Vault.
- בסיס Infraestrutura (אשכולות Kubernetes ou nos חשופות-מתכת) em regioes primaria/secundaria.
- Configuracao de bootstrap atualizada (`configs/nexus/bootstrap/*.toml`) refletindo os parametros de consenso mais recentes.

## אמביינטס דה רד
- Operar dois ambientes Nexus com prefixes de red distintos:
- **Sora Nexus (mainnet)** - prefixo de rede de producao `nexus`, hospedando governanca canonica e servicos piggyback SoraFS/SoraNet (מזהה שרשרת Prometheus /U00NI000 `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - קידומת דה רד דה סטaging `testus`, espelhando a configuracao da mainnet para testes de integracao e validacao טרום-release (שרשרת UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Manter arquivos genesis separados, chaves de governanca e footprints de infraestrutura para cada ambiente. Testus atua como campo de provas para rollouts SoraFS/SoraNet antes de promotor para Nexus.
- Pipelines de CI/CD devem fazer לפרוס את פרימיירו ב-Testus, ביצוע בדיקות עשן אוטומטיות e exigir promocao manual para Nexus quando os checks passerem.
- Bundles de configuracao de referencia vivem em `configs/soranexus/nexus/` (mainnet) e `configs/soranexus/testus/` (testnet), cada um contendo `config.toml`, `genesis.json` e diretorios de admiss I10100X explosion.

## Etapa 1 - Revisao de configuracao
1. Auditar documentacao existente:
   - `docs/source/nexus/architecture.md` (קונצנזו, פריסה דה Torii).
   - `docs/source/nexus/deployment_checklist.md` (דרישות אינפראסטרוטורה).
   - `docs/source/nexus/governance_keys.md` (procedimentos de custodia de chaves).
2. Validar que arquivos genesis (`configs/nexus/genesis/*.json`) alinham com o roster atual de validadores e pesos de staking.
3. Confirmer parametros de rede:
   - Tamanho do comite de consenso e quorum.
   - Intervalo de blocos / limites de finalidade.
   - Portas do servico Torii ואישורי TLS.## Etapa 2 - פרוס לעשות אתחול של אשכול
1. Provisionar nos validadores:
   - פריסת נפחי `irohad` (validadores) com מתמידים.
   - Garantir que regras de Firewall permitam trafego de consenso e Torii entre nos.
2. פתח שירותים Torii (REST/WebSocket) עם קוד validador com TLS.
3. פרוס את ה-nos observadores (somente leitura) para resiliencia adicional.
4. מנהלי סקריפטים של bootstrap (`scripts/nexus_bootstrap.sh`) עבור תחילתו של הפצה, הוסיפו הסכמה ומספרי רשם.
5. ביצוע בדיקות עשן:
   - Enviar transacoes de teste via Torii (`iroha_cli tx submit`).
   - אימות producao/finalidade de blocos באמצעות טלמטריה.
   - Checar replicacao do Ledger entre validadores/observadores.

## Etapa 3 - Governanca e gestao de chaves
1. Carregar configuracao multisig do conselho; confirmar que propostas de governanca podem ser submetidas e ratificadas.
2. Armazenar com seguranca chaves de consenso/comite; הגדרת גיבויים אוטומטיים com logging de acesso.
3. קבע את הליכי סיבוב חירום (`docs/source/nexus/key_rotation.md`) ובדוק את ספר ההפעלה.

## Etapa 4 - Integracao CI/CD
1. הגדר צינורות:
   - בנה e publicacao de imagens validator/Torii (GitHub Actions או GitLab CI).
   - Validacao automatizada de configuracao (מוך דה בראשית, verificacao de assinaturas).
   - פריסת צינורות (Helm/Customize) עבור אשכולות לביצוע ויצור.
2. בדיקות עשן מיושמות ללא CI (subir cluster efemero, rodar suite canonica de transacoes).
3. סקריפטים נוספים להחזרה לפריסת ספרי ריצה דוקומנטריים.

## Etapa 5 - Observabilidade e alertas
1. פרוס את ה-Stack de Monitoramento (Prometheus + Grafana + Alertmanager) עבור השליטה.
2. Coletar metricas centrais:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - יומנים דרך Loki/ELK para servicos Torii e consenso.
3. לוחות מחוונים:
   - Saude do consenso (altura de bloco, finalidade, status de peers).
   - Latencia e taxa de ro da API Torii.
   - Transacoes de governanca e status de propostas.
4. התראות:
   - Parada de producao de blocos (>2 intervalos de bloco).
   - Queda no numero de peers abaixo do quorum.
   - Picos na taxa de ro de Torii.
   - Backlog da fila de propostas de governanca.

## Etapa 6 - Validacao e מסירה
1. Rodar validacao מקצה לקצה:
   - Submeter proposta de governanca (לדוגמה mudanca de parametro).
   - Processar a aprovacao do conselho para garantir que o pipeline de governanca funciona.
   - Rodar diff de estado do Ledger para garantir consistentencia.
2. ספר דוקומנטרי או ריצה עבור כוננות (תשובה לאירועים, כשל בכשל, קנה מידה).
3. Comunicar prontidao para equipes SoraFS/SoraNet; confirmar que פורס piggyback podem apontar para nos Nexus.## רשימת בדיקה ליישום
- [ ] Auditoria de genesis/configuracao concluida.
- [ ] Nos validadores e observadores deployados com consenso saudavel.
- [ ] Chaves de governanca carregadas, proposta testada.
- [ ] Pipelines CI/CD rodando (בנייה + פריסה + בדיקות עשן).
- [ ] לוחות מחוונים de observabilidade ativos com alertas.
- [ ] Documentacao de handoff entregue aos פעמים במורד הזרם.