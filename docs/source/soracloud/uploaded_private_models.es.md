<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/source/soracloud/uploaded_private_models.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 97d6a421ce93a0e85be6cc99e828f965c9d8617d0ee27a772a2c9f2f646e77b7
source_last_modified: "2026-03-24T18:59:46.535846+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Modelos cargados por el usuario de Soracloud y tiempo de ejecución privado

Esta nota define cómo el flujo del modelo subido por el usuario de So Ra debe aterrizar en el
plano modelo Soracloud existente sin inventar un tiempo de ejecución paralelo.

## Objetivo de diseño

Agregue un sistema de modelo cargado exclusivo de Soracloud que permita a los clientes:

- cargar sus propios repositorios de modelos;
- vincular una versión del modelo fijado al apartamento de un agente o al equipo de arena cerrada;
- ejecutar inferencia privada con entradas cifradas y modelo/estado cifrados; y
- recibir compromisos públicos, recibos, precios y pistas de auditoría.

Esta no es una característica `ram_lfe`. `ram_lfe` sigue siendo la función oculta genérica
subsistema documentado en `../universal_accounts_guide.md`. modelo subido
la inferencia privada debería ampliar el registro de modelos existente de Soracloud,
Superficies de artefactos, capacidad de apartamento, FHE y política de descifrado.

## Superficies existentes de Soracloud para reutilizar

La pila actual de Soracloud ya tiene los objetos base correctos:- `SoraModelRegistryV1`
  - nombre autorizado del modelo por servicio y estado de la versión promocionada.
- `SoraModelWeightVersionRecordV1`
  - linaje de versiones, promoción, reversión, procedencia y reproducibilidad
    hash.
- `SoraModelArtifactRecordV1`
  - metadatos de artefactos deterministas ya vinculados al canal de modelo/peso.
- `SoraCapabilityPolicyV1.allow_model_inference`
  - indicador de capacidad de apartamento/servicio que debería ser obligatorio para
    Apartamentos vinculados a modelos cargados.
- `SecretEnvelopeV1` y `CiphertextStateRecordV1`
  - portadores deterministas de bytes cifrados y de estado de texto cifrado.
- `FheParamSetV1`, `FheExecutionPolicyV1`, `FheGovernanceBundleV1`,
  `DecryptionAuthorityPolicyV1` y `DecryptionRequestV1`
  - la capa de política/gobernanza para la ejecución cifrada y la salida controlada
    liberación.
- rutas actuales del modelo Torii:
  - `/v1/soracloud/model/weight/{register,promote,rollback,status}`
  - `/v1/soracloud/model/artifact/{register,status}`
- rutas actuales de arrendamiento compartido HF Torii:
  - `/v1/soracloud/hf/{deploy,status,lease/leave,lease/renew}`

La ruta del modelo cargado debería extender esas superficies. No debe sobrecargar
HF comparte arrendamientos y no debe reutilizar `ram_lfe` como tiempo de ejecución de servicio de modelos.

## Contrato de carga canónica

El contrato de modelo subido privado de Soracloud debería admitir sólo canónicos
Repositorios de modelos estilo Hugging Face:- archivos base requeridos:
  - `config.json`
  - archivos tokenizadores
  - archivos de procesador/preprocesador cuando la familia los requiera
  - `*.safetensors`
- grupos familiares admitidos en este hito:
  - LM causales solo decodificador con semántica RoPE/RMSNorm/SwiGLU/GQA
  - Modelos de texto+imagen estilo LLaVA
  - Modelos de texto+imagen estilo Qwen2-VL
- rechazado en este hito:
  - GGUF como contrato de ejecución privada cargado
  -ONNX
  - faltan activos de tokenizador/procesador
  - arquitecturas/formas no compatibles
  - paquetes multimodales de audio/vídeo

Por qué este contrato:

- coincide con el diseño del modelo del lado del servidor ya dominante utilizado por los existentes
  ecosistema en torno a tensores de seguridad y repositorios de Hugging Face;
- permite que el plano modelo comparta una ruta de normalización determinista a través
  Entonces Ra, Torii y compilación en tiempo de ejecución; y
- evita combinar formatos de importación en tiempo de ejecución local como GGUF con el
  Contrato de ejecución privada de Soracloud.

Los arrendamientos compartidos de HF siguen siendo útiles para los flujos de trabajo de importación de fuentes compartidas/públicas, pero
la ruta privada del modelo cargado almacena bytes compilados cifrados en la cadena en lugar de
que alquilar bytes de modelo de un grupo de origen compartido.

## Diseño de modelo-plano en capas

### 1. Procedencia y capa registral

Amplíe el diseño de registro del modelo actual en lugar de reemplazarlo:- añadir `SoraModelProvenanceKindV1::UserUpload`
  - los tipos actuales (`TrainingJob`, `HfImport`) no son suficientes para distinguir un
    modelo cargado y normalizado directamente por un cliente como So Ra.
- Mantenga `SoraModelRegistryV1` como índice de la versión promocionada.
- mantener `SoraModelWeightVersionRecordV1` como registro de linaje/promoción/reversión.
- ampliar `SoraModelArtifactRecordV1` con tiempo de ejecución privado cargado opcional
  referencias:
  - `private_bundle_root`
  - `chunk_manifest_root`
  - `compile_profile_hash`
  - `privacy_mode`

El registro de artefactos sigue siendo el ancla determinista que vincula la procedencia,
metadatos de reproducibilidad y agrupar identidades. El récord de peso
sigue siendo el objeto de linaje de la versión promocionada.

### 2. Capa de almacenamiento de paquetes/fragmentos

Agregue registros Soracloud de primera clase para material de modelo cargado cifrado:

- `SoraUploadedModelBundleV1`
  - `model_id`
  - `weight_version`
  - `family`
  - `modalities`
  - `runtime_format`
  - `bundle_root`
  - `chunk_count`
  - `plaintext_bytes`
  - `ciphertext_bytes`
  - `compile_profile_hash`
  - `pricing_policy`
  - `decryption_policy_ref`
- `SoraUploadedModelChunkV1`
  - `model_id`
  - `bundle_root`
  - `ordinal`
  - `offset_bytes`
  - `plaintext_len`
  - `ciphertext_len`
  - `ciphertext_hash`
  - carga útil cifrada (`SecretEnvelopeV1`)

Reglas deterministas:- los bytes de texto plano se fragmentan en fragmentos fijos de 4 MiB antes del cifrado;
- el orden de los fragmentos es estricto y se rige por ordinales;
- los resúmenes de fragmentos/raíces son estables durante la reproducción; y
- cada fragmento cifrado debe permanecer por debajo del `SecretEnvelopeV1` actual
  techo de texto cifrado de `33,554,432` bytes.

Este hito almacena bytes cifrados literales en estado de cadena a través de fragmentos.
registros. No descarga bytes privados del modelo cargado a SoraFS.

Debido a que la cadena es pública, la confidencialidad de la carga debe provenir de una persona real.
Clave de destinatario mantenida por Soracloud, no de claves deterministas derivadas de claves públicas
metadatos. El escritorio debería buscar un destinatario de cifrado de carga anunciado,
cifrar fragmentos bajo una clave de paquete aleatoria por carga y publicar solo el
metadatos del destinatario más un sobre con clave de paquete envuelto junto al texto cifrado.

### 3. Capa de compilación/tiempo de ejecución

Agregue una capa de tiempo de ejecución/compilador de transformador privado dedicado en Soracloud:- estandarizar la inferencia compilada determinista de baja precisión respaldada por BFV para
  ahora, porque CKKS existe en la discusión del esquema pero no en el implementado
  tiempo de ejecución local;
- compilar modelos admitidos en un IR privado determinista de Soracloud que cubra
  incrustaciones, capas lineales/proyector, matmuls de atención, RoPE, RMSNorm /
  Aproximaciones LayerNorm, bloques MLP, proyección de parches de visión y
  rutas de proyector de imagen a decodificador;
- utilizar inferencia determinista de punto fijo con:
  - pesos int8
  - activaciones int16
  - acumulación int32
  - aproximaciones polinomiales aprobadas para no linealidades

Este compilador/tiempo de ejecución es independiente de `ram_lfe`. Puede reutilizar primitivas BFV
y objetos de gobierno de Soracloud FHE, pero no es el mismo motor de ejecución
o familia de rutas.

### 4. Capa de inferencia/sesión

Agregue registros de sesión y puntos de control para carreras privadas:- `SoraPrivateCompileProfileV1`
  - `family`
  - `quantization`
  - `opset_version`
  - `max_context`
  -`max_images`
  - `vision_patch_policy`
  -`fhe_param_set`
  - `execution_policy`
- `SoraPrivateInferenceSessionV1`
  - `session_id`
  - `apartment`
  - `model_id`
  - `weight_version`
  - `bundle_root`
  - `input_commitments`
  -`token_budget`
  - `image_budget`
  - `status`
  - `receipt_root`
  - `xor_cost_nanos`
- `SoraPrivateInferenceCheckpointV1`
  - `session_id`
  - `step`
  - `ciphertext_state_root`
  - `receipt_hash`
  - `decrypt_request_id`
  - `released_token`
  - `compute_units`
  - `updated_at_ms`

Ejecución privada significa:

- entradas de mensajes/imágenes cifradas;
- pesos y activaciones de modelos cifrados;
- publicación explícita de la política de descifrado para la salida;
- recibos públicos de ejecución y contabilidad de costes.

No significa ejecución oculta sin compromisos ni auditabilidad.

## Responsabilidades del cliente

Entonces Ra u otro cliente deben realizar un preprocesamiento local determinista antes
la carga llega a Soracloud:

- aplicación de tokenizador;
- preprocesamiento de imágenes en tensores de parche para familias de texto+imagen admitidas;
- normalización determinista del paquete;
- Cifrado del lado del cliente de identificadores de tokens y tensores de parches de imágenes.

Torii debería recibir entradas cifradas más compromisos públicos, no un mensaje sin formato
texto o imágenes sin formato, para la ruta privada.

## Plan API e ISIMantenga las rutas de registro del modelo existente como capa de registro canónica y agregue
nuevas rutas de carga/tiempo de ejecución en la parte superior:

- `POST /v1/soracloud/model/upload/init`
- `POST /v1/soracloud/model/upload/chunk`
- `POST /v1/soracloud/model/upload/finalize`
- `GET /v1/soracloud/model/upload/encryption-recipient`
- `POST /v1/soracloud/model/compile`
- `POST /v1/soracloud/model/allow`
-`POST /v1/soracloud/model/run-private`
- `GET /v1/soracloud/model/run-status`
- `POST /v1/soracloud/model/decrypt-output`

Respaldéelos con Soracloud ISI coincidentes:

- registro de paquete
- fragmento agregar/finalizar
- compilar admisión
- inicio de carrera privada
- registro de punto de control
- liberación de salida

El flujo debe ser:

1. upload/init establece la sesión determinista del paquete y la raíz esperada;
2. upload/chunk agrega fragmentos cifrados en orden ordinal;
3. cargar/finalizar sella la raíz del paquete y el manifiesto;
4. compilar produce un perfil de compilación privado determinista vinculado al
   paquete admitido;
5. Los registros de registro de modelo/artefacto + modelo/peso hacen referencia al paquete cargado
   en lugar de un simple trabajo de formación;
6. permitir-modelo vincula el modelo cargado a un apartamento que ya admite
   `allow_model_inference`;
7. run-private registra una sesión y emite puntos de control/recibos;
8. Las versiones de descifrado de salida regían el material de salida.

## Política de precios y plano de control

Amplíe el comportamiento actual del plano de control/carga de Soracloud:- Se requiere `allow_model_inference` para apartamentos que ejecutan modelos cargados;
- almacenamiento de precios, compilación, pasos de tiempo de ejecución y liberación de descifrado en XOR;
- mantener desactivada la propagación narrativa para las ejecuciones del modelo cargado en este hito;
- Mantenga los modelos cargados dentro del ámbito cerrado de So Ra y los flujos controlados por la exportación.

## Autorización y semántica vinculante

Cargar, compilar y ejecutar son capacidades separadas y deben permanecer separadas en
el modelo de avión.

- cargar un paquete de modelos no debe autorizar implícitamente a un apartamento a ejecutarlo;
- el éxito de la compilación no debe promover implícitamente una versión del modelo a la actual;
- la vinculación del apartamento debe ser explícita mediante una mutación de estilo `allow-model`
  que registra:
  - apartamento,
  - identificación del modelo,
  - versión de peso,
  - raíz del paquete,
  - modo de privacidad,
  - secuencia firmante/auditoría;
- los apartamentos vinculados a los modelos cargados ya deben admitir
  `allow_model_inference`;
- las rutas de mutación deberían seguir requiriendo la misma solicitud firmada de Soracloud
  disciplina utilizada por el modelo/artefacto/rutas de entrenamiento existentes y debe ser
  custodiado por `CanManageSoracloud` o una autoridad delegada igualmente explícita
  modelo.

Esto impide "Lo subí, por lo tanto, cada apartamento privado puede ejecutarlo".
deriva y mantiene explícita la política de ejecución de apartamentos.

## Estado y modelo de auditoría

Los nuevos registros necesitan superficies de lectura y auditoría autorizadas, no sólo mutaciones
rutas.

Adiciones recomendadas:- estado de carga
  - consulta por `service_name + model_name + weight_version` o por
    `model_id + bundle_root`;
- estado de compilación
  - consulta por `model_id + bundle_root + compile_profile_hash`;
- estado de ejecución privada
  - consulta por `session_id`, con contexto apartamento/modelo/versión incluido en el
    respuesta;
- estado de salida de descifrado
  - consulta por `decrypt_request_id`.

La auditoría debe permanecer en la secuencia global existente de Soracloud en lugar de
creando un segundo contador por función. Agregue eventos de auditoría de primera clase para:

- cargar inicio/finalizar
- trozo agregar / sellar
- compilación admitida / compilación rechazada
- modelo de apartamento permitir / revocar
- inicio de ejecución privada/punto de control/finalización/fallo
- liberación / denegación de salida

Eso mantiene visible la actividad del modelo cargado en la misma reproducción autorizada y
historia de operaciones como el servicio actual, capacitación, modelo-peso, modelo-artefacto,
Flujos de auditoría de apartamentos y arrendamiento compartido de HF.

## Cuotas de admisión y límites de crecimiento estatal

Los bytes literales del modelo cifrado en la cadena son viables solo si la admisión está limitada
agresivamente.

La implementación debería definir límites deterministas para al menos:

- bytes máximos de texto plano por paquete cargado;
- bytes cifrados máximos por paquete;
- número máximo de fragmentos por paquete;
- máximo de sesiones de carga simultáneas en vuelo por autoridad/servicio;
- máximo de trabajos de compilación por ventana de servicio/apartamento;
- recuento máximo de puntos de control retenidos por sesión privada;
- solicitudes máximas de liberación de salida por sesión.Torii y el núcleo deben rechazar las cargas que excedan los límites declarados antes
Se produce una amplificación del estado. Los límites deben estar impulsados por la configuración donde
apropiado, pero los resultados de la validación deben seguir siendo deterministas entre pares.

## Reproducción y determinismo del compilador

La ruta del compilador/tiempo de ejecución privado tiene una carga de determinismo mayor que una ruta normal.
despliegue del servicio.

Invariantes requeridos:

- la detección y normalización familiar deben producir un paquete canónico estable
  antes de que se emita cualquier hash de compilación;
- los hashes del perfil de compilación deben vincularse:
  - raíz del paquete normalizada,
  - familia,
  - receta de cuantificación,
  - versión opset,
  - conjunto de parámetros FHE,
  - política de ejecución;
- el tiempo de ejecución debe evitar núcleos no deterministas, deriva de punto flotante y
  reducciones específicas de hardware que podrían cambiar las salidas o recibos en
  compañeros.

Antes de escalar el conjunto familiar admitido, consiga un pequeño elemento determinista para
cada clase familiar y salidas de compilación de bloqueo más recibos de tiempo de ejecución con golden
pruebas.

## Brechas de diseño restantes antes del código

Las cuestiones de implementación más importantes sin resolver ahora se reducen a temas concretos.
decisiones de backend:- formas DTO exactas de carga/fragmento/solicitud y esquemas Norito;
- claves de indexación de estado mundial para búsquedas de paquetes/fragmentos/sesiones/puntos de control;
- colocación de configuración de cuota/predeterminada en `iroha_config`;
- si el estado del modelo/artefacto debería orientarse a la versión en lugar de
  orientado a la formación laboral cuando `UserUpload` está presente;
- el comportamiento preciso de revocación cuando un apartamento pierde
  `allow_model_inference` o una versión de modelo fijada se revierte.

Esos son los próximos elementos del puente de diseño a código. La ubicación arquitectónica de
la característica ahora debería ser estable.

## Matriz de prueba- validación de carga:
  - aceptar repositorios de tensores de seguridad HF canónicos
  - rechazar GGUF, ONNX, faltan activos de tokenizador/procesador, no compatibles
    arquitecturas y paquetes multimodales de audio/vídeo.
- fragmentación:
  - raíces de haces deterministas
  - ordenamiento de fragmentos estable
  - reconstrucción exacta
  - aplicación del techo envolvente
- coherencia del registro:
  - Corrección de promoción de paquete/fragmento/artefacto/peso en repetición
- compilador:
  - un dispositivo pequeño para decodificador solamente, estilo LLaVA y estilo Qwen2-VL
  - rechazo de operaciones y formas no compatibles
- tiempo de ejecución privado:
  - prueba de humo de extremo a extremo encriptada con dispositivos pequeños con recibos estables y
    liberación de salida de umbral
- precios:
  - Cargos XOR por carga, compilación, pasos de ejecución y descifrado
- Integración de So Ra:
  - cargar, compilar, publicar, vincularse al equipo, ejecutar arena cerrada, inspeccionar recibos,
    guardar proyecto, volver a abrir, volver a ejecutar de forma determinista
- seguridad:
  - sin bypass de puerta de exportación
  - sin autopropagación narrativa
  - la vinculación del apartamento falla sin `allow_model_inference`

## Porciones de implementación1. Agregue los campos del modelo de datos que faltan y los nuevos tipos de registros.
2. Agregue los nuevos tipos de solicitud/respuesta y controladores de ruta Torii.
3. Agregue Soracloud ISI coincidentes y almacenamiento de estado mundial.
4. Agregue validación determinista de paquete/fragmento y byte cifrado en cadena
   almacenamiento.
5. Agregue una pequeña ruta de tiempo de ejecución/dispositivo de transformador privado respaldado por BFV.
6. Amplíe los comandos del modelo CLI para cubrir los flujos de carga/compilación/ejecución privada.
7. Integración de Land So Ra una vez que la ruta de backend sea autorizada.