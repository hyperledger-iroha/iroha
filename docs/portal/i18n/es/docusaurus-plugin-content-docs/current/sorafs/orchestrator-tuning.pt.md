---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: afinación del orquestador
título: Despliegue y ajuste del orquestador
sidebar_label: Ajuste del orquestador
descripción: Padrões práticos, orientación de ajuste y puntos de control de auditorio para levantar o orquestador multiorigen à GA.
---

:::nota Fuente canónica
España `docs/source/sorafs/developer/orchestrator_tuning.md`. Mantenha as duas cópias alinhadas até que a documentação alternativa seja aposentada.
:::

# Guía de implementación y ajuste del orquestador

Esta guía se basa en [referencia de configuración](orchestrator-config.md) y no
[runbook de implementación de origen múltiple](multi-source-rollout.md). Ele explica
como ajustar el orquestador para cada fase de rollout, como interpretar os
artefatos do scoreboard e quais sinais de telemetria devem estar prontos antes
de ampliar el tráfico. Aplique as recomendaciones de forma consistente na CLI, nos
SDK y automatización para que cada uno siga una política de búsqueda determinística.

## 1. Conjuntos de parámetros base

Parte de una plantilla de configuración compartida y ajuste de un pequeño conjunto
de perillas a medida que o rollout avança. A tabela abaixo captura de valores
recomendados para as fases mais comuns; os valores não listados voltam aos
padrões de `OrchestratorConfig::default()` e `FetchOptions::default()`.| Fase | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Notas |
|------|-----------------|-------------------------------|------------------------------------|-----------------------|------------------------------|-------|
| **Laboratorio/CI** | `3` | `2` | `2` | `2500` | `300` | Um limite de latência y uma janela de graça estreitos exponen el ruido de la telemetría rápidamente. Mantenha reintries baixos para revelar manifiestos inválidos mais cedo. |
| **Puesta en escena** | `4` | `3` | `3` | `4000` | `600` | Espelha os padrões de produção deixando folga para peers exploratórios. |
| **Canarias** | `6` | `3` | `3` | `5000` | `900` | Igual aos padrões; Defina `telemetry_region` para que los paneles de control puedan separar el tráfico canario. |
| **Disponibilidad general** | `None` (usar todos los elegíveis) | `4` | `4` | `5000` | `900` | Aumente os limiares de reintento e falha para absorber falhas transitórias tanto como auditorias continuam reforçando o determinismo. |- `scoreboard.weight_scale` permanece en el padrão `10_000` a menos que un sistema downstream exija outra resolução inteira. Aumentar a escala não altera a ordenação dos provedores; apenas emite una distribución de créditos más densa.
- Para migrar entre fases, persista el paquete JSON y use `--scoreboard-out` para que una trilha de auditoria registre o conjunto exacto de parámetros.

## 2. Higiene del marcador.

El marcador combina requisitos del manifiesto, anuncios de proveedores y telemetría.
Antes de avanzar:1. **Valide a frescura da telemetria.** Garanta que os snapshots referenciados por
   `--telemetry-json` foram capturados dentro de janela de graça configurada. Entradas
   mais antigas que `telemetry_grace_secs` falham con `TelemetryStale { last_updated }`.
   Trate esto como un bloque rígido y actualice la exportación de telemetría antes de seguir.
2. **Inspecione razões de elegibilidade.** Persista artefatos via
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. cada entrada
   traz um bloco `eligibility` com a causa exata da falha. No sobrescreva
   desajustes de capacidad o anuncios vencidos; corrija o carga útil aguas arriba.
3. **Revise mudanças de peso.** Compare el campo `normalised_weight` con el lanzamiento
   anteriores. Mudanças >10% deben correlacionarse con alteraciones deliberadas en anuncios
   La telemetría y la necesidad de estar registradas en el registro de implementación.
4. **Archivo artefatos.** Configure `scoreboard.persist_path` para cada ejecución
   Emite la instantánea final del marcador. Anexe o artefato ao registro de liberación
   junto al manifiesto y al paquete de telemetria.
5. **Registre evidências de mix de provedores.** A metadata de `scoreboard.json` _e_ o
   `summary.json` correspondiente devem export `provider_count`, `gateway_provider_count`
   e o label derivado `provider_mix` para que os revisores provem se a execução foi
   `direct-only`, `gateway-only` o `mixed`. Informe de puerta de enlace de capturas `provider_count=0`e `provider_mix="gateway-only"`, mientras se ejecutan errores que exigen contagios no
   zero para ambas como fuentes. `cargo xtask sorafs-adoption-check` impõe esses campos
   (e falha quando contagens/labels divergem), entonces ejecuta-o sempre junto com
   `ci/check_sorafs_orchestrator_adoption.sh` o su script de captura para producir
   o paquete de pruebas `adoption_report.json`. Cuando puertas de enlace Torii estiverem
   envolvidos, mantenha `gateway_manifest_id`/`gateway_manifest_cid` na metadatos
   marcador para que o puerta de adoção consiga correlacionar o sobre do manifiesto
   com o mix de proveedores capturados.

Para definiciones detalladas de campos, veja
`crates/sorafs_car/src/scoreboard.rs` y la estructura de resumen de CLI expuesta por
`sorafs_cli fetch --json-out`.

## Referencia de banderas de CLI y SDK

`sorafs_cli fetch` (ver `crates/sorafs_car/src/bin/sorafs_cli.rs`) y envoltorio
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) compartilham
a mesma superfície de configuración del orquestador. Utilice las siguientes banderas ao
Capture evidencias de implementación o reproducción de accesorios canónicos:

Referencia compartilhada de flags multi-origem (mantenha a ajuda da CLI e os docs)
sincronizados editando apenas este archivo):- `--max-peers=<count>` limita la cantidad de proveedores elegíveis que sobreviven al filtro del marcador. Deixe sem configurar para hacer streaming de todos los proveedores elegíveis y defina `1` apenas cuando estiver ejercitando deliberadamente o fallback de fuente única. Espelha o perilla `maxPeers` con SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` encamina hacia el límite de reintentos por fragmento aplicado por `FetchOptions`. Utilice una tabla de implementación sin guía de ajuste para los valores recomendados; Las ejecuciones de CLI que recopilan evidencias deben corresponderse con los padrões dos SDK para mantener la paridad.
- `--telemetry-region=<label>` rotula as series Prometheus `sorafs_orchestrator_*` (y relés OTLP) con una etiqueta de región/ambiente para que los tableros separem tráfego de lab, staging, canary y GA.
- `--telemetry-json=<path>` injeta o snapshot referenciado pelo marcador. Persista el JSON al lado del marcador para que los auditores puedan reproducir la ejecución (y para que `cargo xtask sorafs-adoption-check --require-telemetry` compruebe cuál flujo OTLP alimentou a captura).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) habilitan los ganchos del puente observador. Cuando se definen, el orquestador transmite fragmentos a través del proxy Norito/Kaigi local para que los clientes de navegador, guarden cachés y salas Kaigi reciban los mismos recibos emitidos por Rust.- `--scoreboard-out=<path>` (opcionalmente con `--scoreboard-now=<unix_secs>`) persiste la instantánea de elegibilidad para auditores. Siempre empareje el JSON persistente con los artefactos de telemetría y manifiestos referenciados sin ticket de publicación.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` aplica ajustes determinísticos sobre metadatos de anuncios. Use esses flags apenas para ensayos; Las reducciones de producción deben pasar por artefatos de gobernanza para que cada nó se aplique o mesmo paquete de política.
- `--provider-metrics-out` / `--chunk-receipts-out` retêm métricas de salud por proveedor y recibos de trozos referenciados pela checklist de rollout; anexe ambos os artefatos ao registrar a evidência de adoção.

Ejemplo (usando el accesorio publicado):

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

Los SDK se pueden configurar a través de `SorafsGatewayFetchOptions` sin cliente
Óxido (`crates/iroha/src/client.rs`), sin fijaciones JS
(`javascript/iroha_js/src/sorafs.js`) y sin SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Mantenha esses ayudantes em
sincronización con los botones de CLI para que los operadores puedan copiar políticas entre
automatización sin camadas de traducción solitaria medida.

## 3. Ajuste de la política de recuperación

`FetchOptions` Reintento de control, coincidencia y verificación. Ao ajustar:- **Reintentos:** Elevar `per_chunk_retry_limit` acima de `4` aumenta o tempo de
  recuperação, mas pode mascarar falhas de provedores. Prefira manter `4` como
  Teto e confiar en la rotación de proveedores para exportar los fracos.
- **Limiar de falhas:** `provider_failure_threshold` define quando um provedor é
  desabilitado pelo restante da sessão. Alinhe esse valor à política de reintento:
  um limiar menor que o orçamento de retry força o orquestrador a ejetar um peer
  antes de esgotar todos los reintentos.
- **Concorrência:** Deixe `global_parallel_limit` sem definir (`None`) a menos que
  Un ambiente específico no debe saturar los rangos anunciados. Cuando esté definido,
  garanta que o valor seja ≤ à soma dos orçamentos de streams dos provedores para
  evitar el hambre.
- **Toggles de verificación:** `verify_lengths` e `verify_digests` deben permanecer
  habilitados en producción. Eles garantem determinismo quando há frotas mistas de
  provedores; desative-os apenas em ambientes aislados de fuzzing.

## 4. Estágio de transporte e anonimato

Utilice los campos `rollout_phase`, `anonymity_policy` e `transport_policy` para
representar una postura de privacidade:- Prefira `rollout_phase="snnet-5"` y permita que a política de padrão anonimato
  Acompaña los marcos de SNNet-5. Sustituye a través de `anonymity_policy_override` solo
  cuando la gobernanza emite una dirección assinada.
- Mantenha `transport_policy="soranet-first"` como base en cuanto SNNet-4/5/5a/5b/6a/7/8/12/13 estiverem 🈺
  (veja `roadmap.md`). Utilice `transport_policy="direct-only"` en algún momento para degradar
  documentados o ejercicios de cumplimiento y guarde una revisión de cobertura PQ antes
  de promover `transport_policy="soranet-strict"` — ese nivel falta rápido se apenas
  Los relés clásicos permanecerán.
- `write_mode="pq-only"` solo debe ser imposto cuando cada camino de escritura (SDK,
  orquestador, instrumentación de gobierno) puder satisfazer requisitos PQ. durante
  rollouts, mantenha `write_mode="allow-downgrade"` para que respuestas de emergencia
  Es posible usar rotaciones directas en cuanto a telemetría sinaliza o downgrade.
- La selección de guardias y la puesta en escena de circuitos dependen del directorio SoraNet.
  Forneça o snapshot assinado de `relay_directory` y persista o cache de `guard_set`
  para que o churn de guards permaneça na janela de retenção acordada. Una impresión
  El caché digital registrado por `sorafs_cli fetch` hace parte de la evidencia de implementación.

## 5. Ganchos de degradación y cumplimiento

Dos subsistemas del orquestador ayudan a importar la política sin manual de intervención:- **Remediación de degradación** (`downgrade_remediation`): monitor de eventos
  `handshake_downgrade_total` y, después de `threshold` configurado ser superado en
  `window_secs`, fuerza el proxy local para `target_mode` (solo metadatos por pad).
  Mantenha os padrões (`threshold=3`, `window=300`, `cooldown=900`) a menos que
  Las revisiones de incidentes indican otro padrón. Documente qualquer anulación no
  Registro de implementación y garantía de que los paneles de control acompañan a `sorafs_proxy_downgrade_state`.
- **Política de cumplimiento** (`compliance`): exclusiones de jurisdição e manifesto
  fluem por listas de opt-out geridas pelagobernanza. Nunca insira anula ad hoc
  sin paquete de configuración; em vez disso, solicite una actualización assinada de
  `governance/compliance/soranet_opt_outs.json` y reimplantar el JSON generado.

Para ambos sistemas, persiste el paquete de configuración resultante e incluye
nas evidencias de liberación para que los auditores puedan rastrear como reduções foram
accionadas.

## 6. Telemetría y paneles de control

Antes de ampliar el lanzamiento, confirme que los siguientes sinais están activos no
ambiente alvo:- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  Debes estar en cero después de la conclusión de Canarias.
- `sorafs_orchestrator_retries_total` y
  `sorafs_orchestrator_retry_ratio` — devem se estabilizar bajo 10% durante
  o canary e permanecer abaixo de 5% após GA.
- `sorafs_orchestrator_policy_events_total` — valida que a etapa de implementación esperada
  Está activa (etiqueta `stage`) y registra caídas de tensión a través de `outcome`.
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — acompaña el principio de relés PQ en
  relación con las expectativas de la política.
- Alvos de log `telemetry::sorafs.fetch.*` — devem ser enviados al agregador de logs
  compartilhado com buscas salvas para `status=failed`.

Carregue o tablero Grafana canônico em
`dashboards/grafana/sorafs_fetch_observability.json` (exportado sin portal em
**SoraFS → Obtener observabilidad**) para que los selectores de región/manifiesto, o
Mapa de calor de reintentos por proveedor, los histogramas de latencia de fragmentos y el sistema operativo.
Los contadores de puesto corresponden a los que el SRE revisa durante los quemados. conectar
como se registra en Alertmanager en `dashboards/alerts/sorafs_fetch_rules.yml` y se valida
sintaxis do Prometheus com `scripts/telemetry/test_sorafs_fetch_alerts.sh` (o ayudante
ejecute `promtool test rules` localmente o en Docker). Como pasajes de alertas
Exigimos el mesmo bloco de roteamento que el script imprime para que los operadores possam.
anexar la evidencia al ticket de implementación.

### Flujo de quemado de telemetríaEl elemento de hoja de ruta **SF-6e** exige un burn-in de telemetría de 30 días antes de
alternar el orquestador multiorigen para sus padres GA. Utilice scripts del sistema operativo
repositorio para capturar un paquete de artefatos reproduzível para cada día da
janela:

1. Ejecute `ci/check_sorafs_orchestrator_adoption.sh` com como variante de grabación
   configuradas. Ejemplo:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```Oh ayudante reproduz `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   grava `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` e `adoption_report.json` en
   `artifacts/sorafs_orchestrator/<timestamp>/`, e impone un número mínimo de
   provedores elegíveis vía `cargo xtask sorafs-adoption-check`.
2. Cuando están presentes las variaciones de burn-in, el script también emite
   `burn_in_note.json`, capturando la etiqueta, el índice del día, la identificación del manifiesto,
   a fonte de telemetria e os digests dos artefatos. Anexe esse JSON y el registro de
   rollout para deixar claro cual captura satisface cada día da janela de 30 días.
3. Importar el tablero Grafana actualizado (`dashboards/grafana/sorafs_fetch_observability.json`)
   para el espacio de trabajo de puesta en escena/producción, marca con la etiqueta de grabación y confirmación
   que cada dolor exibe amostras para o manifesto/região em teste.
4. Ejecute `scripts/telemetry/test_sorafs_fetch_alerts.sh` (o `promtool test rules …`)
   siempre que `dashboards/alerts/sorafs_fetch_rules.yml` mudar para documentar que
   El roteamento de alertas corresponde a las métricas exportadas durante el burn-in.
5. Archivar la instantánea en el tablero, a continuación la prueba de alertas y la cola de registros
   das buscas `telemetry::sorafs.fetch.*` junto aos artefatos do orquestador para
   que a gobernador possa reproducir a evidencia sin extrair métricas de sistemas
   en producción.

## 7. Lista de verificación de implementación1. Regenerar marcadores en CI usando la configuración candidata y captura del sistema operativo
   artefatos sollozo controle de verso.
2. Ejecute o busque determinístico dos accesorios en cada ambiente (laboratorio, puesta en escena,
   canario, producción) y anexo de los artefactos `--scoreboard-out` e `--json-out` ao
   registro de lanzamiento.
3. Revisar los paneles de telemetría con el motor de planta, garantizando que
   todas como métricas acima tenham amostras ao vivo.
4. Registre o caminho final de configuração (generalmente a través de `iroha_config`) e o
   commit git do registro de gobernanza usado para anuncios y cumplimiento.
5. Actualizar el rastreador de implementación y notificar como equipos de SDK sobre los nuevos
   padrões para manter as integrações de clientes alinhadas.

Seguir esta guía mantém os despliegues del orquestador determinísticos e
passíveis de auditoria, enquanto fornece ciclos de feedback claros para ajustar
Orçamentos de reintentos, capacidad de proveedores y postura de privacidad.