---
lang: es
direction: ltr
source: docs/source/crypto/sm_config_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee9b1be07edfee6d71031362a5ea95138a6b743a7e596537c1b1c02ce8edef9f
source_last_modified: "2026-01-22T15:38:30.660147+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Migración de configuración de SM

# Migración de configuración de SM

La implementación del conjunto de funciones SM2/SM3/SM4 requiere más que compilar con el
Indicador de característica `sm`. Los nodos controlan la funcionalidad detrás de las capas.
Perfiles `iroha_config` y se espera que el manifiesto de génesis lleve coincidencias
valores predeterminados. Esta nota captura el flujo de trabajo recomendado al promocionar un
red existente de “solo Ed25519” a “habilitada para SM”.

## 1. Verificar el perfil de compilación

- Compile los binarios con `--features sm`; agregue `sm-ffi-openssl` solo cuando
  Planea ejercitar la ruta de vista previa de OpenSSL/Tongsuo. Se construye sin `sm`
  La función rechaza las firmas `sm2` durante la admisión incluso si la configuración habilita
  ellos.
- Confirmar que CI publica los artefactos `sm` y que todos los pasos de validación (`cargo
  prueba -p iroha_crypto --features sm`, accesorios de integración, fuzz suites) pase
  en los archivos binarios exactos que desea implementar.

## 2. Anulaciones de configuración de capas

`iroha_config` aplica tres niveles: `defaults` → `user` → `actual`. Enviar el SM
anulaciones en el perfil `actual` que los operadores distribuyen a los validadores y
deje `user` en Ed25519-solo para que los valores predeterminados del desarrollador permanezcan sin cambios.

```toml
# defaults/actual/config.toml
[crypto]
enable_sm_openssl_preview = false         # flip to true only when the preview backend is rolled out
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]      # keep sorted for deterministic manifests
sm2_distid_default = "CN12345678901234"   # organisation-specific distinguishing identifier
```

Copie el mismo bloque en el manifiesto `defaults/genesis` a través de `kagami genesis
generar …` (add `--sm2 de firma permitida --default-hash sm3-256` si es necesario
anulaciones) para que el bloque `parameters` y los metadatos inyectados concuerden con el
configuración del tiempo de ejecución. Los pares se niegan a iniciar cuando el manifiesto y la configuración
las instantáneas divergen.

## 3. Regenerar los Manifiestos del Génesis

- Ejecute `kagami genesis generate --consensus-mode <mode>` para cada
  entorno y confirme el JSON actualizado junto con las anulaciones de TOML.
- Firme el manifiesto (`kagami genesis sign …`) y distribuya la carga útil `.nrt`.
  Los nodos que arrancan desde un manifiesto JSON sin firmar derivan la criptografía en tiempo de ejecución
  configuración directamente desde el archivo, aún sujeta a la misma coherencia
  cheques.

## 4. Validar antes del tráfico

- Aprovisione un clúster provisional con los nuevos archivos binarios y configuración, luego verifique:
  - `/status` expone `crypto.sm_helpers_available = true` una vez que los pares se reinician.
  - La admisión Torii aún rechaza las firmas SM2 mientras que `sm2` está ausente
    `allowed_signing` y acepta lotes mixtos Ed25519/SM2 cuando la lista
    Incluye ambos algoritmos.
  - Material clave de ida y vuelta `iroha_cli tools crypto sm2 export …` sembrado a través del nuevo
    valores predeterminados.
- Ejecute los scripts de humo de integración que cubren firmas deterministas SM2 y
  Hashing SM3 para confirmar la coherencia del host/VM.

## 5. Plan de reversión- Documente la reversión: elimine `sm2` de `allowed_signing` y restaure
  `default_hash = "blake2b-256"`. Empuje el cambio a través del mismo `actual`
  canalización de perfil para que cada validador gire monótonamente.
- Mantener los manifiestos SM en el disco; pares que ven configuración y génesis no coincidentes
  los datos se niegan a iniciarse, lo que protege contra reversiones parciales.
- Si se trata de la vista previa de OpenSSL/Tongsuo, incluya los pasos para deshabilitarla
  `crypto.enable_sm_openssl_preview` y eliminando los objetos compartidos del
  entorno de ejecución.

## Material de referencia

- [`docs/genesis.md`](../../genesis.md) – estructura de la génesis manifiesta y
  el bloque `crypto`.
-[`docs/source/references/configuration.md`](../references/configuration.md) –
  descripción general de las secciones y valores predeterminados de `iroha_config`.
- [`docs/source/crypto/sm_operator_rollout.md`](sm_operator_rollout.md) – fin de
  Lista de verificación del operador final para el envío de criptografía SM.