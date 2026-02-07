---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: incorporación-de-operador-nexus
título: Incorporacion de operadores de data-space de Sora Nexus
descripción: Espejo de `docs/source/sora_nexus_operator_onboarding.md`, que sigue la checklist de liberación de extremo a extremo para operadores de Nexus.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sora_nexus_operator_onboarding.md`. Mantenga ambas copias alineadas hasta que las ediciones localizadas lleguen al portal.
:::

# Incorporacion de operadores de data-space de Sora Nexus

Esta guía captura el flujo de extremo a extremo que deben seguir los operadores de data-space de Sora Nexus una vez se anuncia un lanzamiento. Complementa el runbook de doble vía (`docs/source/release_dual_track_runbook.md`) y la nota de selección de artefactos (`docs/source/release_artifact_selection.md`) al describir como alinear bundles/imagenes descargados, manifests y plantillas de configuración con las expectativas de carriles globales antes de poner un nodo en línea.## Audiencia y requisitos previos
- Has sido aprobado por el Programa Nexus y recibiste tu asignación de espacio de datos (índice de carril, ID/alias del espacio de datos y requisitos de política de enrutamiento).
- Puedes acceder a los artefactos firmados del lanzamiento publicados por Release Engineering (tarballs, imágenes, manifests, firmas, llaves públicas).
- Has generado o recibido material de llaves de producción para tu rol de validador/observador (identidad de nodo Ed25519; llave de consenso BLS + PoP para validadores; mas cualquier alternancia de funciones confidenciales).
- Puedes alcanzar a los peers existentes de Sora Nexus que bootstrapean tu nodo.

## Paso 1 - Confirmar el perfil de lanzamiento
1. Identifica el alias de red o ID de cadena que te dieron.
2. Ejecuta `scripts/select_release_profile.py --network <alias>` (o `--chain-id <id>`) en un checkout de este repositorio. El ayudante consulta `release/network_profiles.toml` e imprime el perfil a desplegar. Para Sora Nexus la respuesta debe ser `iroha3`. Para cualquier otro valor, deténgase y contacte a Release Engineering.
3. Anota la etiqueta de versión que hace referencia al anuncio del lanzamiento (por ejemplo `iroha3-v3.2.0`); lo usaras para descargar artefactos y manifiestos.## Paso 2 - Recuperar y validar artefactos
1. Descargue el paquete `iroha3` (`<profile>-<version>-<os>.tar.zst`) y sus archivos compañeros (`.sha256`, opcional `.sig/.pub`, `<profile>-<version>-manifest.json`, y `<profile>-<version>-image.json` si despliegas contenedores).
2. Valida la integridad antes de descomprimir:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Reemplaza `openssl` con el verificador aprobado por la organización si usas un KMS con respaldo de hardware.
3. Inspecciona `PROFILE.toml` dentro del tarball y los manifests JSON para confirmar:
   - `profile = "iroha3"`
   - Los campos `version`, `commit` y `built_at` coinciden con el anuncio del lanzamiento.
   - El OS/arquitectura coincide con tu objetivo de implementación.
4. Si usas la imagen de contenedor, repite la verificación de hash/firma para `<profile>-<version>-<os>-image.tar` y confirma la imagen ID registrada en `<profile>-<version>-image.json`.## Paso 3 - Preparar configuración desde plantillas
1. Extrae el paquete y copia `config/` a la ubicación donde el nodo leerá su configuración.
2. Trata los archivos bajo `config/` como plantillas:
   - Reemplaza `public_key`/`private_key` con tus llaves Ed25519 de produccion. Elimina llaves privadas del disco si el nudo las obtiene de un HSM; Actualiza la configuración para apuntar al conector HSM.
   - Ajusta `trusted_peers`, `network.address` y `torii.address` para reflejar tus interfaces accesibles y los pares de bootstrap asignados.
   - Actualiza `client.toml` con el endpoint Torii de cara al operador (incluyendo configuración TLS si aplica) y las credenciales que aprovisionas para herramienta operativa.
3. Mantiene el chain ID provisto en el paquete a menos que Governance lo indique explícitamente: el carril global espera un identificador de cadena canonico unico.
4. Planea iniciar el nodo con el flag de perfil Sora: `irohad --sora --config <path>`. El cargador de configuración rechazará los ajustes de SoraFS o multicarril si la bandera está ausente.## Paso 4 - Metadatos alineales de espacio de datos y enrutamiento
1. Edita `config/config.toml` para que la sección `[nexus]` coincida con el catálogo de data-spaces proporcionado por el Nexus Council:
   - `lane_count` debe igualar el total de carriles habilitados en la época actual.
   - Cada entrada en `[[nexus.lane_catalog]]` y `[[nexus.dataspace_catalog]]` debe contener un `index`/`id` único y los alias acordados. No elimina las entradas globales existentes; agrega tus alias delegados si el consejo asigna data-spaces adicionales.
   - Asegúrese de que cada entrada de espacio de datos incluya `fault_tolerance (f)`; los comités lane-relay se dimensionan en `3f+1`.
2. Actualiza `[[nexus.routing_policy.rules]]` para capturar la política que te asignaron. La plantilla por defecto enruta instrucciones de gobernanza al carril `1` y despliegues de contratos al carril `2`; Agrega o modifica reglas para que el tráfico destinado a tu espacio de datos vaya al carril y alias correctos. Coordina con Release Engineering antes de cambiar el orden de reglas.
3. Revise los umbrales de `[nexus.da]`, `[nexus.da.audit]` y `[nexus.da.recovery]`. Se espera que los operadores mantengan los valores aprobados por el consejo; ajustalos solo si se ratifica una política actualizada.4. Registre la configuración final en su rastreador de operaciones. El runbook de liberación de doble vía requiere adjuntar el `config.toml` efectivo (con secretos redactados) al ticket de onboarding.

## Paso 5 - Validación previa
1. Ejecuta el validador de configuración integrado antes de unirte a la red:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Esto imprime la configuración resultante y falla temprano si las entradas de catalogo/routing son inconsistentes o si genesis y config no coinciden.
2. Si despliegas contenedores, ejecuta el mismo comando dentro de la imagen después de cargarla con `docker load -i <profile>-<version>-<os>-image.tar` (recuerda incluir `--sora`).
3. Revise los registros para advertencias sobre identificadores de marcador de posición de carril/espacio de datos. Si aparecen, regresa al Paso 4: los despliegues de producción no deben depender de los IDs placeholder que vienen con las plantillas.
4. Ejecuta tu procedimiento local de smoke (p. ej., enviar una consulta `FindNetworkStatus` con `iroha_cli`, confirmar que los endpoints de telemetria exponen `nexus_lane_state_total` y verificar que las llaves de streaming se rotaron o importaron segun corresponda).## Paso 6 - Transición y transferencia
1. Guarde el `manifest.json` verificado y los artefactos de firma en el ticket de liberación para que los auditores puedan reproducir sus verificaciones.
2. Notifica a Nexus Operations que el nodo está listo para ser introducido; incluye:
   - Identidad del nodo (ID de par, nombres de host, punto final Torii).
   - Valores efectivos de catálogo de lane/data-space y política de enrutamiento.
   - Hashes de los binarios/imagenes que verificaste.
3. Coordina la admision final de peers (gossip seeds y asignacion de lane) con `@nexus-core`. No te unas a la red hasta recibir aprobación; Sora Nexus aplica ocupación determinista de carriles y requiere un manifiesto de admisiones actualizado.
4. Después de que el nodo este en vivo, actualiza tus runbooks con cualquier override que introduzcas y anota la etiqueta de liberación para que la siguiente iteración arranque desde esta línea base.## Lista de verificación de referencia
- [ ] Perfil de lanzamiento validado como `iroha3`.
- [ ] Hashes y firmas del paquete/imagen verificados.
- [ ] Llaves, direcciones de peers y endpoints Torii actualizados a valores de producción.
- [ ] Catalogo de lanes/dataspace y politica de enrutamiento de Nexus coinciden con la asignacion del consejo.
- [ ] Validador de configuración (`irohad --sora --config ... --trace-config`) pasa sin advertencias.
- [ ] Manifiestos/firmas archivados en el ticket de onboarding y Ops notificado.

Para contexto adicional sobre fases de migración de Nexus y expectativas de telemetría, revisa [Nexus notas de transición](./nexus-transition-notes).