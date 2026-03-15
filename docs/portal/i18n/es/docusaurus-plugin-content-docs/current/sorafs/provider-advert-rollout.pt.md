---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "Plano de despliegue de anuncios de proveedores SoraFS"
---

> Adaptado de [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# Plano de despliegue de anuncios de proveedores SoraFS

Este plano coordina o cut-over de anuncios permisivos de proveedores para a
superficie totalmente gobernada `ProviderAdvertV1` exigida para recuperación
trozos de múltiples fuentes. Ele foca em tres entregables:

- **Guia de operadores.** Pasos que proveedores de almacenamiento precisan concluir antes de cada puerta.
- **Cobertura de telemetria.** Paneles y alertas que usan Observabilidad y Operaciones
  para confirmar que a rede aceita apenas anuncios conformes.
  para que equipes de SDK y lanzamientos de planos de herramientas.

O rollout se alinha aos marcos SF-2b/2c no
[roadmap de migracao SoraFS](./migration-roadmap) e asume que una política de admisión
no [política de admisión del proveedor](./provider-admission-policy) ja esta activa.

## Línea de tiempo de fases

| Fase | Janela (alvo) | Compartimento | Ações del operador | Foco de observabilidad |
|-------|-----------------|-----------|------------------|-------------------|

## Lista de verificación del operador1. **Inventariar anuncios.** Liste cada anuncio publicado y registre:
   - Camino de la envolvente de gobierno (`defaults/nexus/sorafs_admission/...` o equivalente en producción).
   - `profile_id` e `profile_aliases` hacen publicidad.
   - Lista de capacidades (espera-se pelo menos `torii_gateway` e `chunk_range_fetch`).
   - Bandera `allow_unknown_capabilities` (necesaria quando TLVs reservados por el proveedor estiverem presentes).
2. **Regenerar herramientas com del proveedor.**
   - Reconstruya la carga útil con su editor de anuncio del proveedor, garantizando:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` con `max_span` definido
     - `allow_unknown_capabilities=<true|false>` cuando houver TLVs GRASA
   - Validar vía `/v1/sorafs/providers` e `sorafs_fetch`; advertencias sobre capacidades
     desconhecidas devem ser triageadas.
3. **Validación de preparación de múltiples fuentes.**
   - Ejecute `sorafs_fetch` con `--provider-advert=<path>`; o CLI ahora falta cuando
     `chunk_range_fetch` esta ausente y mostra advertencias para capacidades desconhecidas
     ignoradas. Capture o informe JSON y archive con registros de operaciones.
4. **Preparar renovaciones.**
   - Envie sobres `ProviderAdmissionRenewalV1` pelo menos 30 días antes de hacerlo
     aplicación sin puerta de enlace (R2). Renovacoes devem manter o mango canonico e o
     establecer capacidades; Solo hay que apostar, los puntos finales o los metadatos deben cambiar.
5. **Comunicar equipos dependientes.**
   - Donos de SDK devem lancar versoes que mostrem advertencias aos operadores quando
     anuncios forem rejeitados.- DevRel anuncia cada transición de fase; Incluye enlaces de paneles y lógica.
     de umbrales abaixo.
6. **Instalar paneles y alertas.**
   - Importar o Grafana exportar y colocar sob **SoraFS / Provider Rollout** con UID
     `sorafs-provider-admission`.
   - Garanta que as regras de alert apontem para o canal compartilhado
     `sorafs-advert-rollout` en puesta en escena y producción.

## Telemetría y paneles de control

Como siguientes métricas y estas expuestas vía `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` - conta aceitos, rejeitados e
  advertencias. Los motivos incluyen `missing_envelope`, `unknown_capability`, `stale`.
  y `policy_violation`.

Exportación Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Importar archivo al repositorio compartido de paneles (`observability/dashboards`)
Actualizamos apenas el UID de la fuente de datos antes de publicarlo.

O board e publicado na pasta Grafana **SoraFS / Provider Rollout** con UID
estavel `sorafs-provider-admission`. Como regras de alerta
`sorafs-admission-warn` (advertencia) e `sorafs-admission-reject` (crítico) estao
preconfiguradas para usar una política de notificación `sorafs-advert-rollout`; ajuste
Ese punto de contacto se o destino cambiar, en vez de editar o JSON en el panel.

Paneles Grafana recomendados:| Paneles | Consulta | Notas |
|-------|-------|-------|
| **Tasa de resultados de admisión** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Gráfico de pila para visualizar aceptar vs advertir vs rechazar. Alerta cuando advierte > 0,05 * total (advertencia) o rechaza > 0 (crítico). |
| **Proporción de advertencia** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Serie temporal de línea única que alimenta el umbral del buscapersonas (tasa de advertencia del 5% rolando 15 minutos). |
| **Motivos del rechazo** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Guía de triaje del runbook; enlaces anexos para mitigar. |
| **Actualizar deuda** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Indica proveedores que perderán la fecha límite de actualización; Los registros de Cruze com hacen caché de descubrimiento. |

Artefactos de CLI para paneles manuales:

- `sorafs_fetch --provider-metrics-out` contadores de pantalla `failures`, `successes`
  e `disabled` por proveedor. Importar paneles de control ad-hoc para monitorear ejecuciones en seco
  hacer orquestador antes de proveedores de trocar en producción.
- Los campos `chunk_retry_rate` e `provider_failure_rate` reportan JSON destacados
  estrangulamiento o síntomas de cargas útiles obsoletas que acostumbran anteceder rechazos de admisión.

### Diseño del panel Grafana

Observabilidad publica um board dedicado - **SoraFS Admisión del proveedor
Lanzamiento** (`sorafs-provider-admission`) - sollozo **SoraFS / Lanzamiento del proveedor**
con los siguientes ID canónicos del panel:- Panel 1 - *Tasa de resultados de admisión* (área apilada, unidade "ops/min").
- Panel 2 - *Relación de advertencia* (serie única), com a expressao
  `suma(tasa(torii_sorafs_admission_total{result="warn"}[5m])) /
   suma(tasa(torii_sorafs_admission_total[5m]))`.
- Panel 3 - *Motivos de rechazo* (series temporales agrupadas por `reason`), ordenada por
  `rate(...[5m])`.
- Panel 4 - *Actualizar deuda* (stat), espelha a query da tabela acima e e anotada
  com actualizar fechas límite dos anuncios extraidos del libro mayor de migración.

Copia (o llora) el esqueleto JSON en el repositorio de paneles de infrarrojos
`observability/dashboards/sorafs_provider_admission.json`, después de actualizar apenas
o UID de origen de datos; os IDs de panel e registros de alerta sao pelos referenciados
runbooks abaixo, entao evite renumerar sin revisar esta documentacao.

Por conveniencia, el repositorio incluye una definición de tablero de referencia en
`docs/source/grafana_sorafs_admission.json`; copia para su pasta Grafana se
Precisar de un punto de partida para testes locais.

### Registros de alerta Prometheus

Adicione o seguinte grupo de regras em
`observability/prometheus/sorafs_admission.rules.yml` (llora o archivo se este para
o primer grupo de registros SoraFS) e incluye la configuración de Prometheus.
Substitua `<pagerduty>` pelo label de roteamento real da sua rotacao on-call.

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

Ejecute `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
antes de enviar mudancas para garantizar que a sintaxe passe `promtool check rules`.

## Matriz de implementación| Características del anuncio | R0 | R1 | R2 | R3 |
|--------------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` presente, alias canónicos, `signature_strict=true` | Aceptar | Aceptar | Aceptar | Aceptar |
| Ausencia de `chunk_range_fetch` capacidad | WARN (ingesta + telemetría) | ADVERTENCIA | RECHAZAR (`reason="missing_capability"`) | RECHAZAR |
| TLV de capacidad desconhecida sin `allow_unknown_capabilities=true` | Aceptar | ADVERTENCIA (`reason="unknown_capability"`) | RECHAZAR | RECHAZAR |
| `refresh_deadline` caducado | RECHAZAR | RECHAZAR | RECHAZAR | RECHAZAR |
| `signature_strict=false` (accesorios de diagnóstico) | OK (desarrollo apenas) | ADVERTENCIA | ADVERTENCIA | RECHAZAR |

Todos los horarios usan UTC. Datos de cumplimiento sao refletidas no migración
libro mayor e nao mudam sem voto do consejo; qualquer mudanca requer actualizar este
arquivo e o ledger no mesmo PR.

> **Nota de implementación:** R1 introdujo la serie `result="warn"` en
> `torii_sorafs_admission_total`. O patch de ingestao do Torii que adiciona o
> novo label e acompanhado junto das tarefas de telemetria SF-2; comió la, uso

## Comunicación y tratamiento de incidentes- **Mensaje publicitario de estado semanal.** DevRel comparte un resumen de métricas de admisión,
  avisos pendientes y plazos próximos.
- **Respuesta a incidentes.** Se alertas `reject` dispararem, ingenieros de guardia:
  1. Buscamos un anuncio ofensivo a través del descubrimiento Torii (`/v1/sorafs/providers`).
  2. Vuelva a ejecutar la validación del anuncio sin canalización del proveedor y compare con
     `/v1/sorafs/providers` para reproducir o producir un error.
  3. Coordenam con el proveedor a rotacao do anuncio antes de la próxima fecha límite de actualización.
- **El cambio se congela.** Nenhuma mudanca no esquema de capacidades durante R1/R2 a
  menos que o comite de rollout aprove; ensayos GRASA devem ser agendados na
  janela semanal de manutencao e registrados en el libro mayor de migración.

## Referencias

- [SoraFS Protocolo de cliente/nodo](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Política de admisión de proveedores](./provider-admission-policy)
- [Hoja de ruta de migración](./migration-roadmap)
- [Extensiones de múltiples fuentes de anuncios de proveedores](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)