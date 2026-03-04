---
lang: es
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbooks de incidentes y ejercicios de rollback

## propuesta

El elemento de la hoja de ruta **DOCS-9** exige playbooks acionaveis mais um plano de ensaio para que
Los operadores del portal consigam recuperan falhas de envío sin adivinhacao. Esta nota cobre tres
incidentes de alto sinal - despliega falhos, degradación de replicacao e quedadas de análisis - e
documenta los ejercicios trimestrales que demuestran que o rollback de alias e a validacao sintetica
continuam funcionando de punta a punta.

### Material relacionado

- [`devportal/deploy-guide`](./deploy-guide) - flujo de trabajo de empaquetado, firma y promoción de alias.
- [`devportal/observability`](./observability) - etiquetas de liberación, análisis y sondas referenciados abaixo.
- `docs/source/sorafs_node_client_protocol.md`
  mi [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - Telemetría del registro y límites de escalamiento.
- `docs/portal/scripts/sorafs-pin-release.sh` y ayudantes `npm run probe:*`
  referenciados nos listas de verificación.

### Telemetría y herramientas compartidas| Señal / Herramienta | propuesta |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (cumplido/incumplido/pendiente) | Detecta bloques de réplica y violaciones de SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Quantifica profundidade do backlog y latencia de completacao para triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Mostra falhas do gateway que frecuentemente siguen un despliegue ruim. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Sondas sintéticas que bloquean liberaciones y validan reversiones. |
| `npm run check:links` | Puerta de enlaces quebrados; usado apos cada mitigacao. |
| `sorafs_cli manifest submit ... --alias-*` (usado por `scripts/sorafs-pin-release.sh`) | Mecanismo de promoción/reversao de alias. |
| Placa `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Agrega telemetría de rechazos/alias/TLS/replicacao. Alertas do PagerDuty referenciam este dolor es como evidencia. |

## Runbook - Implementar falho ou artefato ruim

### Condicoes de disparo

- Sondas de vista previa/producción falham (`npm run probe:portal -- --expect-release=...`).
- Alertas Grafana en `torii_sorafs_gateway_refusals_total` o
  `torii_sorafs_manifest_submit_total{status="error"}` implementación apos um.
- Manual de control de calidad nota rotas quebradas ou falhas do proxy Pruébelo inmediatamente apos
  una promocao hacer alias.

### Contención inmediata1. **Congelar despliegues:** marcar o pipeline CI con `DEPLOY_FREEZE=1` (entrada del flujo de trabajo
   GitHub) o pausar el trabajo Jenkins para que no se envíe ningún artefato.
2. **Capturar artefatos:** bajar `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, y dijo que dos sondas se construyen con falta para que
   o referencia de reversión de resúmenes exatos.
3. **Notificar a las partes interesadas:** almacenamiento SRE, líder Docs/DevRel, e o oficial de turno de
   gobernanza para concienciación (especialmente cuando `docs.sora` esta impactado).

### Procedimiento de reversión

1. Identificar el último bien conocido (LKG). O flujo de trabajo de producción os armazena em
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Re-vincule o alias a esse manifest com o helper de envío:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. Registre o resumo do rollback no ticket do incidente junto com os digests do
   manifiesto LKG e do manifiesto com falha.

### Validacao

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` e `sorafs_cli proof verify ...`
   (veja o guía de despliegue) para confirmar que o manifiesto repromovido continua
   batendo con el CAR arquivado.
4. `npm run probe:tryit-proxy` para garantizar que el proxy Try-It staging vuelva a funcionar.

### Pos-incidente

1. Reative o pipeline de desplegar apenas depois de entender a causa raiz.
2. Preencha as entradas "Lecciones aprendidas" en [`devportal/deploy-guide`](./deploy-guide)
   com novos pontos, se houver.
3. Abra defectos para una suite de testes falhada (sonda, verificador de enlaces, etc.).## Runbook - Degradación de replicación

### Condicoes de disparo

- Alerta: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  abrazadera_min(suma(torii_sorafs_replication_sla_total{resultado=~"cumplido|perdido"}), 1) <
  0,95` por 10 minutos.
- `torii_sorafs_replication_backlog_total > 10` por 10 minutos (veja
  `pin-registry-ops.md`).
- Gobernanca reporta alias lento apos um liberación.

### Triaje

1. Inspeccione paneles de control de [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) para
   Confirme si el trabajo pendiente está localizado en una clase de almacenamiento o en una flota de proveedores.
2. Los registros de Cruze hacen Torii por advertencias `sorafs_registry::submit_manifest` para determinar si
   como presentaciones estao falhando.
3. Amostre a saude das replicas vía `sorafs_cli manifest status --manifest ...` (lista
   resultados por proveedor).

### Mitigacao

1. Reemita el manifiesto con mayor contagio de réplicas (`--pin-min-replicas 7`) usando
   `scripts/sorafs-pin-release.sh` para que el planificador distribuya carga en un conjunto mayor
   de proveedores. Registre o novo digest no log do incidente.
2. Se o backlog estiver preso a um proveedor único, desabilite-o temporalmente vía o
   planificador de replicación (documentado en `pin-registry-ops.md`) y envie um novo
   manifest forcando los otros proveedores a actualizar o alias.
3. Quando a frescura do alias for mais critica que a paridade de replicacao, re-vincule o
   alias a um manifest quente ja em staging (`docs-preview`), depois publique um manifest de
   acompañamiento cuando el SRE limpia el backlog.### Recuperacao e fechamento

1. Monitoree `torii_sorafs_replication_sla_total{outcome="missed"}` para garantizar que o
   contador estabilizar.
2. Capture a saya `sorafs_cli manifest status` como evidencia de que cada réplica voltou
   una conformidade.
3. Abra ou atualize o post-mortem do backlog de replicacao com proximos passos
   (escalado de proveedores, ajuste del fragmentador, etc.).

## Runbook - Queda de análisis o telemetría

### Condicoes de disparo

- `npm run probe:portal` pasa, más paneles de control para realizar eventos
  `AnalyticsTracker` por >15 minutos.
- Revisión de privacidad aponta um aumento inesperado de eventos descartados.
- `npm run probe:tryit-proxy` falta en las rutas `/probe/analytics`.

### Respuesta

1. Verifique las entradas de compilación: `DOCS_ANALYTICS_ENDPOINT` e
   `DOCS_ANALYTICS_SAMPLE_RATE` no se libera ningún artefato (`build/release.json`).
2. Vuelva a ejecutar `npm run probe:portal` con `DOCS_ANALYTICS_ENDPOINT` apontando para o
   Collector de staging para confirmar que el rastreador todavía emite cargas útiles.
3. Se os coleccionistas estiverem abajo, define `DOCS_ANALYTICS_ENDPOINT=""` y reconstruir
   para que o tracker faca cortocircuito; registre a janela de outage na linha
   hacer tempo hacer incidente.
4. Valide que `scripts/check-links.mjs` además de la huella digital de `checksums.sha256`
   (Quedas de Analytics *nao* deben bloquear la validación del mapa del sitio).
5. Cuando voltar el colector, montó `npm run test:widgets` para ejercitar las pruebas unitarias
   hacer ayudante de análisis antes de republicar.

### Pos-incidente1. Actualizar [`devportal/observability`](./observability) con nuevas limitaciones del recopilador
   ou requisitos de amostragem.
2. Abra um aviso de gobierno se dados de análisis foram perdidos ou redigidos fora
   la política.

## Ejercicios trimestrales de resiliencia

Ejecute os dos ejercicios durante a **primeira terca-feira de cada trimestre** (Ene/Abr/Jul/Out)
ou inmediatamente apos qualquer mudanca maior de infraestructura. Armazene artefatos em
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Taladro | Pasos | Pruebas |
| ----- | ----- | -------- |
| Ensayo de reversión de alias | 1. Repita o revierta "Deploy falho" usando el manifiesto de producción más reciente.2. Re-vincular a producao asim que os probes passarem.3. Registrador `portal.manifest.submit.summary.json` y registros de sondas en pasta del taladro. | `rollback.submit.json`, dijo las sondas y la etiqueta de liberación del ensayo. |
| Auditorio de validación sintética | 1. Rodar `npm run probe:portal` e `npm run probe:tryit-proxy` contra producción y puesta en escena.2. Rodar `npm run check:links` y archivo `build/link-report.json`.3. Anexar capturas de pantalla/exportaciones de dolor es Grafana confirmando el éxito de dos sondas. | Logs de probes + `link-report.json` referenciando o manifestación de huellas dactilares. |

Escalone taladra perdidos para o manager de Docs/DevRel e a revisao degobernanza de SRE,
Pois o roadmap exige evidencia trimestral determinista de que o rollback de alias e os
Sondas del portal continuo saudaveis.

## Coordenacao PagerDuty y de guardia- El servicio PagerDuty **Docs Portal Publishing** y dono dos alertas generadas desde
  `dashboards/grafana/docs_portal.json`. Como se indica `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, e `DocsPortal/TLSExpiry` páginas principales de Docs/DevRel
  com Almacenamiento SRE como secundario.
- Cuando toca la página, incluye `DOCS_RELEASE_TAG`, capturas de pantalla adjuntas de Grafana
  afetados e linke a saya de probe/link-check nas notas do incidente antes de iniciar
  un mitigacao.
- Después de mitigacao (revertir o volver a implementar), vuelva a ejecutar `npm run probe:portal`,
  `npm run check:links`, y captura instantáneas Grafana recientes mostrando como métricas
  umbrales de volta aos. Anexo toda la evidencia del incidente de PagerDuty
  antes de resolver.
- Si dos alertas se disparan al mismo tiempo (por ejemplo, expiracao de TLS mais backlog),
  rechazos de clasificación primeiro (parar o publicación), ejecutar o procedimento de rollback e
  depois resolva TLS/backlog com Almacenamiento SRE na ponte.