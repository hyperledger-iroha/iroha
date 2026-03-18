---
lang: es
direction: ltr
source: docs/portal/docs/reference/address-safety.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Seguridad y accesibilidad de direcciones
descripción: Exigencias UX para presentar y compartir las direcciones Iroha de forma segura (ADDR-6c).
---

Esta página captura el archivo disponible de documentación ADDR-6c. Aplique estas restricciones a las billeteras, exploradores, herramientas SDK y toda la superficie del portal que le permite acceder o aceptar direcciones destinadas a los humanos. El modelo de mujeres canónicas se encuentra en `docs/account_structure.md`; La lista de verificación contiene comentarios explícitos que exponen estos formatos sin comprometer la seguridad o la accesibilidad.

## Flujo de partición en el sur- Por defecto, toda acción de copia/partición debe utilizar la dirección I105. Afichez le domaine resolu comme contexte d'appui afin que la chaine avec checksum reste au centre.
- Proponga una acción "Partager" que reagrupe la dirección en texto bruto y un QR para derivar la carga útil del meme. Permettez aux utilisateurs d'inspecter les dos avant de confirmer.
- Cuando el espacio imponga la troncatura (cartas minúsculas, notificaciones), conserve el prefijo lisible, afiche de elipses y guarde los 4-6 últimos caracteres para que el ancho de verificación sobreviva. Ofrez un geste/raccourci clavier pour copier la chaine complete sans troncature.
- Empechez la desynchronisation du presse-papiers y emita un brindis de confirmación que previsualise la cadena I105 exacta copia. La o la telemetría existen, comptez las tentativas de copia versus las acciones de partición para detectar rápidamente las regresiones UX.

## IME y protecciones de entrada- Rechace todas las entradas que no sean ASCII en los campos de direcciones. Cuando aparecen los artefactos de composición IME (ancho completo, Kana, marques de tonalite), se muestra un aviso en línea con un comentario explícito que pasa el teclado en saisie latine antes del reessayer.
- Fournissez una zona de collage en texto brut que suprime las marcas combinadas y reemplaza los espacios por los espacios ASCII antes de la validación. Esto evite perder la progresión si el usuario desactiva el IME en curso de flujo.
- Durcissez la validación contre les joiners de ancho cero, selectores de variación y otros puntos de código Unicode furtifs. Registre la categoría del punto de código rechazado para que las suites de fuzzing puedan importar la telemetría.

## Attentes pour les technologies d'assistance- Anote cada bloque de direcciones con `aria-label` o `aria-describedby` que epelle le prefixe lisible et segmente le payload en grupos de 4-8 caracteres ("ih guión b tres dos ..."). Evite que los lectores de pantalla produzcan un flujo de caracteres ininteligible.
- Annoncez les action de copie/partage reussies via une mise a jour de live region en mode polite. Incluez el destino (papeles de prensa, partición, QR) para que el usuario pueda realizar la acción sin cambiar el enfoque.
- Introduzca un texto descriptivo `alt` para la apertura QR (por ejemplo, "Adresse I105 pour `<account>` sur la chaine `0x1234`"). Ofrezca un respaldo "Copiar la dirección en texto" en la base de datos QR para los usuarios malintencionados.

## Direcciones comprimidas solo para Sora- Gating: cachez la chaine compressee `sora...` después de una confirmación explícita. La confirmación debe reiterar que el formato no funciona en las cadenas Sora Nexus.
- Etiqueta: cada aparición debe incluir una insignia visible "Solo Sora" y una información sobre herramientas explicativa para otras investigaciones exigentes en la forma I105.
- Guardrails: si el discriminador de cadena activo no pasa la asignación Nexus, rechace generar la dirección comprimida y redirigir hacia I105.
- Telemetría: registre la frecuencia de demanda y la copia de la forma comprimida para que el libro de jugadas de incidente detecte las imágenes de participación accidental.

## Puertas de calidad

- Actualice las pruebas de interfaz de usuario automatizadas (o las suites de storybook) para verificar qué componentes de direcciones exponen los metadones solicitados por ARIA y que aparecen los mensajes de rechazo de IME.
- Incluye escenarios de control de calidad manuales para la entrada IME (kana, pinyin), un lector de texto en pantalla (VoiceOver/NVDA) y una copia de QR en temas con un fuerte contraste antes del lanzamiento.
- Haga remontar estas verificaciones en las listas de verificación de liberación aux cotes des tests de parite I105 a fin de que las regresiones retengan bloques justos de corrección.