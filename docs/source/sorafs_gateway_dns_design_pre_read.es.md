---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_dns_design_pre_read.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 548a01a269d5b25438c037ec4c0a7a5512163aa4abfe1e8d1cda1e01f3814b68
source_last_modified: "2025-11-09T01:57:45.496541+00:00"
translation_last_reviewed: "2026-01-30"
---

# Arranque de diseño de SoraFS Gateway y DNS (2025-03-03) — Pre-read

Este memo enmarca el kickoff de diseño gateway/DNS solicitado en `roadmap.md`.
Los stakeholders deben revisar el alineamiento de alcance, las barandillas de
GAR enforcement y las expectativas del harness de conformidad antes de la
reunión para dedicar la sesión a decisiones y no a sincronizar estados.

## Logística de la reunión
- **Fecha / hora:** 2025-03-03 @ 16:00 UTC (60 minutos). Ajustar según la
  disponibilidad final.
- **Facilitador:** Networking TL (dueño de la agenda).
- **Participantes esperados:** Networking TLs, Ops leads, reps de Storage Team,
  Tooling WG, governance liaison, QA Guild (mantenedores del harness), y
  observadores de Docs/DevRel para playbooks downstream.
- **Paquete de lectura previa:** Esta nota más los artefactos normativos bajo
  [Paquete de referencia](#paquete-de-referencia).
- **Criterios de éxito:** Acuerdo sobre alcance, hand-offs y orden de hitos
  para iniciar implementación apenas SF-2 salga de revisión de rollout.

## Objetivos del kickoff
1. Fijar la visión compartida para el naming determinista de SoraDNS (SF-4) y
   el servicio trustless de gateway (SF-5), incluyendo secuencia y mapa de owners.
2. Ratificar el MVP de GAR enforcement para que las violaciones de política
   sean deterministas y observables en todos los operadores.
3. Validar el plan de cobertura del harness de conformidad SF-5a y confirmar
   el resourcing de QA Guild para suites de replay, negativas y de carga.
4. Capturar el plan de integración para Torii, manifiestos de admisión y
   runbooks operativos para que Docs/DevRel pueda preparar updates junto a los
   cambios de código.

## Contexto y dependencias
- **Inputs upstream:** `sorafs_manifest` provider adverts (SF-2), fixtures de
  chunker (SF-1b) y tooling de política de admisión (SF-2b/2c) son las fuentes
  canónicas para política de gateway y naming determinista.
- **Consumidores downstream:** `sorafs-node`, orquestadores de SDK (SF-6b),
  tooling de operadores de gateway y el portal Docusaurus (`docs/portal/`) dependen
  de las decisiones de este kickoff.
- **Alineación de políticas:** Los manifiestos GAR y sobres de telemetría ya
  pasan por `sorafs_manifest::gateway` e `iroha_torii` (ver referencias); el
  kickoff debe decidir cómo la automatización DNS y los despliegues de gateway
  ingieren esos mismos artefactos sin overrides específicos por operador.

## Snapshot de alcance

### DNS determinista y naming (SF-4/SF-4a)
- Derivación canónica de hosts desde manifiestos, incluyendo namespace y labels
  de capacidades (p. ej., `cid.gateway.sora` vs `anon.gateway.sora`).
- Cache de pruebas de alias y política TTL (`roadmap.md` SF-4a) para mantener
  pruebas deterministas y evitar la circulación de sobres GAR obsoletos.
- Expectativas de automatización DNS: integración ACME DNS-01 para wildcard,
  registros TXT canónicos con GAR y logging de auditoría para rotación de pruebas.
- Hooks de governance para agrupar cambios DNS con updates de manifiesto GAR
  para que la auto-certificación del operador sea reproducible.

### Hardening del servicio de gateway (SF-5)
- Alineación del perfil HTTP trustless (ver `docs/source/sorafs_gateway_profile.md`)
  que cubre streams CAR, semánticas de rechazo y headers de telemetría.
- Operación en modo directo para respetar checks de capacidades del manifiesto
  mientras se incrementan los defaults de SoraNet (ver `docs/source/sorafs_gateway_direct_mode.md`).
- Rate limiting y hooks de denylist ligados a manifiestos Norito para que governance
  revise cada decisión de política con artefactos firmados.
- Baseline de automatización de despliegue (`docs/source/sorafs_gateway_deployment_handbook.md`)
  para asegurar que cada gateway exponga la misma superficie: métricas
  `torii_sorafs_gar_violations_total`, telemetría `Sora-TLS-State` y bundles de
  atestación GAR.

## Alcance de GAR enforcement (borrador kickoff)
Estado actual:
- Parseo de sobres GAR, verificación de firmas y matching de host canónico están
  implementados en `crates/sorafs_manifest/src/gateway.rs`.
- Telemetría y alertas de violaciones GAR viven en
  `crates/iroha_torii/src/sorafs/api.rs` y `crates/iroha_telemetry/src/metrics.rs`.
- Documentos de automatización TLS enumeran artefactos GAR y flujos de renovación
  (`docs/source/sorafs_gateway_tls_automation.md`).

Temas a debatir:
1. **Motor de política en runtime:** definir si los gateways enlazan directo con
   `sorafs_manifest::gateway::Evaluator` o consumen un cache de política firmado
   en Norito refrescado vía Torii.
2. **Superficie de configuración:** confirmar knobs de `iroha_config` que exponen
   GAR enforcement (patrones de host, fallbacks de admisión, sinks de telemetría) y
   mapearlos a overrides del operador sin debilitar determinismo.
3. **Escalamiento de violaciones:** acordar el mínimo de automatización para enrutar
   alertas `torii_sorafs_gar_violations_total` hacia governance, incluyendo el
   esquema JSON que operadores adjuntan a tickets de incidentes.
4. **Artefactos de auditoría:** definir la estructura de archivo para atestaciones
   GAR (`manifest_signatures.json`, sobre del operador, output de conformidad) para
   que Docs los incluya en el flujo de auto-certificación.

## Chequeo de alcance del harness de conformidad (SF-5a)
Plan de implementación de referencia: `docs/source/sorafs_gateway_conformance.md`.

Confirmaciones clave para el kickoff:
- La suite de replay debe gatear _cada_ manifiesto del bundle canónico de fixtures
  y se invoca via `ci/check_sorafs_gateway_conformance.sh` (ya scriptado).
- La cobertura negativa incluye chunkers no soportados, pruebas malformadas,
  mismatches de admisión, intentos de downgrade y escenarios de rechazo GAR; QA
  Guild confirma ownership de curaduría continua de fixtures.
- Target del harness de carga: ≥1.000 streams concurrentes (mix de éxito y rechazo)
  con thresholds deterministas de latencia y telemetría que alimente el output de
  atestación firmado.
- Pipeline de atestación: asegurar que el flujo de sobres Norito descrito en
  `sorafs_gateway_conformance.md` alinea con requisitos de governance y documentar
  cómo los operadores envían reportes via `sorafs-gateway-cert`.

## Puntos de decisión para el kickoff
1. Elegir el stack canónico de automatización DNS (Terraform + RFC2136 signer vs
   Torii-managed) y definir el proceso de escrow de artefactos GAR/DNS.
2. Aprobar el enfoque de integración del motor de política GAR y sus superficies
   de configuración asociadas.
3. Validar criterios de aceptación del harness, owners de sign-off y entry-points
   de CI.
4. Secuenciar hitos: MVP de automatización DNS, enforcement de política en gateway,
   corrida pública de conformidad, lanzamiento de auto-certificación de operadores.

## Preguntas abiertas a resolver
- ¿Requerimos soporte ECH en el MVP o puede quedar tras el hito SF-5b ya entregado
  en la automatización TLS?
- ¿Qué telemetría adicional o hooks de governance son necesarios para acceso
  anónimo / blinded-CID (liga a ítems del roadmap SNNet-2)?
- ¿Se necesitan más sign-offs legales/compliance para logs de GAR enforcement
  antes de publicar plantillas de operadores?
- ¿Qué entornos (devnet, staging, prod) deben adoptar el flujo DNS determinista
  antes de permitir onboarding al gateway beta?

## Paquete de referencia
- `docs/source/sorafs_gateway_profile.md`
- `docs/source/sorafs_gateway_conformance.md`
- `docs/source/sorafs_gateway_direct_mode.md`
- `docs/source/sorafs_gateway_deployment_handbook.md`
- `docs/source/sorafs_gateway_tls_automation.md`
- `docs/source/sorafs_gateway_dns_design_runbook.md`
- `crates/sorafs_manifest/src/gateway.rs`
- `crates/iroha_torii/src/sorafs/api.rs`
- `crates/iroha_telemetry/src/metrics.rs`
- `roadmap.md` secciones SF-4/SF-5 y Near-Term Execution (Next 6 Weeks)

## Próximas acciones antes de la reunión
- Circular este pre-read a Networking, Ops, Storage, QA, Governance y Docs;
  recolectar adiciones de agenda antes del 2025-02-28.
- Producir un borrador de lista de asistentes e invitación de Slack para el kickoff;
  confirmar disponibilidad de Ops y leads de automatización DNS.
- Preparar diagramas anotados (flujo DNS + rutas de política del gateway) para el deck
  de la reunión; los borradores pueden vivir junto a esta nota en `docs/source/images/`.
- Recopilar métricas de violaciones GAR en staging (`torii_sorafs_gar_violations_total`)
  para aterrizar la discusión en el comportamiento actual.
- Recorrer el ensayo de automatización descrito en
  `docs/source/sorafs_gateway_dns_design_runbook.md`, archivar los artefactos bajo
  `artifacts/sorafs_gateway_dns/<run>/` y enlazar el bundle en la invitación para que
  los participantes inspeccionen evidencia antes de tiempo.

Una vez concluya el kickoff, actualizar `status.md` y `roadmap.md` con los hitos
acordados y asignaciones de owners.
