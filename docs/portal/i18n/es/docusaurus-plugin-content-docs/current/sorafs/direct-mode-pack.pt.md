---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: paquete de modo directo
título: Pacote de contingencia de modo directo do SoraFS (SNNet-5a)
sidebar_label: Pacote de modo directo
descripción: Configuración obligatoria, comprobaciones de cumplimiento y pasos de implementación para operar SoraFS en modo directo Torii/QUIC durante una transmisión SNNet-5a.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/direct_mode_pack.md`. Mantenha ambas como copias sincronizadas.
:::

Los circuitos SoraNet siguen como transporte padrao do SoraFS, pero el elemento de hoja de ruta **SNNet-5a** exige un respaldo regulado para que los operadores mantengan un acceso de lectura determinístico en cuanto o implementación de anonimato se completa. Este paquete captura las perillas de CLI/SDK, perfecciona la configuración, prueba de cumplimiento y la lista de verificación de implementación necesaria para rodar o SoraFS en modo directo a Torii/QUIC sin tocar los transportes de privacidad.

El respaldo se aplica a la puesta en escena y los ambientes de producción regulados cuando SNNet-5 a SNNet-9 pasan por las puertas de prontidao. Mantenha os artefatos abaixo junto com o material de despliegue do SoraFS para que los operadores alternen entre modos anónimos y directos a demanda.

## 1. Banderas de CLI y SDK- `sorafs_cli fetch --transport-policy=direct-only ...` desactivación o agendamento de relés e impoe transportes Torii/QUIC. Ajuda do CLI ahora lista `direct-only` como valor aceito.
- Los SDK deben definir `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` siempre que exuserem um alternar de "modo directo". Como enlaces generados en `iroha::ClientOptions` e `iroha_android` encaminham o mesmo enum.
- Los arneses de puerta de enlace (`sorafs_fetch`, enlaces Python) pueden interpretar o alternar solo directamente a través de los ayudantes Norito JSON compartidos para que un automóvil reciba el mismo comportamiento.

Documente o marque los runbooks voltados a parceiros y pase los conmutadores a través de `iroha_config` en vez de variaciones de ambiente.

## 2. Perfis de politica do gateway

Utilice Norito JSON para persistir la configuración determinística del orquestador. El perfil de ejemplo en `docs/examples/sorafs_direct_mode_policy.json` codifica:

- `transport_policy: "direct_only"` - rejeita provedores que así anunciam transportes de retransmisión SoraNet.
- `max_providers: 2`: limita los pares directos a los puntos finales Torii/QUIC pero confiamos. Ajuste conforme como concesos de cumplimiento regional.
- `telemetry_region: "regulated-eu"` - rotula as metricas emitidas para que tableros y auditorias distingam execucoes de fallback.
- Orcamentos de reintento conservadores (`retry_budget: 2`, `provider_failure_threshold: 3`) para evitar enmascarar gateways mal configurados.Carregue o JSON vía `sorafs_cli fetch --config` (automacao) o enlaces al SDK (`config_from_json`) antes de exportar a política a operadores. Persista a Saida do Scoreboard (`persist_path`) para trilhas de auditoria.

Las perillas de aplicación del lado de la puerta de enlace están en `docs/examples/sorafs_gateway_direct_mode.toml`. La plantilla espelha a Saida de `iroha app sorafs gateway direct-mode enable`, desabilitando cheques de sobre/admisión, conectando defaults de rate-limit y preenchendo a tabela `direct_mode` con nombres de host derivados del plano y resúmenes de manifiesto. Sustituya los valores de placeholder por su plano de despliegue antes de versionar o trecho na gestao de configuracao.

## 3. Conjunto de pruebas de cumplimiento

A continuación, haga clic en el modo directo para incluir la cobertura tanto en el operador como en las cajas de CLI:- `direct_only_policy_rejects_soranet_only_providers` garantiza que `TransportPolicy::DirectOnly` falhe rapido quando cada advert candidato soporta retransmisiones SoraNet. [crates/sorafs_orchestrator/src/lib.rs:7238]
- `direct_only_policy_prefers_direct_transports_when_available` garantía que transporta Torii/QUIC sejam usados quando disponiveis y que relés SoraNet sejam excluidos da sessao. [crates/sorafs_orchestrator/src/lib.rs:7285]
- `direct_mode_policy_example_is_valid` faz parse de `docs/examples/sorafs_direct_mode_policy.json` para garantizar que la documentación permanezca alineada con los ayudantes. [crates/sorafs_orchestrator/src/lib.rs:7509] [docs/examples/sorafs_direct_mode_policy.json:1]
- `fetch_command_respects_direct_transports` ejerza `sorafs_cli fetch --transport-policy=direct-only` contra una puerta de enlace Torii simulada, realizando una prueba de humo para ambientes regulados que fijan el transporte directo. [crates/sorafs_car/tests/sorafs_cli.rs:2733]
- `scripts/sorafs_direct_mode_smoke.sh` implica un mesmo comando con un JSON de política y una persistencia del marcador para el lanzamiento automático.

Monté una suite focada antes de publicar actualizaciones:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Si se compila el espacio de trabajo falhar por mudanzas upstream, se registra el error bloqueado en `status.md` y se activa nuevamente cuando se actualiza la dependencia.

## 4. Ejecuta automatizados de humo.La cobertura de CLI no revela retornos específicos del ambiente (por ejemplo, deriva política de la puerta de entrada o divergencias de manifiesto). Un ayudante de humo dedicado vive en `scripts/sorafs_direct_mode_smoke.sh` e involucra a `sorafs_cli fetch` con la política del orquestador en modo directo, persistencia del marcador y captura de currículum.

Ejemplo de uso:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- El script respectivo a las banderas de CLI y archivos de configuración clave=valor (veja `docs/examples/sorafs_direct_mode_smoke.conf`). Preencha o digest do manifest e as entradas de advert de provedor com valores de producao antes de rodar.
- `--policy` tiene como padrao `docs/examples/sorafs_direct_mode_policy.json`, pero cualquier JSON de orquestador producido por `sorafs_orchestrator::bindings::config_to_json` puede ser fornecido. O CLI aceita a politica via `--orchestrator-config=PATH`, habilitando ejecuta reproducciones sin ajustar flags manualmente.
- Cuando `sorafs_cli` no está en `PATH`, el ayudante compila a partir de la caja `sorafs_orchestrator` (liberación de perfil) para que el ejercicio de fumar o la plomería de modo directo enviado.
- Saidas:
  - Carga útil montada (`--output`, padrao `artifacts/sorafs_direct_mode/payload.bin`).
  - Resumen de fetch (`--summary`, padrao ao lado do payload) contendo a regiao de telemetria e relatorios de proveedores usados ​​como evidencia de rollout.
  - Instantánea del marcador persistente en el camino declarado en JSON de política (por ejemplo, `fetch_state/direct_mode_scoreboard.json`). Arquive junto ao resumo em tickets de mudanca.- Automacao do gate de adocao: apos o fetch, o helper invoca `cargo xtask sorafs-adoption-check` usando los caminos persistentes de marcador y resumen. El quórum requerido por el padrón y el número de proveedores formados en la línea de comando; sobrescreva com `--min-providers=<n>` quando precisar de uma amostra maior. Relatorios de adocao sao gravados ao lado do resumo (`--adoption-report=<path>` pode definir um local customizado) y o helper passa `--require-direct-only` por padrao (alinhado ao fallback) e `--require-telemetry` siempre que voce fornecer o flag correspondiente. Utilice `XTASK_SORAFS_ADOPTION_FLAGS` para repasar argumentos adicionales de xtask (por ejemplo, `--allow-single-source` durante una degradación aprobada para que la puerta admita e imponga el respaldo). So pule o gate com `--skip-adoption-check` ao rodar diagnosticos locais; La hoja de ruta exige que cada ejecución regulada en modo directo incluya el paquete de relatotorio de adocao.

## 5. Lista de verificación de implementación1. **Congelar la configuración:** armazene o perfil JSON de modo directo al repositorio `iroha_config` y registre o hash no ticket de mudanca.
2. **Auditoría de puerta de enlace:** confirme que los puntos finales Torii aplican TLS, TLV de capacidad y registro de auditoría antes de virar para modo directo. Publico el perfil de política de la puerta de enlace para los operadores.
3. **Aprobación de cumplimiento:** compartilhe o playbook actualizado con revisores de cumplimiento/regulatorios y captura aprovacoes para operar fora do overlay de anonimato.
4. **Ejecución en seco:** ejecute una suite de cumplimiento más una búsqueda de puesta en escena contra proveedores Torii confiaveis. Archivar resultados de marcador y currículums en CLI.
5. **Cutover em producao:** anuncie a janela de mudanca, altere `transport_policy` para `direct_only` (se voce tinha optado por `soranet-first`) y monitore os tableros de modo directo (latencia de `sorafs_fetch`, contadores de falha de proveedores). Documente el plano de reversión para volver a SoraNet-first cuando SNNet-4/5/5a/5b/6a/7/8/12/13 se graduó en `roadmap.md:532`.
6. **Revisao pos-mudanca:** instantáneas anexas del marcador, resumenes de búsqueda y resultados de monitoreo del ticket de mudanca. Actualiza `status.md` con datos efectivos y cualquier anomalía.Mantenga la lista de verificación junto al runbook `sorafs_node_ops` para que los operadores puedan ensayar el flujo antes de una virada en vivo. Cuando SNNet-5 se conecta a GA, retira el respaldo y confirma la paridad de la telemetría de producción.

## 6. Requisitos de evidencia y puerta de adocao

Las capturas en modo directo aún necesitan satisfacer la puerta de acceso SF-6c. Paquete de marcador, currículum vitae, sobre de manifiesto y relatorio de adocao em cada ejecución para que `cargo xtask sorafs-adoption-check` valide una postura de respaldo. Campos ausentes fazem o gate falhar, entao registre o metadata esperado nos tickets de mudanca.- **Metados de transporte:** `scoreboard.json` debe declarar `transport_policy="direct_only"` (y virar `transport_policy_override=true` cuando forcar o downgrade). Mantenha os campos de politica de anonimato pareados mesmo quando herdarem defaults para que revisores vejam se houve desvio do plano de anonimato em fases.
- **Contadores de proveedores:** Sessoes gateway-only debe persistir `provider_count=0` y preencher `gateway_provider_count=<n>` con el número de proveedores Torii usados. Evite editar JSON manualmente: el CLI/SDK se deriva como contagios y la puerta de adocao rejeita capturas que omiten a separacao.
- **Evidencia de manifiesto:** Cuando las gateways Torii participan, pasan el `--gateway-manifest-envelope <path>` assinado (o equivalente en SDK) para que `gateway_manifest_provided` pero `gateway_manifest_id`/`gateway_manifest_cid` estén registrados en `scoreboard.json`. Garanta que `summary.json` carregue o mesmo `manifest_id`/`manifest_cid`; a checagem de adocao falha se qualquer archivo omitir o par.
- **Expectativas de telemetria:** Quando a telemetria acompanhar a captura, rode o gate com `--require-telemetry` para que o relatorio pruebe que métricas foram emitidas. Ensaios air-gapped podem omitir o flag, mas CI e tickets de mudanca devem documentar a ausencia.

Ejemplo:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```Anexe `adoption_report.json` junto ao marcador, ao resumen, ao sobre de manifiesto y ao paquete de troncos de humo. Esses artefatos espelham o que o job de adocao em CI (`ci/check_sorafs_orchestrator_adoption.sh`) aplica y mantiene downgrades de modo directo auditaveis.