---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-elástico-carril
título: Provisión de carril elástico (NX-7)
sidebar_label: provisión de carril elástico
Descripción: Flujo de trabajo de bootstrap para creer los manifiestos del carril Nexus, las entradas del catálogo y las etapas previas al lanzamiento.
---

:::nota Fuente canónica
Esta página informa `docs/source/nexus_elastic_lane.md`. Gardez les deux copys alignees jusqu'a ce que la vague de traduction llegan sur le portail.
:::

# Kit de provisión de carril elástico (NX-7)

> **Elemento de la hoja de ruta:** NX-7: herramientas de provisión de carril elástico  
> **Estado:** herramientas completas: genera manifiestos, fragmentos de catálogo, cargas útiles Norito, pruebas de humo,
> y el ayudante de paquete de prueba de carga ensamblar mantenimiento de puerta de latencia por ranura + des manifiestos de preuve afin que les ejecuta de validadores de carga
> puissent etre publies sans scripting sur mesure.

Esta guía acompaña a los operadores a través del nuevo asistente `scripts/nexus_lane_bootstrap.sh` que automatiza la generación de manifiestos de carril, fragmentos de catálogo de carril/espacio de datos y las preferencias de implementación. El objetivo es facilitar la creación de nuevos carriles Nexus (públicos o privados) sin editar los archivos principales ni reenviar la geometría principal del catálogo.

## 1. Requisitos previos1. Aprobación de la gobernanza para el alias de carril, el espacio de datos, el conjunto de validadores, la tolerancia a los paneles (`f`) y la política de asentamiento.
2. Una lista final de validadores (ID de cuenta) y una lista de espacios de nombres protegidos.
3. Acceda al depósito de configuración de nuevos elementos para poder agregar fragmentos genéricos.
4. Chemins pour le registro de manifests de lane (voir `nexus.registry.manifest_directory` et `cache_directory`).
5. Telemetría de contactos/identificadores PagerDuty para la línea para que las alertas se conecten des que la línea está en línea.

## 2. Generar artefactos de carril

Lancez le helper depuis la racine du depot :

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator ih58... \
  --validator ih58... \
  --validator ih58... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Banderas cle :- `--lane-id` corresponde al índice de la nueva entrada en `nexus.lane_catalog`.
- `--dataspace-alias` e `--dataspace-id/hash` controlan la entrada del catálogo del espacio de datos (por defecto, el ID del carril cuando se omiten).
- `--validator` puede repetirse o después de `--validators-file`.
- `--route-instruction` / `--route-account` emettent des regles de routage pretes a coller.
- `--metadata key=value` (o `--telemetry-contact/channel/runbook`) captura los contactos del runbook para que los paneles muestren los buenos propietarios.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` agregan el gancho runtime-upgrade al manifiesto cuando la línea requiere de controles operativos vigentes.
- `--encode-space-directory` invoca automáticamente `cargo xtask space-directory encode`. Combine con `--space-directory-out` cuando desee que el archivo `.to` codifique todos los ailleurs que le chemin por defecto.

El script produce tres artefactos en `--output-dir` (por defecto el repertorio actual), además de cuatro opciones adicionales cuando la codificación está activa:1. `<slug>.manifest.json`: manifiesto del carril que contiene el quórum de validadores, los espacios de nombres protegidos y las opciones de metadatos del gancho runtime-upgrade.
2. `<slug>.catalog.toml`: un fragmento TOML con `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` y todas las reglas de ruta solicitadas. Asegúrese de que `fault_tolerance` está definido en el espacio de datos principal para dimensionar el comité lane-relay (`3f+1`).
3. `<slug>.summary.json` - resumen de auditoría de la geometría (slug, segmentos, metadones) más las etapas de requisitos de implementación y el comando exacto `cargo xtask space-directory encode` (bajo `space_directory_encode.command`). Ingrese este JSON en el boleto de embarque como antes.
4. `<slug>.manifest.to` - emite cuando `--encode-space-directory` está activo; pret pour le flux `iroha app space-directory manifest publish` de Torii.

Utilice `--dry-run` para previsualizar archivos JSON/snippets sin escribir archivos, y `--force` para borrar los artefactos existentes.

## 3. Aplicar cambios1. Copie el manifiesto JSON en la configuración `nexus.registry.manifest_directory` (y en el directorio de caché si el registro refleja los paquetes distantes). Confirme el archivo si los manifiestos son versiones en su repositorio de configuración.
2. Agregue el fragmento de catálogo a `config/config.toml` (o a `config.d/*.toml` apropiado). Asegúrese de que `nexus.lane_count` soit au moins `lane_id + 1`, y mettez a jour toute regle `nexus.routing_policy.rules` qui doit pointer vers la nouvelle lane.
3. Codifique (si tiene salteado `--encode-space-directory`) y publique el manifiesto en el Directorio espacial mediante el comando capturado en el resumen (`space_directory_encode.command`). Esto produce la carga útil `.manifest.to` atendida por Torii y registra la presión para las auditorías; soumettez a través de `iroha app space-directory manifest publish`.
4. Lancez `irohad --sora --config path/to/config.toml --trace-config` y archiva la salida de seguimiento en el ticket de lanzamiento. Esto demuestra que la nueva geometría corresponde a los segmentos Kura du slug genere.
5. Redemarrez les validadores asignas a la lane une fois les changements manifest/catalogue implementados. Conserve el resumen JSON en el ticket para futuras auditorías.

## 4. Construya un paquete de distribución del registroEmpaquetez le manifest genere et la overlay para que los operadores puedan distribuir las donnees de gouvernance des lanes sans editer les configs sur cada hote. El asistente de agrupación copia los manifiestos en el diseño canónico, produce una opción de superposición del catálogo de gobierno para `nexus.registry.cache_directory` y puede editar un archivo comprimido para transferir sin conexión:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Salidas :

1. `manifests/<slug>.manifest.json`: copie los archivos en la configuración `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - depositado en `nexus.registry.cache_directory`. Cada vez que ingrese `--module`, obtendrá una definición de módulo ramificable, lo que permitirá intercambiar módulos de gobierno (NX-2) y agregará un día de superposición de caché cuando edite `config.toml`.
3. `summary.json`: incluye hashes, metadones de superposición e instrucciones del operador.
4. Opcional `registry_bundle.tar.*`: disponible para SCP, S3 o rastreadores de artefactos.

Sincronice todo el repertorio (o el archivo) con cada validador, extraiga los hotes air-gapped y copie los manifiestos + superposición de caché en los caminos de registro antes del canje Torii.

## 5. Pruebas de humo de validadoresDespués de la redistribución de Torii, lanza el nuevo ayudante de humo para verificar que la relación de carril `manifest_ready=true`, que las métricas exponen el nombre de los carriles, y que el indicador sellado está vide. Los carriles que requieren manifiestos deben exponer un `manifest_path` no vide; El ayudante se hace eco de la manera en que el camino se realiza a fin de que cada implementación del NX-7 incluya la preuve du manifest signe:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Ajoutez `--insecure` cuando pruebe los entornos autofirmados. El script se ordena con un código distinto de cero si la línea manque está sellada o si las métricas/telemetría derivan de los valores asistentes. Utilice los mandos `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` y `--max-headroom-events` para mantener la telemetría por carril (alta velocidad de bloque/finalita/acumulación/espacio libre) en sus envolventes operativas y acoplarlos con `--max-slot-p95` / `--max-slot-p99` (más `--min-slot-samples`) para imponer los objetivos de duración de la ranura NX-18 sin dejar el ayudante.

Para las validaciones con espacio de aire (o CI), puede recuperar una respuesta Torii capturada en lugar de interrogar un punto final en vivo:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```Los dispositivos registrados en `fixtures/nexus/lanes/` reflejan los artefactos producidos por el asistente de arranque para que los nuevos manifiestos puedan entre líneas sin secuencias de comandos a medida. El CI ejecuta el flujo de memes a través de `ci/check_nexus_lane_smoke.sh` e `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) para comprobar que el asistente de humo NX-7 se ajusta al formato de carga útil pública y para asegurar que los resúmenes/superposiciones del paquete sean reproducibles.