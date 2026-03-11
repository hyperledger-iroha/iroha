---
lang: es
direction: ltr
source: docs/portal/docs/reference/address-safety.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Seguranca e acessibilidade de enderecos
descripción: Requisitos de UX para presentar y compartir enderecos Iroha con seguranca (ADDR-6c).
---

Esta página captura o entregavel de documentacao ADDR-6c. Aplique estas restricciones a billeteras, exploradores, herramientas de SDK y cualquier superficie del portal que renderice o enderecos voltados para personas. O modelo de dados canónico vive em `docs/account_structure.md`; Una lista de verificación a continuación explica cómo exportar estos formatos sin comprometer la seguridad o la accesibilidad.

## Fluxos seguros de compartilhamento- Por padrao, toda acao de copiar/compartilhar debe usar o endereco I105. Exiba o dominio resuelto como contexto de apoio para mantener una cadena con suma de comprobación en destaque.
- Ofereca um atalho de "Compartilhar" que incluye o endereco em texto puro y un código QR derivados de la misma carga útil. Permita que una persona usuaria inspeccione ambos antes de confirmar.
- Cuando el espacio requiera truncagem (tarjetas pequeñas, notificaciones), mantenga el prefijo legivel inicial, use reticencias y preserve los últimos 4-6 caracteres para que a ancora do checksum sobreviva. Disponibilice un gesto de toque/atalho de teclado para copiar una cadena completa sin truncarse.
- Evite desincronizar con el portapapeles exibindo un brindis de confirmación que muestra exactamente una cadena I105 copiada. Cuando usted tiene telemetría, conte tentativas de copiar versus acciones de compartir para detectar rápidamente retrocesos de UX.

## Salvaguardas para IME y entrada- Rejeite entrada non-ASCII en campos de endereco. Cuando surgen artefatos de IME (ancho completo, Kana, marcas de tom), mostre um aviso inline explicando como trocar o teclado para entrada latina antes de tentar novamente.
- Forneca uma área de pegar en texto puro que elimina marcas combinatorias y sustituye espacios en blanco por espacios ASCII antes de la validación. Esto evita perder el progreso cuando el usuario desativa o IME no meio do fluxo.
- Fortaleca a validacao contra carpinteros de ancho cero, selectores de variación y otros puntos de código Unicode furtivos. Registre una categoría de punto de código rechazado para que las suites de fuzzing puedan incorporar telemetría.

## Expectativas para tecnologías asistenciales- Anote cada bloque de endereco con `aria-label` o `aria-describedby` que detalla el prefijo legivel y el grupo de carga útil en bloques de 4-8 caracteres ("ih guión b tres dos ..."). Esto impide que los lectores de tela produzcan un flujo de caracteres inteligivel.
- Anuncie eventos de copy/share bem-sucedidos por meio de uma live region "polite". Inclua o destino (portapapeles, hoja para compartir, QR) para que a pessoa usuaria saiba que acao foi concluida sem mover o foco.
- Forneca texto `alt` descriptivo para previsualización de QR (por ejemplo, "Endereco I105 para `<account>` na cadena `0x1234`"). Coloque al lado del lienzo del QR un botao "Copiar endereco em texto" para personas con baja visa.

## Enderecos comprimidos somente Sora

- Gating: oculta una cadena comprimida `sora...` atras de uma confirmacao explicita. A confirmacao deve deixar claro que ese formato funciona en cadenas Sora Nexus.
- Rotulaje: toda corrección debe incluir una insignia visible "Somente Sora" y una información sobre herramientas explicando por que otras redes exigen el formato I105.
- Protecoes: se o discriminante de chain ativo nao for a alocacao Nexus, recuse gerar o endereco comprimido e redirecione o usuario de volta para I105.
- Telemetría: registre cuantas veces o formato comprimido y solicitado y copiado para que o playbook de incidentes consiga detectar picos de compartilhamento acidental.## Puertas de calidad

- Estas pruebas de UI automatizadas (o suites de accesibilidad sin libro de cuentos) para garantizar que los componentes de endereco exponen los metadatos ARIA necesarios y que envían mensajes de rechazo de IME aparecam.
- Incluye escenarios de control de calidad manual para entrada vía IME (kana, pinyin), pasaje con lector de tela (VoiceOver/NVDA) y copia vía QR en temas de alto contraste antes de lanzar.
- Torne esses checks visiveis nas checklists de release, juntamente con los testes de paridade I105, para que los regresos continúen bloqueados ate serem corrigidas.