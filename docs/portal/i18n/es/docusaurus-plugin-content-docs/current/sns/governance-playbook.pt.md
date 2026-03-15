---
lang: es
direction: ltr
source: docs/portal/docs/sns/governance-playbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Esta página espelha `docs/source/sns/governance_playbook.md` y ahora sirve como a
copia canónica del portal. El archivo fuente permanece para PRs de traducción.
:::

# Libro de jugadas de gobierno del Servicio de nombres de Sora (SN-6)

**Estado:** Redigido 2026-03-24 - referencia viva para a prontidao SN-1/SN-6  
**Enlaces a la hoja de ruta:** SN-6 "Cumplimiento y resolución de disputas", SN-7 "Resolver & Gateway Sync", política de endereco ADDR-1/ADDR-5  
**Requisitos previos:** Esquema do registro em [`registry-schema.md`](./registry-schema.md), contrato da API do registrador em [`registrar-api.md`](./registrar-api.md), guía UX de enderecos em [`address-display-guidelines.md`](./address-display-guidelines.md), y registros de estructura de cuentas en [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Este manual describe cómo los cuerpos de gobierno del Servicio de Nombres de Sora (SNS)
adotam cartas, aprovam registros, escalam disputas e provam que os estados de
El solucionador y la puerta de enlace permanecen en sincronización. Ele cumpre o requisito do roadmap de
que a CLI `sns governance ...`, manifiestos Norito e artefatos de auditoria
compartilhem uma única referencia voltada al operador antes de N1 (lancamento
público).

## 1. Escopo y público

El documento se destina a:- Miembros del Consejo de Gobierno que votan en cartas, políticas de sufijo y
  resultados de disputa.
- Miembros del consejo de guardianes que emiten congelamentos de emergencia e
  revisam reversos.
- Stewards de sufixo que operam filas do registrador, aprovam leiloes e gerenciam
  divisao de receitas.
- Operadores de resolución/gateway responsables de la propagación de SoraDNS, actualizados
  GAR y barandillas de telemetría.
- Equipes de conformidade, tesouraria y suporte que devem demostrar que toda
  acao degobernanca deixou artefatos Norito auditaveis.

El cobre como fases beta fechada (N0), lancamento público (N1) y expansión (N2)
listadas em `roadmap.md`, vinculando cada flujo de trabajo como evidencias
necessarias, tableros de instrumentos y caminos de escalada.

## 2. Papeles y mapa de contacto| Papel | Responsabilidades principales | Artefatos y telemetria principais | Escalacao |
|-------|------------------------------|-----------------------------------|-----------|
| Consejo de Gobierno | Redige e ratifica cartas, políticas de sufixo, vereditos de disputa e rotacoes de steward. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, votos do conselho armazenados vía `sns governance charter submit`. | Presidente do conselho + rastreador de agenda de gobierno. |
| Consejo de Guardianes | Emite congelamentos blandos/duros, cañones de emergencia y revisiones de 72 h. | Tickets guardian emitidos por `sns governance freeze`, manifiestos de anulación registrados en `artifacts/sns/guardian/*`. | Guardián de Rotacao de guardia (<=15 min ACK). |
| Mayordomos de sufixo | Operam filas do registrador, leiloes, niveis de preco e comunicacao com clientes; reconhecem conformidades. | Politicas de steward em `SuffixPolicyV1`, folhas de referencia de preco, acknowledgements de steward armazenados junto a memos regulatorios. | Líder del programa Steward + PagerDuty por sufijo. |
| Operaciones de registro y cobranza | Opera los puntos finales `/v1/sns/*`, reconcilia pagos, emite telemetría y mantiene instantáneas de CLI. | API del registrador ([`registrar-api.md`](./registrar-api.md)), métricas `sns_registrar_status_total`, pruebas de pago archivadas en `artifacts/sns/payments/*`. | Gerente de turno del registrador y enlace de tesouraria. || Operadores de resolución y puerta de enlace | Mantem SoraDNS, GAR y estado del gateway alinhados com eventos do registrador; transmiten métricas de transparencia. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | SRE resolver de guardia + ponte ops do gateway. |
| Tesouraria y financieras | Aplica divisao de receita 70/30, carve-outs de referidos, registros fiscales/tesouraria y atestacoes SLA. | Manifiestos de acumulación de ingresos, exportaciones Stripe/tesouraria, apéndices KPI trimestrales en `docs/source/sns/regulatory/`. | Controller financiero + oficial de conformidad. |
| Enlace de conformidad y regulación | Acompanha obrigacoes globalis (EU DSA, etc.), actualiza los acuerdos KPI y registra divulgacoes. | Memos regulatorios em `docs/source/sns/regulatory/`, decks de referencia, entradas `ops/drill-log.md` para ensayos de mesa. | Líder del programa de conformidad. |
| Soporte / SRE de guardia | Lida com incidentes (colisoes, drift de cobranca, quedas de resolver), coordina mensajes a clientes y dono dos runbooks. | Plantillas de incidente, `ops/drill-log.md`, evidencia de laboratorio, transcricos Slack/war-room arquivadas en `incident/`. | Rotacao de guardia SNS + gestao SRE. |

## 3. Artefatos canónicos y fuentes de datos| Artefacto | Localizacao | propuesta |
|----------|------------|----------|
| Carta + adenda KPI | `docs/source/sns/governance_addenda/` | Cartas assinadas con control de versao, convenios KPI y decisiones de gobierno referenciadas por votos da CLI. |
| Esquema del registro | [`registry-schema.md`](./registry-schema.md) | Estructuras canónicas Norito (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Contrato del registrador | [`registrar-api.md`](./registrar-api.md) | Cargas útiles REST/gRPC, métricas `sns_registrar_status_total` y expectativas de gancho de gobernanza. |
| Guía UX de enderecos | [`address-display-guidelines.md`](./address-display-guidelines.md) | Renderizacoes canonicas I105 (preferido) y comprimidas (segunda melhor opcao) refletidas por wallets/explorers. |
| Documentos SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Derivacao deterministica de hosts, fluxo do tailer de transparencia e gras de alerta. |
| Memos regulatorios | `docs/source/sns/regulatory/` | Notas de entrada por jurisdicao (ex. EU DSA), agradecimientos de administrador, anexos de plantilla. |
| Registro de perforación | `ops/drill-log.md` | Registro de ensayos de caos e IR requeridos antes de dichas de fase. |
| Armazenamento de artefatos | `artifacts/sns/` | Pruebas de pago, guardián de tickets, diferencias de resolución, KPI de exportaciones y datos de CLI producidos por `sns governance ...`. |Todas las decisiones de gobierno deben referenciar pelo menos un artefato na tabela
acima para que auditores reconstruam o rastro de decisión em 24 horas.

## 4. Libros de jugadas del ciclo de vida

### 4.1 Mocoes de carta y mayordomo

| Etapa | Responsavel | CLI/Evidencia | Notas |
|-------|-------------|-----------------|-------|
| Redigir addendum y deltas KPI | Relator do conselho + líder de azafata | Plantilla Markdown armada en `docs/source/sns/governance_addenda/YY/` | Incluir ID de KPI de convenio, ganchos de telemetría y indicadores de activación. |
| Enviar propuesta | Presidente del consejo | `sns governance charter submit --input SN-CH-YYYY-NN.md` (producto `CharterMotionV1`) | Una CLI emite el manifiesto Norito salvo en `artifacts/sns/governance/<id>/charter_motion.json`. |
| Voto y reconocimiento tutor | Consejo + tutores | `sns governance ballot cast --proposal <id>` e `sns governance guardian-ack --proposal <id>` | Anexar atas hasheadas e provas de quorum. |
| Mayordomo de aceitacao | Programa de azafata | `sns governance steward-ack --proposal <id> --signature <file>` | Obrigatorio antes de mudar políticas de sufixo; Sobre de registrador en `artifacts/sns/governance/<id>/steward_ack.json`. |
| Ativacao | Operaciones de registro | Actualizar `SuffixPolicyV1`, actualizar cachés del registrador, publicar nota en `status.md`. | Marca de tiempo de activación registrada en `sns_governance_activation_total`. |
| Registro de auditorias | Conformidad | Agregar entrada en `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` y sin taladrar troncos sobre mesa. | Incluir referencias a paneles de telemetría y diferencias políticas. |

### 4.2 Aprovacoes de registro, leilao y preco1. **Preflight:** El registrador consulta `SuffixPolicyV1` para confirmar el nivel de
   preco, termos disponiveis e janelas de graca/redencao. Mantenha folhas de
   preco sincronizadas con tabla de niveles 3/4/5/6-9/10+ (nivel base +
   coeficientes de sufixo) documentada sin hoja de ruta.
2. **Leiloes oferta sellada:** Para pools premium, ejecutar o ciclo 72 h commit /
   Revelación de 24 h a través de `sns governance auction commit` / `... reveal`. público un
   Lista de confirmaciones (apenas hashes) en `artifacts/sns/auctions/<name>/commit.json`
   para que auditores verifiquem aleatoriedade.
3. **Verificacao de pago:** Registradores validam `PaymentProofV1` contra a
   divisao de tesouraria (70% tesouraria / 30% steward com carve-out de referido <=10%).
   Armazene o JSON Norito en `artifacts/sns/payments/<tx>.json` y vincule-o na
   respuesta del registrador (`RevenueAccrualEventV1`).
4. **Hook degobernanza:** Anexe `GovernanceHookV1` para nomes premium/guarded com
   referencia a ids de proposta do conselho e assinaturas de steward. Ganchos
   resultados ausentes en `sns_err_governance_missing`.
5. **Ativacao + sync do resolutor:** Assim que Torii emite o evento de registro,
   acción o tailer de transparencia do resolver para confirmar que o novo estado
   GAR/zone se propagou (veja 4.5).
6. **Divulgacao al cliente:** Actualizar el libro mayor voltado al cliente (billetera/explorador)
   a través de accesorios del sistema operativo compartidos en [`address-display-guidelines.md`](./address-display-guidelines.md),Garantizando que renderizacoes I105 y comprimidos corresponden a orientaciones de copia/QR.

### 4.3 Renovações, cobranca e reconciliacao da tesouraria- **Fluxo de renovacao:** Registradores aplicam a janela de graca de 30 días + janela
  de redencao de 60 días especificados en `SuffixPolicyV1`. Apos 60 días, un
  secuencia de reabertura holandesa (7 días, taxones 10x decaindo 15%/dia) e
  accionada automáticamente vía `sns governance reopen`.
- **Divisao de receita:** Cada renovación o transferencia cria um
  `RevenueAccrualEventV1`. Las exportaciones de tesouraria (CSV/Parquet) deben conciliarse
  eses eventos diariamente; El anexo prueba en `artifacts/sns/treasury/<date>.json`.
- **Carve-outs de referidos:** Porcentajes de referidos opcionales de los rastreados por
  sufixo ao adicionar `referral_share` a politica de steward. Los registradores emiten un
  divisao final e armazenam manifiestos de referencia al lado de la prueba de pago.
- **Cadencia de relatorios:** Financas publica anexos KPI mensais (registros,
  renovacoes, ARPU, uso de disputas/bond) em `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
  Dashboards devem puxar das mesmas tabelas exportadas para que os numeros de
  Grafana batam com como evidencias del libro mayor.
- **Revisao KPI mensal:** O checkpoint da primeira terca-feira junta o líder de
  financas, steward de plantao y PM do programa. Abra o [panel de KPI de SNS](./kpi-dashboard.md)
  (insertar en el portal de `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`),
  exportar como tablas de rendimiento + recibo del registrador, registrar deltas no
  anexo e anexe os artefatos ao memo. Acción um incidente se a revisao encontrarquebras de SLA (janelas de congelación >72 h, picos de error del registrador, deriva de ARPU).

### 4.4 Congelados, disputas y apelacoes

| Fase | Responsavel | Acao y evidencia | Acuerdo de Nivel de Servicio |
|------|-------------|------------------|-----|
| Pedido de congelación suave | Mayordomo / suporte | Abrir ticket `SNS-DF-<id>` com provas de pago, referencia do bond de disputa e seletor(es) afetados. | <=4 h desde la entrada. |
| Guardián de entradas | Guardián del Consejo | `sns governance freeze --selector <I105> --reason <text> --until <ts>` producto `GuardianFreezeTicketV1`. Armazene o JSON del ticket en `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h de ejecución. |
| Ratificaçao do conselho | Consejo de Gobierno | Aprovar ou rejeitar congelamentos, documentar decisao com link ao ticket guardian e digest do bond de disputa. | Próxima sesión del consejo o voto assincrono. |
| Panel de arbitraje | Conformidad + azafata | Convocar dolor de 7 jurados (hoja de ruta conforme) con cédulas hasheadas vía `sns governance dispute ballot`. Anexar recibos de voto anonimizados al paquete de incidente. | Veredito <=7 días apos deposito do bond. |
| Apelacao | Guardián + consejo | Apelacoes dobram o bond e repetem o processo de jurados; manifiesto del registrador Norito `DisputeAppealV1` e referenciar ticket primario. | <=10 días. |
| Descongelar y remediar | Registrador + operaciones de resolución | Ejecute `sns governance unfreeze --selector <I105> --ticket <id>`, actualice el estado del registrador y propague diferencias GAR/resolver. | Inmediatamente apos o veredito. |Canones de emergencia (congelamentos aficionados por guardian <=72 h) seguem o mesmo
fluxo, mas exigem revisao retroativa do conselho e uma nota de transparencia em
`docs/source/sns/regulatory/`.

### 4.5 Propagación de resolución y puerta de enlace

1. **Hook de evento:** Cada evento de registro emite para la transmisión de eventos
   solucionador (`tools/soradns-resolver` SSE). Ops de resolver se inscrevem e
   diferencias de registro a través del tailer de transparencia
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Actualización de plantilla GAR:** Las puertas de enlace deben actualizar plantillas GAR
   referenciados por `canonical_gateway_suffix()` y re-assinar a lista
   `host_pattern`. Armazene se diferencia en `artifacts/sns/gar/<date>.patch`.
3. **Publicación del archivo de zona:** Utilice el esqueleto del archivo de zona descrito en `roadmap.md`
   (nombre, ttl, cid, prueba) y envie para Torii/SoraFS. Archivo en JSON Norito en
   `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Cheque de transparencia:** Ejecute `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   para garantizar que os alertas sigam verdes. Anexo a dicha de texto
   Prometheus ao relatorio semanal de transparencia.
5. **Auditoria de gateway:** Registre amostras de headers `Sora-*` (politica de
   caché, CSP, resumen GAR) y anexos del registro de gobierno para los operadores
   Possam provar que o gateway serviu o novo nome com os guardrails esperados.

## 5. Telemetria y relatorios| Sinal | Fuente | Descricao / Acao |
|-------|-------|------------------|
| `sns_registrar_status_total{result,suffix}` | Controladores del registrador Torii | Contador de éxito/error para registros, renovaciones, congelamientos, transferencias; La alerta cuando `result="error"` aumenta por sufijo. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Métricas Torii | SLO de latencia para manejadores de API; Tableros de alimentación basados ​​en `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` e `soradns_bundle_cid_drift_total` | Tailer de transparencia del solucionador | Detecta pruebas obsoletas o deriva de GAR; barandillas definidas en `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | CLI de gobierno | Contador incrementado quando um charter/addendum ativa; usado para reconciliar decisiones do conselho vs addenda publicadas. |
| Calibre `guardian_freeze_active` | Guardián CLI | Acompaña las janelas de congelación blanda/dura mediante el selector; página SRE se o valor ficar `1` alem do SLA declarado. |
| Cuadros de mando de anexos KPI | Finanzas / Docs | Rollups mensais publicados junto a memos regulatorios; El portal os embute via [SNS KPI panel](./kpi-dashboard.md) para que azafatas y reguladores accedan a mi visao Grafana. |

## 6. Requisitos de evidencia y auditoria| Acao | Pruebas a archivar | Armazenamento |
|------|----------------------|---------------|
| Mudanca de carta / política | Manifiesto Norito assinado, transcripción CLI, diferencia de KPI, reconocimiento del administrador. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Registro / renovación | Carga útil `RegisterNameRequestV1`, `RevenueAccrualEventV1`, prueba de pago. | `artifacts/sns/payments/<tx>.json`, registros de la API del registrador. |
| Leilao | Manifiestos cometer/revelar, semente de aleatoriedade, planilha de calculo do vencedor. | `artifacts/sns/auctions/<name>/`. |
| Congelar / descongelar | Ticket guardián, hash de voto do conselho, URL de registro de incidente, plantilla de comunicación con cliente. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Propagación de resolución | Archivo de zona diferencial/GAR, trecho JSONL de tailer, instantánea Prometheus. | `artifacts/sns/resolver/<date>/` + relatos de transparencia. |
| Regulación de ingesta | Memo de admisión, seguimiento de plazos, reconocimiento de mayordomo, currículum de mudanzas KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Lista de verificación de puerta de fase| Fase | Criterios de dicha | Paquete de evidencia |
|------|--------------------|--------------------|
| N0 - Beta fechada | Esquema de registro SN-1/SN-2, CLI de registrador manual, taladro guardián completo. | Motion de carta + ACK Steward, logs de dry-run do registrador, relatorio de transparencia do resolutor, entrada em `ops/drill-log.md`. |
| N1 - Lanzamiento público | Leiloes + niveles de preco fixo activos para `.sora`/`.nexus`, autoservicio de registrador, sincronización automática de resolución, paneles de cobranca. | Diff de folha de preco, resultados CI do registrador, anexo de pago/KPI, Saida do tailer de transparencia, notas de ensayo de incidente. |
| N2 - Expansão | `.dao`, API de revendedor, portal de disputa, cuadros de mando de administrador, paneles de análisis. | Capturas del portal, métricas SLA de disputa, exportaciones de cuadros de mando de Steward, carta de gobierno actualizada con políticas de revendedor. |

As Saidas de fase exigem Drills tabletop registrados (fluxo feliz de registro,
congelar, interrupción de resolución) con artefactos adjuntos en `ops/drill-log.md`.

## 8. Respuesta a incidentes y escalada| Gatilho | Severidad | Dono inmediato | Acoes obrigatorias |
|---------|-----------|---------------|-------------------|
| Deriva de resolución/GAR o pruebas obsoletas | Septiembre 1 | SRE solucionador + consejo guardián | Pagine o on-call do resolutor, capture a saya do tailer, decida se debe congelar os nomes afetados, publique status cada 30 min. |
| Queda de registrador, falta de cobranza, o errores API generalizados | Septiembre 1 | Gerente de turno del registrador | Pare novos leiloes, mude para CLI manual, notifique stewards/tesouraria, anexe logs do Torii ao doc de incidente. |
| Disputa de nombre único, discrepancia de pago o escalada de cliente | Septiembre 2 | Steward + líder de apoyo | Colete provas de pago, determine si se congela suavemente y es necesario, responda al solicitante dentro de SLA, registre o resultado no tracker de disputa. |
| Achado de auditoria de conformidade | Septiembre 2 | Enlace de conformidad | Redigir plano de remediacao, archiva memo em `docs/source/sns/regulatory/`, agendar sesión de consejo de acompañamiento. |
| Taladro o ensayo | Septiembre 3 | PM hacer programa | Ejecute el escenario roteirizado de `ops/drill-log.md`, archive artefatos, marque gaps como tarefas do roadmap. |

Todos los incidentes devem criar `incident/YYYY-MM-DD-sns-<slug>.md` com tabelas
de propiedad, registros de comandos e referencias como evidencias producidas ao longo
este libro de jugadas.

## 9. Referencias-[`registry-schema.md`](./registry-schema.md)
-[`registrar-api.md`](./registrar-api.md)
-[`address-display-guidelines.md`](./address-display-guidelines.md)
-[`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
-[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (secoes SNS, DG, ADDR)

Mantenha este playbook atualizado siempre que el texto de las cartas, como superficies
de CLI ou os contratos de telemetría mudarem; como entradas del roadmap que
referencia `docs/source/sns/governance_playbook.md` siempre corresponde a
última revisión.