---
lang: es
direction: ltr
source: docs/portal/docs/reference/address-safety.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Seguridad y accesibilidad de direcciones
descripción: Requisitos de UX para presentar y compartir direcciones de Iroha con seguridad (ADDR-6c).
---

Esta página captura el entregable de documentación ADDR-6c. La aplicación está restringida a billeteras, exploradores, herramientas de SDK y cualquier superficie del portal que renderice o acepte direcciones orientadas a personas. El modelo de datos canónico vive en `docs/account_structure.md`; la lista de verificación de abajo explica cómo exponer esos formatos sin comprometer la seguridad o la accesibilidad.

## Flujos seguros de comparticion- Por defecto, cada acción de copiar/compartir debe usar la dirección I105. Muestra el dominio resuelto como contexto de apoyo para que la cadena con checksum permanezca al frente.
- Ofrece una acción "Compartir" que incluye la dirección en texto plano y un QR derivado del mismo payload. Permite que las personas inspeccionen ambos antes de confirmar.
- Cuando el espacio obliga a truncar (tarjetas pequeñas, notificaciones), conserva el prefijo legible, muestra puntos suspensivos y reten los últimos 4-6 caracteres para que sobreviva el ancla del checksum. Provee un toque/atajo de teclado para copiar la cadena completa sin truncamiento.
- Evita la desincronizacion del portapapeles emitiendo un brindis de confirmación que previsualice la cadena I105 exacta que se copia. Donde haya telemetría, cuenta intentos de copia versus acciones de compartir para detectar regresiones de UX rápidamente.

## IME y salvaguardas de entrada- Rechaza entradas no ASCII en campos de dirección. Cuando aparecen artefactos de composición IME (ancho completo, Kana, marcas de tono), muestra una advertencia en línea que explica cómo cambiar el teclado a entrada en latín antes de reintentar.
- Provee una zona de pegado en texto plano que elimine marcas combinantes y reemplace espacios en blanco por espacios ASCII antes de validar. Esto evita que la persona pierda progreso cuando desactiva el IME a mitad de flujo.
- Endurece la validación contra ensambladores de ancho cero, selectores de variación y otros puntos de código Unicode sigilosos. Registre la categoría del punto de código rechazado para que los fuzzing suites puedan importar la telemetría.

## Expectativas de tecnología asistencial- Anota cada bloque de dirección con `aria-label` o `aria-describedby` que elimina el prefijo legible y agrupa el payload en bloques de 4-8 caracteres ("ih dash b three two ..."). Esto evita que los lectores de pantalla produzcan un flujo de caracteres ininteligible.
- Anuncia los eventos de copia/comparticion exitosos mediante una actualizacion de live region en modo polite. Incluye el destino (portapapeles, hoja de compartir, QR) para que la persona sepa que la acción se completa sin mover el foco.
- Provee texto `alt` descriptivo para las vistas previas de QR (p. ej., "Direccion I105 para `<account>` en la cadena `0x1234`"). Incluye un respaldo "Copiar dirección como texto" junto al lienzo de QR para personas con baja visión.

## Direcciones comprimidas solo Sora- Gating: oculta la cadena comprimida `sora...` detrás de una confirmación explícita. La confirmación debe reiterar que el formato solo funciona en cadenas Sora Nexus.
- Etiquetado: cada aparición debe incluir una insignia visible "Solo Sora" y una información sobre herramientas que explica por que otras redes requieren la forma I105.
- Guardrails: si el discriminante de cadena activo no es la asignación de Nexus, rechaza generar la dirección comprimida y dirige a la persona de vuelta a I105.
- Telemetria: registre con que frecuencia se solicita y se copia la forma comprimida para que el playbook de incidentes detecte picos de compartición accidental.

## Puertas de calidad

- Extiende las pruebas UI automatizadas (o suites de a11y en storybook) para afirmar que los componentes de direcciones exponen los metadatos ARIA requeridos y que aparecen los mensajes de rechazo por IME.
- Incluye escenarios de control de calidad manual para entrada IME (kana, pinyin), pase de lector de pantalla (VoiceOver/NVDA) y copia de QR en temas de alto contraste antes del lanzamiento.
- Refleja estas comprobaciones en las checklists de release junto a las pruebas de paridad I105 para que las regresiones sigan bloqueadas hasta corregirse.