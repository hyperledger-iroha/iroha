---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Proceso de liberación
Resumen: Ejecute la puerta de liberación de CLI/SDK, aplique una política de versión compartida y publique notas de liberación canónicas.
---

# Proceso de liberación

Los binarios de SoraFS (`sorafs_cli`, `sorafs_fetch`, ayudantes) y las cajas de SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) sao entregues juntos. oh tubería
de liberación mantem o CLI y como bibliotecas alinhados, garantía de cobertura de lint/test e
captura de artefatos para consumidores posteriores. Ejecutar una lista de verificación abaixo para cada
etiqueta de candidato.

## 0. Confirmar aprobación de revisión de seguranca

Antes de ejecutar la puerta técnica de liberación, capture los artefactos más recientes de
revisao de seguranca:

- Baixe o memo de revisao de seguranca SF-6 mais Recente ([reports/sf6-security-review](./reports/sf6-security-review.md))
  Registre su hash SHA256 sin boleto de liberación.
- Anexo o enlace del ticket de reparación (por ejemplo, `governance/tickets/SF6-SR-2026.md`) y nota
  os aprobadores de Ingeniería de Seguridad y del Grupo de Trabajo de Herramientas.
- Verifique que a checklist de remediacao no memo esta fechada; ítems pendientes bloquean o liberan.
- Preparar o cargar dos registros del arnés de paridade (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  junto com o paquete de manifiesto.
- Confirme que o comando de assinatura que voce vai ejecutar incluye `--identity-token-provider` e
  um `--identity-token-audience=<aud>` explícito para capturar o esconder a Fulcio na evidencia de liberación.Incluye estos artefactos en la notificación al gobierno y la publicación o liberación.

## 1. Ejecutar o gate de release/testes

O helper `ci/check_sorafs_cli_release.sh` roda formatacao, Clippy e testes nos crates
CLI y SDK con un directorio de destino local para el espacio de trabajo (`.target`) para evitar conflictos
de permiso para ejecutar dentro de contenedores CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

O script faz como siguientes verificacoes:

- `cargo fmt --all -- --check` (espacio de trabajo)
- `cargo clippy --locked --all-targets` para `sorafs_car` (con una característica `cli`),
  `sorafs_manifest` y `sorafs_chunker`
- `cargo test --locked --all-targets` para cajas mesmos esses

Se algum passo falhar, corrija a regressao antes de taguear. Construye el dispositivo de lanzamiento.
ser continuo com principal; nao faca cherry-pick de correcoes em sucursales de liberación. oh
gate tambem verifica se os flags de assinatura sem chave (`--identity-token-issuer`,
`--identity-token-audience`) foram fornecidos quando aplicavel; argumentos faltando
fazem a execucao falhar.

## 2. Aplicar una política de versionamento

Todas las cajas CLI/SDK de SoraFS usan SemVer:- `MAJOR`: Introducción a la primera versión 1.0. Antes de 1.0 o aumento menor
  `0.y` **indica mudancas quebradoras** en la superficie del CLI o en los esquemas Norito.
- `MINOR`: Trabajo de características compatibles para tras (nuevos comandos/flags, novos
  campos Norito protegidos por política opcional, médicos de telemetría).
- `PATCH`: Correcoes de bugs, releases somente de documentacao e actualizacoes de
  dependencias que nao mudam o comportamento observavel.

Mantenha siempre `sorafs_car`, `sorafs_manifest` e `sorafs_chunker` en mesma versao
para que los consumidores de SDK downstream puedan depender de una única cadena de
versao alinhada. Ao actualizar versos:

1. Realice los campos `version =` en cada `Cargo.toml`.
2. Regenere o `Cargo.lock` vía `cargo update -p <crate>@<new-version>` (o espacio de trabajo
   exige versos explícitos).
3. Rode novamente o gate de liberación para garantizar que nao restem artefatos desatualizados.

## 3. Preparar notas de lanzamiento

Cada lanzamiento debe publicar un registro de cambios en Markdown que elimina cambios de CLI, SDK y
impacto de gobernanza. Utilice la plantilla en `docs/examples/sorafs_release_notes.md`
(copia-o para su directorio de artefatos de liberación e preencha as secoes com detalhes
concretos).

Conteudo mínimo:- **Aspectos destacados**: manchetes de características para consumidores de CLI y SDK.
- **Compatibilidade**: mudancas quebradoras, actualizaciones de política, requisitos mínimos
  de puerta de enlace/nodo.
- **Pasos de actualización**: comandos TL;DR para actualizar dependencias de carga y refazer
  accesorios determinísticos.
- **Verificacao**: hashes de Saida o sobres y una revisión exata de
  `ci/check_sorafs_cli_release.sh` ejecutada.

Anexo como notas de lanzamiento preenchidas ao tag (por ejemplo, cuerpo de lanzamiento de GitHub) e
guarde junto com os artefatos gerados de forma determinística.

## 4. Ejecutar ganchos de liberación

Rode `scripts/release_sorafs_cli.sh` para generar el paquete de assinatura y el resumen de
verificacao que acompanham cada liberación. El contenedor compila la CLI cuando es necesario,
chama `sorafs_cli manifest sign` e inmediatamente reejecuta `manifest verify-signature`
para que falhas aparecam antes de etiquetar. Ejemplo:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

Dicas:- Registro de entradas de lanzamiento (carga útil, planes, resúmenes, hash esperado del token)
  No hay repositorio ni configuración de implementación para mantener la reproducción del script. oh
  paquete de accesorios en `fixtures/sorafs_manifest/ci_sample/` muestra el diseño canónico.
- Base de automóvil CI en `.github/workflows/sorafs-cli-release.yml`; ele roda o
  gate de release, chama o script acima e arquiva bundles/assinaturas como artefatos
  hacer flujo de trabajo. Mantenha a mesma ordem de comandos (gate de release -> assinatura ->
  verificacao) en otros sistemas CI para que los registros de auditoría batam con los hashes
  gerados.
- Mantenha `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json` y
  `manifest.verify.summary.json` juntos; eles formam o pacote referenciado na
  notificación de gobierno.
- Cuando se liberan accesorios canónicos, se copia el manifiesto actualizado, o
  resúmenes de plan y sistema operativo fragmentados para `fixtures/sorafs_manifest/ci_sample/` (e atualizar
  `docs/examples/sorafs_ci_sample/manifest.template.json`) antes de taguear. Operadores
  aguas abajo dependen de dos accesorios comprometidos para reproducir o paquete de lanzamiento.
- Captura o registro de ejecución de verificación de canales acotados de
  `sorafs_cli proof stream` y anexo ao pacote do release para demostrar que como
  salvaguardas de prueba streaming continuam activas.
- Registre o `--identity-token-audience` exato usado durante una assinatura nas
  notas de liberación; agobernanca cruza o audiencia com a politica de Fulcio antes de
  aprobar a publicacao.Utilice `scripts/sorafs_gateway_self_cert.sh` cuando el lanzamiento también incluya un lanzamiento
de puerta de enlace. Aponte para o mesmo paquete de manifiesto para probar que una atestación
corresponde ao artefato candidato:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Etiqueta y publicación

Después de que los cheques pasen y los ganchos forem concluidos:1. Rode `sorafs_cli --version` e `sorafs_fetch --version` para confirmar que os binarios
   Reportam a nova versao.
2. Prepare una configuración de lanzamiento en un `sorafs_release.toml` versionado (preferido)
   ou otro archivo de configuración rastreado en su repositorio de implementación. Evite el dependiente
   de variaveis de ambiente ad-hoc; pase los caminos para el CLI con `--config` (o
   equivalente) para que las entradas del sistema operativo se liberen y se reproduzcan explícitamente.
3. Grita una etiqueta asesinada (preferida) o anotada:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Faca upload dos artefatos (paquetes CAR, manifiestos, resumos de pruebas, notas de liberación,
   salidas de atestación) para el registro del proyecto siguiendo una lista de verificación de gobierno
   no [guía de implementación](./developer-deployment.md). Se lanzarán accesorios de gerou novas,
   envie-as para o repo de accesorios compartilhado ou object store para que a automacao
   de auditoria podrá comparar el paquete publicado con el control de código.
5. Notifique el canal de gobierno con enlaces para la etiqueta assinada, notas de liberación, hashes
   do bundle/assinaturas do manifest, resumos arquivados de `manifest.sign/verify` e
   sobres quaisquer de atestación. Incluye una URL para el trabajo de CI (o archivo de registros)
   que rodou `ci/check_sorafs_cli_release.sh` e `scripts/release_sorafs_cli.sh`. actualizar
   o billete de gobierno para que os auditores possam rastreen aprovacoes ate os artefatos;
   Cuando el trabajo `.github/workflows/sorafs-cli-release.yml` publica notificaciones, enlaceLos hashes registrados en vez de colar resumos ad-hoc.

## 6. Pos-liberación

- Garanta que a documentacao apontando para a nova versao (inicios rápidos, plantillas de CI)
  esteja atualizada ou confirme que nenhuma mudanca e necessaria.
- Registre entradas sin hoja de ruta para el trabajo posterior necesario (por ejemplo, banderas
- Archive os logs do gate de release para auditoria - guarde-os ao lado dos artefatos
  asinados.

Seguir este pipeline mantem o CLI, os crates SDK y os material de Gobernanza Alinhados
em cada ciclo de liberación.