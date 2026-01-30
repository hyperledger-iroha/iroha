---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/nexus-operator-onboarding.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-operator-onboarding
title: Incorporacion de operadores de data-space de Sora Nexus
description: Espejo de `docs/source/sora_nexus_operator_onboarding.md`, que sigue la checklist de release end-to-end para operadores de Nexus.
---

:::note Fuente canonica
Esta pagina refleja `docs/source/sora_nexus_operator_onboarding.md`. Manten ambas copias alineadas hasta que las ediciones localizadas lleguen al portal.
:::

# Incorporacion de operadores de data-space de Sora Nexus

Esta guia captura el flujo end-to-end que deben seguir los operadores de data-space de Sora Nexus una vez se anuncia un release. Complementa el runbook de doble via (`docs/source/release_dual_track_runbook.md`) y la nota de seleccion de artefactos (`docs/source/release_artifact_selection.md`) al describir como alinear bundles/imagenes descargados, manifests y plantillas de configuracion con las expectativas de lanes globales antes de poner un nodo en linea.

## Audiencia y prerequisitos
- Has sido aprobado por el Programa Nexus y recibiste tu asignacion de data-space (indice de lane, data-space ID/alias y requisitos de politica de routing).
- Puedes acceder a los artefactos firmados del release publicados por Release Engineering (tarballs, imagenes, manifests, firmas, llaves publicas).
- Has generado o recibido material de llaves de produccion para tu rol de validator/observer (identidad de nodo Ed25519; llave de consenso BLS + PoP para validators; mas cualquier toggle de funciones confidenciales).
- Puedes alcanzar a los peers existentes de Sora Nexus que bootstrapean tu nodo.

## Paso 1 - Confirmar el perfil de release
1. Identifica el alias de red o chain ID que te dieron.
2. Ejecuta `scripts/select_release_profile.py --network <alias>` (o `--chain-id <id>`) en un checkout de este repositorio. El helper consulta `release/network_profiles.toml` e imprime el perfil a desplegar. Para Sora Nexus la respuesta debe ser `iroha3`. Para cualquier otro valor, deten y contacta a Release Engineering.
3. Anota el tag de version que referencio el anuncio del release (por ejemplo `iroha3-v3.2.0`); lo usaras para descargar artefactos y manifests.

## Paso 2 - Recuperar y validar artefactos
1. Descarga el bundle `iroha3` (`<profile>-<version>-<os>.tar.zst`) y sus archivos companeros (`.sha256`, opcional `.sig/.pub`, `<profile>-<version>-manifest.json`, y `<profile>-<version>-image.json` si despliegas contenedores).
2. Valida la integridad antes de descomprimir:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Reemplaza `openssl` con el verificador aprobado por la organizacion si usas un KMS con respaldo de hardware.
3. Inspecciona `PROFILE.toml` dentro del tarball y los manifests JSON para confirmar:
   - `profile = "iroha3"`
   - Los campos `version`, `commit` y `built_at` coinciden con el anuncio del release.
   - El OS/arquitectura coinciden con tu objetivo de despliegue.
4. Si usas la imagen de contenedor, repite la verificacion de hash/firma para `<profile>-<version>-<os>-image.tar` y confirma el image ID registrado en `<profile>-<version>-image.json`.

## Paso 3 - Preparar configuracion desde plantillas
1. Extrae el bundle y copia `config/` a la ubicacion donde el nodo leera su configuracion.
2. Trata los archivos bajo `config/` como plantillas:
   - Reemplaza `public_key`/`private_key` con tus llaves Ed25519 de produccion. Elimina llaves privadas del disco si el nodo las obtiene de un HSM; actualiza la configuracion para apuntar al conector HSM.
   - Ajusta `trusted_peers`, `network.address` y `torii.address` para reflejar tus interfaces accesibles y los peers de bootstrap asignados.
   - Actualiza `client.toml` con el endpoint Torii de cara al operador (incluyendo configuracion TLS si aplica) y las credenciales que aprovisionas para tooling operativo.
3. Mantiene el chain ID provisto en el bundle a menos que Governance lo indique explicitamente: el lane global espera un identificador de cadena canonico unico.
4. Planea iniciar el nodo con el flag de perfil Sora: `irohad --sora --config <path>`. El loader de configuracion rechazara ajustes de SoraFS o multi-lane si el flag esta ausente.

## Paso 4 - Alinear metadata de data-space y routing
1. Edita `config/config.toml` para que la seccion `[nexus]` coincida con el catalogo de data-spaces proporcionado por el Nexus Council:
   - `lane_count` debe igualar el total de lanes habilitados en la epoca actual.
   - Cada entrada en `[[nexus.lane_catalog]]` y `[[nexus.dataspace_catalog]]` debe contener un `index`/`id` unico y los alias acordados. No elimines las entradas globales existentes; agrega tus alias delegados si el consejo asigno data-spaces adicionales.
   - Asegura que cada entrada de dataspace incluya `fault_tolerance (f)`; los comites lane-relay se dimensionan en `3f+1`.
2. Actualiza `[[nexus.routing_policy.rules]]` para capturar la politica que te asignaron. La plantilla por defecto enruta instrucciones de gobernanza al lane `1` y despliegues de contratos al lane `2`; agrega o modifica reglas para que el trafico destinado a tu data-space vaya al lane y alias correctos. Coordina con Release Engineering antes de cambiar el orden de reglas.
3. Revisa los umbrales de `[nexus.da]`, `[nexus.da.audit]` y `[nexus.da.recovery]`. Se espera que los operadores mantengan los valores aprobados por el consejo; ajustalos solo si se ratifico una politica actualizada.
4. Registra la configuracion final en tu tracker de operaciones. El runbook de release de doble via requiere adjuntar el `config.toml` efectivo (con secretos redactados) al ticket de onboarding.

## Paso 5 - Validacion previa
1. Ejecuta el validador de configuracion integrado antes de unirte a la red:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Esto imprime la configuracion resuelta y falla temprano si las entradas de catalogo/routing son inconsistentes o si genesis y config no coinciden.
2. Si despliegas contenedores, ejecuta el mismo comando dentro de la imagen despues de cargarla con `docker load -i <profile>-<version>-<os>-image.tar` (recuerda incluir `--sora`).
3. Revisa logs para advertencias sobre identificadores placeholder de lane/data-space. Si aparecen, regresa al Paso 4: los despliegues de produccion no deben depender de los IDs placeholder que vienen con las plantillas.
4. Ejecuta tu procedimiento local de smoke (p. ej., enviar una consulta `FindNetworkStatus` con `iroha_cli`, confirmar que los endpoints de telemetria exponen `nexus_lane_state_total` y verificar que las llaves de streaming se rotaron o importaron segun corresponda).

## Paso 6 - Cutover y hand-off
1. Guarda el `manifest.json` verificado y los artefactos de firma en el ticket de release para que los auditores puedan reproducir tus verificaciones.
2. Notifica a Nexus Operations que el nodo esta listo para ser introducido; incluye:
   - Identidad del nodo (peer ID, hostnames, endpoint Torii).
   - Valores efectivos de catalogo de lane/data-space y politica de routing.
   - Hashes de los binarios/imagenes que verificaste.
3. Coordina la admision final de peers (gossip seeds y asignacion de lane) con `@nexus-core`. No te unas a la red hasta recibir aprobacion; Sora Nexus aplica ocupacion determinista de lanes y requiere un manifest de admisiones actualizado.
4. Despues de que el nodo este en vivo, actualiza tus runbooks con cualquier override que introdujiste y anota el tag de release para que la siguiente iteracion arranque desde esta baseline.

## Checklist de referencia
- [ ] Perfil de release validado como `iroha3`.
- [ ] Hashes y firmas del bundle/imagen verificados.
- [ ] Llaves, direcciones de peers y endpoints Torii actualizados a valores de produccion.
- [ ] Catalogo de lanes/dataspace y politica de routing de Nexus coincide con la asignacion del consejo.
- [ ] Validador de configuracion (`irohad --sora --config ... --trace-config`) pasa sin advertencias.
- [ ] Manifests/firmas archivados en el ticket de onboarding y Ops notificado.

Para contexto adicional sobre fases de migracion de Nexus y expectativas de telemetria, revisa [Nexus transition notes](./nexus-transition-notes).
