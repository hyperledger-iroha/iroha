<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/source/soracloud/cli_local_control_plane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 567b63e9b61afaecfa5d85aa60f0348c856557e171559885ffaba45168ce61dc
source_last_modified: "2026-03-26T06:12:11.480025+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud CLI y plano de control

Soracloud v1 es un tiempo de ejecución autorizado exclusivo de IVM.

- `iroha app soracloud init` es el único comando sin conexión. es un andamio
  `container_manifest.json`, `service_manifest.json` y plantilla opcional
  artefactos para los servicios de Soracloud.
- Todos los demás comandos CLI de Soracloud están respaldados únicamente por la red y requieren
  `--torii-url`.
- La CLI no mantiene ningún espejo o estado del plano de control de Soracloud local.
  archivo.
- Torii sirve el estado público de Soracloud y las rutas de mutación directamente desde
  estado mundial autorizado más el administrador de tiempo de ejecución integrado de Soracloud.

## Alcance del tiempo de ejecución- Soracloud v1 acepta solo `SoraContainerRuntimeV1::Ivm`.
- `NativeProcess` sigue rechazado.
- Las ejecuciones de buzón ordenadas admitieron controladores IVM directamente.
- La hidratación y la materialización provienen del contenido comprometido SoraFS/DA en lugar de
  que las instantáneas locales sintéticas.
- `SoraContainerManifestV1` ahora lleva `required_config_names` y
  `required_secret_names`, más `config_exports` explícito. Implementar, actualizar,
  y la reversión fallan cerradas cuando el conjunto de material autorizado efectivo
  no satisface esos enlaces declarados o cuando una exportación de configuración tiene como objetivo un
  configuración no requerida o un destino de archivo/env duplicado.
- Las entradas de configuración de servicio comprometidas ahora se materializan en
  `services/<service>/<version>/configs/<config_name>` como JSON canónico
  archivos de carga útil.
- Las exportaciones de entorno de configuración explícita se proyectan en
  `services/<service>/<version>/effective_env.json`, y las exportaciones de archivos son
  materializado bajo
  `services/<service>/<version>/config_exports/<relative_path>`. Exportado
  Los valores utilizan el texto de carga útil JSON canónico de la entrada de configuración a la que se hace referencia.
- Los controladores Soracloud IVM ahora pueden leer esas cargas útiles de configuración autorizadas.
  directamente a través de la superficie del host de tiempo de ejecución `ReadConfig`, por lo que es normal
  Los controladores `query`/`update` no necesitan adivinar las rutas de los archivos locales del nodo solo para
  consumir configuración de servicio comprometida.
- Los sobres secretos de servicio comprometido ahora se materializan en
  `services/<service>/<version>/secret_envelopes/<secret_name>` como
  archivos de sobre autorizados.- Los controladores ordinarios de Soracloud IVM ahora pueden leer los secretos comprometidos.
  sobres directamente a través de la superficie del host de tiempo de ejecución `ReadSecretEnvelope`.
- El árbol de reserva de tiempo de ejecución privado heredado ahora está sincronizado desde el compromiso
  estado de implementación bajo `secrets/<service>/<version>/<secret_name>` para que el
  ruta de lectura secreta sin procesar más antigua y el punto del plano de control autorizado en el
  mismos bytes.
- El tiempo de ejecución privado `ReadSecret` ahora resuelve la implementación autorizada
  `service_secrets` primero y solo recurre al nodo local heredado
  `secrets/<service>/<version>/...` árbol de archivos materializado cuando no se ha confirmado
  Existe una entrada secreta de servicio para la clave solicitada.
- La ingesta secreta sigue siendo intencionalmente más limitada que la ingesta de configuración:
  `ReadSecretEnvelope` es el contrato de manejo ordinario seguro para el público, mientras que
  `ReadSecret` permanece solo en tiempo de ejecución privado y aún devuelve el compromiso
  bytes de texto cifrado del sobre en lugar de un contrato de montaje de texto plano.
- Los planes de servicio en tiempo de ejecución ahora exponen la capacidad de ingesta correspondiente.
  booleanos más el `config_exports` declarado y el proyectado efectivo
  entorno, por lo que los consumidores de estado pueden saber si una revisión materializada
  admite lecturas de configuración de host, lecturas de sobre secreto de host, secreto privado sin formato
  lecturas e inyección de configuración explícita sin inferirla del controlador
  clases solo.

## Comandos CLI- `iroha app soracloud init`
  - Solo andamio fuera de línea.
  - admite plantillas `baseline`, `site`, `webapp` e `pii-app`.
- `iroha app soracloud deploy`
  - valida localmente las reglas de admisión `SoraDeploymentBundleV1`, firma el
    solicitud y llama a `POST /v1/soracloud/deploy`.
  - `--initial-configs <path>` e `--initial-secrets <path>` ahora pueden conectarse
    configuración autorizada del servicio en línea/mapas secretos atómicamente con el
    primer despliegue para que las vinculaciones requeridas puedan cumplirse en la primera admisión.
  - la CLI ahora firma canónicamente la solicitud HTTP con cualquiera de los dos
    `X-Iroha-Account`, `X-Iroha-Signature`, `X-Iroha-Timestamp-Ms` y
    `X-Iroha-Nonce` para cuentas ordinarias de firma única, o
    `X-Iroha-Account` más `X-Iroha-Witness` cuando
    `soracloud.http_witness_file` apunta a una carga útil JSON de testigo multifirma;
    Torii devuelve un borrador de conjunto de instrucciones de transacción determinista y el
    Luego, CLI envía la transacción real a través del cliente Iroha normal.
    carril.
  - Torii también aplica límites de admisión de host SCR y capacidad de cierre fallido
    controles antes de que se acepte la mutación.
- `iroha app soracloud upgrade`
  - valida y firma una nueva revisión del paquete, luego llama
    `POST /v1/soracloud/upgrade`.
  - el mismo flujo `--initial-configs <path>` / `--initial-secrets <path>` es
    disponible para actualizaciones de material atómico durante la actualización.
  - Las mismas comprobaciones de admisión del host SCR se ejecutan en el lado del servidor antes de realizar la actualización.
    admitido.
- `iroha app soracloud status`- Consulta el estado del servicio autorizado desde `GET /v1/soracloud/status`.
- `iroha app soracloud config-*`
  - `config-set`, `config-delete` e `config-status` están respaldados únicamente por Torii.
  - La CLI firma llamadas y cargas útiles de procedencia de configuración de servicio canónica
    `POST /v1/soracloud/service/config/set`,
    `POST /v1/soracloud/service/config/delete`, y
    `GET /v1/soracloud/service/config/status`.
  - Las entradas de configuración persisten en el estado de implementación autorizada y permanecen
    adjunto a través de cambios de revisión de implementación/actualización/reversión.
  - `config-delete` ahora falla cuando la revisión activa aún declara
    la configuración nombrada en `container.required_config_names`.
- `iroha app soracloud secret-*`
  - `secret-set`, `secret-delete` y `secret-status` están respaldados únicamente por Torii.
  - La CLI firma llamadas y cargas útiles de procedencia secreta del servicio canónico
    `POST /v1/soracloud/service/secret/set`,
    `POST /v1/soracloud/service/secret/delete`, y
    `GET /v1/soracloud/service/secret/status`.
  - las entradas secretas se conservan como registros autorizados `SecretEnvelopeV1`
    en estado de implementación y sobrevivir a los cambios normales de revisión del servicio.
  - `secret-delete` ahora falla cuando la revisión activa aún declara
    el secreto nombrado en `container.required_secret_names`.
- `iroha app soracloud rollback`
  - firma metadatos de reversión y llama a `POST /v1/soracloud/rollback`.
- `iroha app soracloud rollout`
  - firma metadatos de implementación y llama a `POST /v1/soracloud/rollout`.
- `iroha app soracloud agent-*`
  - todos los comandos del ciclo de vida del apartamento, billetera, buzón y autonomía son
    Solo con respaldo Torii.
- `iroha app soracloud training-*`
  - Todos los comandos de trabajos de entrenamiento están respaldados únicamente por Torii.- `iroha app soracloud model-*`
  - todos los comandos de artefacto, peso, modelo cargado y tiempo de ejecución privado del modelo
    están respaldados únicamente por Torii.
  - la superficie de modelo cargado/tiempo de ejecución privado ahora vive en la misma familia:
    `model-upload-encryption-recipient`, `model-upload-init`,
    `model-upload-chunk`, `model-upload-finalize`, `model-upload-status`,
    `model-compile`, `model-compile-status`, `model-allow`,
    `model-run-private`, `model-run-status`, `model-decrypt-output` y
    `model-publish-private`.
  - `model-run-private` ahora oculta el protocolo de enlace en tiempo de ejecución de borrador y luego finalización en
    la CLI y devuelve el estado autorizado de la sesión posterior a la finalización.
  - `model-publish-private` ahora admite tanto un preparado
    paquete/fragmento/finalizar/compilar/permitir plan de publicación y un nivel superior
    borrador del documento. El borrador ahora lleva `source: PrivateModelSourceV1`,
    que acepta `LocalDir { path }` o
    `HuggingFaceSnapshot { repo, revision }`.
  - cuando se llama con `--draft-file`, la CLI normaliza la fuente declarada
    en un árbol temporal determinista, valida el contrato de tensores de seguridad v1 HF,
    serializa y cifra de manera determinista el paquete contra el activo
    Torii carga el destinatario, lo fragmenta en fragmentos cifrados de tamaño fijo,
    Opcionalmente escribe el plan preparado a través de `--emit-plan-file` y luego
    ejecuta la secuencia cargar/finalizar/compilar/permitir.
  - Las revisiones de `HuggingFaceSnapshot` son obligatorias y se deben fijar como confirmación.
    SHA; Las referencias tipo rama se rechazan y se cierran por error.- el diseño de la fuente admitida es intencionalmente estrecho en v1: `config.json`,
    activos del tokenizador, uno o más fragmentos `*.safetensors` y opciones
    Metadatos de procesador/preprocesador para modelos con capacidad de imagen. GGUF, ONNX,
    Se incluyen otros pesos que no son tensores de seguridad y diseños personalizados anidados arbitrarios.
    rechazado.
  - cuando se llama con `--plan-file`, la CLI aún consume un ya
    documento de plan de publicación preparado y cierre fallido cuando se carga el plan
    El destinatario ya no coincide con el destinatario autorizado Torii.
  - ver `uploaded_private_models.md` para ver el diseño que superpone esas rutas
    en el registro de modelos existente y en los registros de artefactos/peso.
- Rutas del plano de control `model-host`
  - Torii ahora expone autorización
    `POST /v1/soracloud/model-host/advertise`,
    `POST /v1/soracloud/model-host/heartbeat`,
    `POST /v1/soracloud/model-host/withdraw`, y
    `GET /v1/soracloud/model-host/status`.
  - estas rutas persisten en los anuncios de capacidad del host del validador optativo en
    estado mundial autorizado y permitir que los operadores inspeccionen qué validadores están
    Actualmente anuncia capacidad de modelo-host.
  - `iroha app soracloud model-host-advertise`,
    `model-host-heartbeat`, `model-host-withdraw` y
    `model-host-status` ahora firma las mismas cargas útiles de procedencia canónica que el
    API sin formato y llame directamente a las rutas Torii coincidentes.
- `iroha app soracloud hf-*`
  - `hf-deploy`, `hf-status`, `hf-lease-leave` e `hf-lease-renew` son
    Solo con respaldo Torii.- `hf-deploy` e `hf-lease-renew` ahora también admiten automáticamente el determinista
    servicio de inferencia HF generado para el `service_name` solicitado, y
    admitir automáticamente el apartamento HF generado determinista para
    `apartment_name` cuando se solicita uno, antes de la mutación de arrendamiento compartido
    se presenta.
  - la reutilización se cierra a prueba de fallos: si el servicio/apartamento nombrado ya existe pero está
    no es el despliegue de HF generado esperado para esa fuente canónica, el HF
    La mutación se rechaza en lugar de vincular silenciosamente el contrato de arrendamiento a personas no relacionadas.
    Objetos de Soracloud.
  - cuando se adjunta el administrador de tiempo de ejecución integrado, ahora también `hf-status`
    devuelve una proyección en tiempo de ejecución para la fuente canónica, incluido el enlace
    servicios/apartamentos, visibilidad en la siguiente ventana en cola y paquete/
    fallas en el caché de artefactos; `importer_pending` sigue esa proyección de tiempo de ejecución
    en lugar de confiar únicamente en la enumeración de fuentes autorizadas.
  - cuando `hf-deploy` o `hf-lease-renew` admite el servicio HF generado en
    la misma transacción que la mutación de arrendamiento compartido, la autoridad HF
    la fuente ahora cambia a `Ready` inmediatamente y `importer_pending` permanece
    `false` en la respuesta.
  - El estado de arrendamiento de HF y las respuestas a las mutaciones ahora también exponen cualquier autoridad
    instantánea de ubicación ya adjunta a la ventana de arrendamiento activa, incluidahosts asignados, recuento de hosts elegibles, recuento de hosts calientes y
    campos de tarifa de almacenamiento versus computación.
  - `hf-deploy` e `hf-lease-renew` ahora derivan el recurso HF canónico
    perfil de los metadatos del repositorio de Hugging Face resueltos antes de enviar el
    mutación:
    - Torii inspecciona el repositorio `siblings`, prefiere `.gguf` a
      `.safetensors` sobre diseños de peso de PyTorch, dirige los archivos seleccionados a
      deriva `required_model_bytes` y lo asigna a una primera versión
      backend/formato más RAM/pisos de disco;
    - La admisión de arrendamiento falla y se cierra cuando no se puede publicar ningún anuncio del host del validador en vivo
      satisfacer ese perfil; y
    - cuando un conjunto de hosts está disponible, la ventana activa ahora registra un
      colocación determinista ponderada por participación y una reserva de cálculo separada
      tarifa junto con la contabilidad de arrendamiento de almacenamiento existente.
  - Los miembros posteriores que se unan a una ventana HF activa ahora pagan almacenamiento prorrateado y
    calcular las acciones sólo para la ventana restante, mientras que los miembros anteriores
    recibir la misma contabilidad determinista de reembolso de almacenamiento y reembolso de cálculo
    de esa unión tardía.
  - el administrador de tiempo de ejecución integrado puede sintetizar el paquete de código auxiliar HF generado
    localmente, para que esos servicios generados puedan materializarse sin esperar una
    carga útil SoraFS comprometida solo para el paquete de inferencia de marcador de posición.- El administrador de tiempo de ejecución integrado ahora también importa el repositorio Hugging Face incluido en la lista permitida.
    archivos en `soracloud_runtime.state_dir/hf_sources/<source_id>/files/` y
    persiste un `import_manifest.json` local con el compromiso resuelto, importado
    archivos, archivos omitidos y cualquier error del importador.
  - las lecturas locales HF `metadata` generadas ahora devuelven ese manifiesto de importación local,
    incluido el inventario de archivos importados y si la ejecución local y
    El respaldo del puente está habilitado para el nodo.
  - Las lecturas locales generadas por HF `infer` ahora prefieren la ejecución en el nodo frente al
    bytes compartidos importados:
    - `irohad` materializa un script de adaptador Python integrado en el local
      Directorio de estado de tiempo de ejecución de Soracloud y lo invoca a través de
      `soracloud_runtime.hf.local_runner_program`;
    - el corredor incrustado primero verifica si hay una estrofa de fijación determinista en
      `config.json` (utilizado por las pruebas); de lo contrario, carga la fuente importada
      directorio a través de `transformers.pipeline(..., local_files_only=True)` entonces
      el modelo se ejecuta contra la importación local compartida en lugar de tirar
      bytes nuevos del concentrador; y
    - si `soracloud_runtime.hf.allow_inference_bridge_fallback = true` y
      `soracloud_runtime.hf.inference_token` está configurado, el tiempo de ejecución cae
      volver a la URL base de HF Inference configurada solo cuando la ejecución local
      no está disponible o falla y la persona que llama explícitamente opta por
      `x-soracloud-hf-allow-bridge-fallback: 1`, `true` o `yes`.
  - la proyección en tiempo de ejecución ahora mantiene una fuente HF en `PendingImport` hasta queexiste un manifiesto de importación local exitoso y las fallas del importador surgen como
    tiempo de ejecución `Failed` más `last_error` en lugar de informar silenciosamente `Ready`.
  - Los apartamentos HF generados ahora consumen autonomía aprobada a través del
    ruta de ejecución local del nodo:
    - `agent-autonomy-run` ahora sigue un flujo de dos pasos: el primero firmado
      La mutación registra la aprobación autorizada y devuelve un determinista.
      borrador de la transacción, luego una segunda solicitud de finalización firmada le pide al
      administrador de tiempo de ejecución integrado para ejecutar esa ejecución aprobada contra el límite
      generó el servicio HF `/infer` y devuelve cualquier seguimiento autorizado
      instrucciones como otro borrador determinista;
    - el registro de ejecución aprobado ahora también persiste como canónico
      `request_commitment`, por lo que el recibo de servicio generado posteriormente se puede
      vinculado a la aprobación exacta de la autonomía autoritaria;
    - Las aprobaciones ahora pueden persistir en un `workflow_input_json` canónico opcional.
      cuerpo; cuando está presente, el tiempo de ejecución incorporado reenvía esa carga útil JSON exacta
      al controlador HF `/infer` generado y, cuando está ausente, vuelve a
      el sobre `run_label`-as-`inputs` más antiguo con la autorización
      `artifact_hash` / `provenance_hash` / `budget_units` / `run_id` llevado
      como parámetros estructurados;
    - `workflow_input_json` ahora también puede optar por secuencial deterministaejecución de varios pasos con
      `{ "workflow_version": 1, "steps": [...] }`, donde cada paso ejecuta un
      La solicitud HF `/infer` generada y los pasos posteriores pueden hacer referencia a resultados anteriores
      a través de `${run.*}`, `${previous.text|json|result_commitment}` y
      Marcadores de posición `${steps.<step_id>.text|json|result_commitment}`; y
    - tanto la respuesta a la mutación como `agent-autonomy-status` ahora emergen al
      resumen de ejecución local del nodo cuando esté disponible, incluido el éxito/fracaso,
      la revisión del servicio vinculado, compromisos de resultados deterministas, punto de control
      / hashes de artefactos de diario, el servicio generado `AuditReceipt` y el
      cuerpo de respuesta JSON analizado.
    - cuando el recibo de servicio generado está presente, Torii lo registra en
      autorizado `soracloud_runtime_receipts` y expone el resultado
      recibo de tiempo de ejecución autorizado sobre el estado de ejecución reciente junto con el
      resumen de ejecución local del nodo.
    - la ruta de autonomía generada en HF ahora también registra una autoridad dedicada
      evento de auditoría del apartamento `AutonomyRunExecuted` y estado de ejecución reciente
      devuelve esa auditoría de ejecución junto con el recibo autorizado del tiempo de ejecución.
  - `hf-lease-renew` ahora tiene dos modos:
    - si la ventana actual ha caducado o se ha agotado, inmediatamente se abre una nueva
      ventana;
    - si la ventana actual todavía está activa, pone en cola a la persona que llama como
      patrocinador de la siguiente ventana, cobra el almacenamiento y la computación completos de la siguiente ventanatarifas de reserva por adelantado, persiste la siguiente ventana determinista
      plan de colocación y expone ese patrocinio en cola a través de `hf-status`
      hasta que una mutación posterior haga avanzar el grupo.
  - El ingreso `/infer` público de HF generado ahora resuelve la autoridad
    ubicación y, cuando el nodo receptor no es el principal cálido, representa el nodo
    solicitud a través de mensajes de control P2P de Soracloud al host principal asignado;
    el tiempo de ejecución incorporado todavía falla al cerrarse en una réplica directa/local no asignado
    la ejecución y los recibos generados en tiempo de ejecución de HF llevan `placement_id`,
    validador y atribución de pares del registro de colocación autorizado.
  - cuando la ruta de proxy a principal expira, se cierra antes de una respuesta, o
    regresa con una falla de tiempo de ejecución no cliente por parte de la autoridad
    primario, el nodo de ingreso ahora informa `AssignedHeartbeatMiss` para ese
    primario y pone en cola `ReconcileSoracloudModelHosts` a través del mismo
    carril de mutación interna.
  - La reconciliación autorizada del host caducado ahora persiste en los registros.
    evidencia de violación de modelo-host, reutiliza la ruta de barra del validador de carril público,
    y aplica la política de penalización predeterminada por arrendamiento compartido de HF:
    `warmup_no_show_slash_bps=500`,
    `assigned_heartbeat_miss_slash_bps=250`,
    `assigned_heartbeat_miss_strike_threshold=3`, y
    `advert_contradiction_slash_bps=1000`.
  - El estado del tiempo de ejecución de HF asignado localmente ahora también alimenta esa misma ruta de evidencia:Errores de importación/calentamiento en tiempo de conciliación en una emisión de host local `Warming`
    `WarmupNoShow` y fallas de trabajador residente en el primario cálido local
    emitir informes `AssignedHeartbeatMiss` acelerados a través del modo normal
    cola de transacciones.
  - Conciliar ahora también los pre-inicios y sondeos de los trabajadores residentes de HF a nivel local.
    hosts asignados en caliente/en calentamiento, incluidas las réplicas, por lo que una réplica puede fallar
    cerrado en la ruta autorizada `AssignedHeartbeatMiss` antes de cualquier
    La solicitud pública `/infer` alguna vez llega al primario.
  - cuando esa sonda local tiene éxito, el tiempo de ejecución ahora también emite uno
    mutación autorizada `model-host-heartbeat` para el validador local cuando
    el host asignado sigue siendo `Warming` o el anuncio del host activo necesita un TTL
    actualizar, por lo que la preparación local exitosa promueve la misma autoridad
    Estado de ubicación/anuncio en el que se actualizarían los latidos manuales.
  - cuando el tiempo de ejecución emite un `WarmupNoShow` local o
    `AssignedHeartbeatMiss`, ahora también se pone en cola
    `ReconcileSoracloudModelHosts` a través del mismo carril de mutación interna, por lo que
    La conmutación por error/relleno autorizado comienza inmediatamente en lugar de esperar
    un barrido periódico posterior de vencimiento del host.
  - cuando el ingreso de HF generado públicamente falla incluso antes debido a que el
    La ubicación no tiene un servidor primario activo al que representar como proxy, Torii ahora solicita el tiempo de ejecución.
    manejar para poner en cola esa misma autoridadInstrucción `ReconcileSoracloudModelHosts` inmediatamente en lugar de esperar
    para una señal de vencimiento posterior o de falla del trabajador.
  - cuando el ingreso público de HF generado recibe una respuesta de éxito aproximada,
    Torii ahora verifica que el recibo de tiempo de ejecución incluido todavía demuestra la ejecución por
    las primarias cálidas y comprometidas para la colocación activa; faltante o no coincidente
    La atribución de ubicación ahora falla y sugiere que la misma autoridad
    Ruta `ReconcileSoracloudModelHosts` en lugar de devolver un
    respuesta no autorizada. Torii ahora también rechaza el éxito del proxy
    respuestas cuando los compromisos de recepción en tiempo de ejecución o la política de certificación no
    no coincide con la respuesta que está a punto de devolver, y ese mismo mal recibo
    La ruta también alimenta el informe primario remoto `AssignedHeartbeatMiss`.
    gancho.
  - Los fallos de ejecución de HF generados por proxy ahora solicitan lo mismo
    ruta autorizada `ReconcileSoracloudModelHosts` después de informar el
    falla de salud primaria remota, en lugar de esperar a que expire más tarde
    barrer.
  - Torii ahora vincula cada solicitud de proxy HF generada pendiente al
    par principal autorizado al que apuntaba. Una respuesta proxy del mal
    peer ahora se ignora en lugar de envenenar esa solicitud pendiente, por lo que solo
    el primario autorizado puede completar o rechazar la solicitud. un apoderado
    respuesta del par esperado con un esquema de respuesta de proxy no compatibleLa versión aún falla al cerrarse en lugar de ser aceptada solo porque su
    `request_id` coincidió con una solicitud pendiente. Si el compañero equivocado que respondió
    sigue siendo un host HF generado asignado para esa ubicación, el
    El tiempo de ejecución ahora informa que el host a través del existente
    Ruta de evidencia `WarmupNoShow` / `AssignedHeartbeatMiss` basada en su
    estado de asignación autorizado y también sugerencias autorizadas
    `ReconcileSoracloudModelHosts`, deriva de autoridad primaria/réplica obsoleta
    alimenta el bucle de control en lugar de ser ignorado únicamente en el ingreso.
  - La ejecución entrante del proxy Soracloud ahora también está restringida al objetivo previsto.
    Caso de consulta generado-HF `infer` en el primario activo comprometido. No HF
    rutas públicas de lectura local y solicitudes HF generadas entregadas a un nodo
    Esa ya no es la primaria cálida y autorizada, ahora falla cerrada.
    de ejecutar a través de la ruta del proxy P2P. Las primarias autorizadas también ahora
    vuelve a calcular el compromiso de solicitud de HF canónico generado antes de la ejecución,
    por lo que los sobres proxy falsificados o que no coinciden no se cierran.
  - cuando una réplica asignada o una primaria anterior obsoleta rechaza esa entrada
    ejecución de proxy HF generado porque ya no es la autoridad
    primario cálido, el tiempo de ejecución del lado del receptor ahora también sugiere
    `ReconcileSoracloudModelHosts` en lugar de depender únicamente del lado de la persona que llama
    vista de enrutamiento.- cuando ocurre el mismo error de autoridad de proxy HF generado entrante en
    el primario autorizado local en sí, el tiempo de ejecución ahora lo trata como un
    señal de salud del huésped de primera clase: autoinforme de primarias cálidas
    `AssignedHeartbeatMiss`, autoinforme de primarias de calentamiento `WarmupNoShow`,
    y ambas rutas reutilizan inmediatamente la misma autoridad
    Lazo de control `ReconcileSoracloudModelHosts`.
  - cuando ese mismo receptor no primario sigue siendo uno de los autorizados
    hosts asignados y puede resolver el primario cálido desde el estado de cadena comprometido,
    ahora vuelve a enviar la solicitud HF generada a ese primario en su lugar.
    de fracasar inmediatamente. Los validadores no asignados fallan al cerrarse en lugar de actuar
    como saltos de proxy HF intermediarios genéricos, y el nodo de entrada original aún
    valida el recibo de tiempo de ejecución devuelto contra la ubicación autorizada
    estado. Si ese salto de réplica asignada al primario falla después de la
    La solicitud se envía realmente, el tiempo de ejecución del lado del receptor informa la
    falla de salud primaria remota y sugerencias autorizadas
    `ReconcileSoracloudModelHosts`; si la réplica local asignada ni siquiera puede
    intente ese salto hacia adelante porque falta su propio transporte/tiempo de ejecución de proxy,
    la falla ahora se trata como una falla local del host asignado en lugar de
    culpando a la primaria.
  - reconciliar ahora también emite `AdvertContradiction` automáticamente cuandoLa identificación del par en tiempo de ejecución configurada del validador local no está de acuerdo con el
    ID de par autorizado `model-host-advertise` para ese validador.
  - Las mutaciones válidas de reanuncio de modelo-host ahora también sincronizan con autoridad
    metadatos de host asignado `peer_id` / `host_class` y recalcular la corriente
    tarifas de reserva de ubicación cuando cambia la clase anfitriona.
  - Las mutaciones contradictorias del modelo-huésped que se vuelven a anunciar ahora se emiten de forma inmediata.
    `AdvertContradiction` evidencia, aplique la barra/desalojo del validador existente
    ruta y actualizar las ubicaciones afectadas en lugar de simplemente fallar la validación.
  - El trabajo restante de alojamiento de HF es ahora:
    - señales de estado más amplias entre nodos/clústeres de tiempo de ejecución más allá de lo local
      Observaciones directas del trabajador/calentamiento del validador más el host asignado
      fallas de autoridad del lado del receptor cuando la salud interna del par remoto debería
      también alimente la ruta autorizada de reequilibrio/barra.
  - La ejecución local HF generada ahora mantiene un trabajador Python residente por fuente
    vivo bajo `irohad`, reutiliza el modelo cargado en `/infer` repetido
    llama y reinicia ese trabajador de manera determinista si la importación local
    cambios manifiestos o el proceso sale.
  - Estas rutas no son la ruta privada del modelo cargado. Los arrendamientos compartidos de HF se mantienen
    centrado en membresía de fuente/importación compartida en lugar de cifrado en cadena
    bytes de modelo privado.- Las aprobaciones de autonomía HF generadas ahora admiten secuencial determinista
    sobres de solicitud de varios pasos, pero más amplios, no lineales y con uso de herramientas
    La orquestación y la ejecución de gráficos de artefactos aún siguen siendo trabajos de seguimiento.
    más allá de los pasos encadenados `/infer`.

## Semántica de estado

`/v1/soracloud/status` y los puntos finales de estado de agente/capacitación/modelo relacionados ahora
reflejar el estado de tiempo de ejecución autorizado:

- revisiones de servicio admitidas por parte de un estado mundial comprometido;
- estado de hidratación/materialización en tiempo de ejecución desde el administrador de tiempo de ejecución integrado;
- recibos de ejecución de buzones reales y estado de falla;
- artefactos publicados en revistas/puntos de control;
- caché y estado del tiempo de ejecución en lugar de ajustes de estado de marcador de posición.

Si el material autorizado en tiempo de ejecución está obsoleto o no está disponible, las lecturas fallan y se cierran
en lugar de recurrir a los espejos estatales locales.

`/v1/soracloud/status` es el único endpoint de estado de Soracloud documentado en v1.
No existe una ruta `/v1/soracloud/registry` separada.

## Se eliminó el andamio local

Estos conceptos antiguos de simulación local ya no existen en la versión 1:

- Archivos de estado/registro local CLI u opciones de ruta de registro
- Torii: espejos del plano de control respaldados por archivos locales

## Ejemplo

```bash
iroha app soracloud deploy \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080 \
  --api-token <token-if-required> \
  --timeout-secs 10
```

## Notas- La validación local aún se ejecuta antes de que se firmen y envíen las solicitudes.
- Los puntos finales de mutación estándar de Soracloud ya no aceptan `authority` sin procesar /
  `private_key` Campos JSON para implementación, actualización, reversión, implementación, agente
  rutas de ciclo de vida, capacitación, modelo-host y modelo-peso; Torii autentica
  en su lugar, esas solicitudes provienen de los encabezados de firma HTTP canónicos.
- Los propietarios de Soracloud controlados por Multisig ahora usan `X-Iroha-Witness`; punto
  `soracloud.http_witness_file` en el JSON testigo exacto que desea que la CLI
  reproducir para la siguiente solicitud de mutación, y Torii no se cerrará si el
  La cuenta del sujeto testigo o el hash de solicitud canónica no coinciden.
- `hf-deploy` e `hf-lease-renew` ahora incluyen auxiliar firmado por el cliente
  procedencia de los artefactos de servicio/apartamento de HF generados deterministas,
  por lo que Torii ya no necesita claves privadas de la persona que llama para admitir ese seguimiento
  objetos.
- `agent-autonomy-run` e `model/run-private` ahora usan un borrador y luego finalizan
  flujo: la primera mutación firmada registra la aprobación/inicio autorizado,
  y una segunda solicitud de finalización firmada ejecuta la ruta de ejecución y devuelve
  cualquier instrucción de seguimiento autorizada como borrador determinista
  transacciones.
- `model/decrypt-output` ahora devuelve la inferencia privada autorizada
  punto de control como un borrador de transacción determinista, firmado sólo por el exteriortransacción en lugar de mediante una clave privada incorporada Torii.
- El CRUD adjunto de ZK ahora elimina el arrendamiento de la cuenta Iroha firmada y aún
  trata los tokens API como una puerta de acceso adicional cuando están habilitados.
- El ingreso de lectura local pública de Soracloud ahora aplica una tasa explícita por IP y
  limita la concurrencia y vuelve a verificar la visibilidad de la ruta pública antes
  ejecución representada.
- La aplicación de la capacidad de tiempo de ejecución privado ocurre dentro de la ABI del host de Soracloud,
  no dentro de CLI o andamiaje local Torii.
- `ram_lfe` sigue siendo un subsistema de funciones ocultas independiente. Privado subido por el usuario
  La ejecución del transformador debe reutilizar la gobernanza de descifrado/FHE de Soracloud y
  registros de modelo, no la ruta de solicitud `ram_lfe`.
- La salud, la hidratación y la ejecución en tiempo de ejecución provienen de
  Configuración `[soracloud_runtime]` y estado comprometido, no entorno
  alterna.