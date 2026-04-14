<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/source/isi_extension_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9648381ac7cc1716ffd3c48aca425ed17a6afe1ac73bdeff866ebbbd9147cf68
source_last_modified: "2026-03-30T18:22:55.972718+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Plan de extensión ISI (v1)

Esta nota firma el orden de prioridad para las nuevas Instrucciones Especiales Iroha y captura
invariantes no negociables para cada instrucción antes de la implementación. El orden coincide
Primero, el riesgo de seguridad y operatividad y, en segundo lugar, el rendimiento de UX.

## Pila de prioridades

1. **RotateAccountSignatory**: necesario para una rotación higiénica de claves sin migraciones destructivas.
2. **DeactivateContractInstance** / **RemoveSmartContractBytes**: proporciona un contrato determinista
   Kill Switches y recuperación de almacenamiento para implementaciones comprometidas.
3. **SetAssetKeyValue** / **RemoveAssetKeyValue**: ampliar la paridad de metadatos a activos concretos
   saldos para que las herramientas de observabilidad puedan etiquetar las participaciones.
4. **BatchMintAsset** / **BatchTransferAsset**: ayudas deterministas de distribución para mantener el tamaño de la carga útil
   y la presión de reserva de VM es manejable.

## Invariantes de instrucción

### EstablecerAssetKeyValue / EliminarAssetKeyValue
- Reutilice el espacio de nombres `AssetMetadataKey` (`state.rs`) para que las claves WSV canónicas se mantengan estables.
- Aplique límites de esquema y tamaño JSON de manera idéntica a los asistentes de metadatos de la cuenta.
- Emitir `AssetEvent::MetadataInserted` / `AssetEvent::MetadataRemoved` con el `AssetId` afectado.
- Requerir los mismos tokens de permiso que las ediciones de metadatos de activos existentes (propietario de la definición O
  subvenciones estilo `CanModifyAssetMetadata`).
- Cancelar si falta el registro del activo (sin creación implícita).### Rotar firmante de cuenta
- Intercambio atómico del firmante en `AccountId` preservando los metadatos de la cuenta y vinculados
  recursos (activos, activadores, roles, permisos, eventos pendientes).
- Verificar que el firmante actual coincida con la persona que llama (o la autoridad delegada mediante un token explícito).
- Rechazar si la nueva clave pública ya respalda otra cuenta canónica.
- Actualice todas las claves canónicas que incrustan el ID de la cuenta e invaliden los cachés antes de la confirmación.
- Emitir un `AccountEvent::SignatoryRotated` dedicado con claves antiguas/nuevas para pistas de auditoría.
- Andamio de migración: confíe en `AccountAlias` + `AccountRekeyRecord` (consulte `account::rekey`), por lo que
  Las cuentas existentes pueden mantener enlaces de alias estables durante una actualización continua sin interrupciones de hash.

### Desactivar instancia de contrato
- Eliminar o eliminar el enlace `(namespace, contract_id)` mientras se conservan los datos de procedencia.
  (quién, cuándo, código de motivo) para solucionar problemas.
- Requiere el mismo permiso de gobierno establecido como activación, con enlaces de políticas para no permitir
  desactivación de espacios de nombres del sistema central sin aprobación elevada.
- Rechazar cuando la instancia ya esté inactiva para mantener los registros de eventos deterministas.
- Emite un `ContractInstanceEvent::Deactivated` que los observadores posteriores pueden consumir.### EliminarBytes de contrato inteligente
- Permitir la poda del código de bytes almacenado por `code_hash` solo cuando no haya manifiestos ni instancias activas.
  hacer referencia al artefacto; de lo contrario fallará con un error descriptivo.
- Registro de espejos de puerta de permiso (`CanRegisterSmartContractCode`) más un nivel de operador
  guardia (por ejemplo, `CanManageSmartContractStorage`).
- Verifique que el `code_hash` proporcionado coincida con el resumen del cuerpo almacenado justo antes de la eliminación para evitar
  mangos rancios.
- Emitir `ContractCodeEvent::Removed` con hash y metadatos de la persona que llama.

### Activo de menta por lotes / Activo de transferencia por lotes
- Semántica de todo o nada: o cada tupla tiene éxito o la instrucción aborta sin lado
  efectos.
- Los vectores de entrada deben estar ordenados de manera determinista (sin clasificación implícita) y delimitados por la configuración.
  (`max_batch_isi_items`).
- Emitir eventos de activos por artículo para que la contabilidad posterior se mantenga consistente; el contexto por lotes es aditivo,
  no es un reemplazo.
- Las comprobaciones de permisos reutilizan la lógica de elemento único existente por objetivo (propietario del activo, propietario de la definición,
  o capacidad concedida) antes de la mutación del estado.
- Los conjuntos de acceso de asesoramiento deben unir todas las claves de lectura/escritura para mantener correcta la simultaneidad optimista.

## Andamiaje de implementación- El modelo de datos ahora incluye andamios `SetAssetKeyValue` / `RemoveAssetKeyValue` para metadatos de balanza
  ediciones (`transparent.rs`).
- Los visitantes ejecutores exponen marcadores de posición que bloquearán los permisos una vez que llegue el cableado del host.
  (`default/mod.rs`).
- Los tipos de prototipos de recodificación (`account::rekey`) proporcionan una zona de aterrizaje para migraciones continuas.
- El estado mundial incluye `account_rekey_records` codificado por `AccountAlias` para que podamos preparar el alias →
  migraciones de firmantes sin tocar la codificación histórica `AccountId`.

## IVM Redacción de llamada al sistema

- Cuñas de host para el envío `DeactivateContractInstance` / `RemoveSmartContractBytes` como
  `SYSCALL_DEACTIVATE_CONTRACT_INSTANCE` (0x43) y
  `SYSCALL_REMOVE_SMART_CONTRACT_BYTES` (0x44), ambos consumen TLV Norito que reflejan el
  estructuras ISI canónicas.
- Extienda `abi_syscall_list()` solo después de que los controladores de host reflejen las rutas de ejecución de `iroha_core` para mantener
  Los hashes ABI son estables durante el desarrollo.
- La actualización Kotodama se reduce una vez que se estabilizan los números de llamadas al sistema; agregue cobertura dorada para el expandido
  superficie al mismo tiempo.

## Estado

El orden y las invariantes anteriores están listos para su implementación. Las sucursales de seguimiento deben hacer referencia
este documento al cablear rutas de ejecución y exposición a llamadas al sistema.