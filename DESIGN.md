---
name: FinanZillo
description: Tus finanzas como un catálogo de Factory Records; negro mate, una cifra enorme y el año dibujado en pulsos.
colors:
  bg: "#0b0b0c"
  surface: "#121214"
  surface-2: "#19191c"
  line: "#26262a"
  line-strong: "#3a3a3f"
  ink: "#f4f4f2"
  ink-2: "#b4b4b9"
  muted: "#8d8d93"
  signal: "#ff3b2f"
  gasto: "#ff6a45"
  ingreso: "#6e9bff"
  positive: "#37d6a4"
  warning: "#ffd23f"
  critical: "#ff5a36"
  bg-light: "#f2f2ef"
  surface-light: "#fbfbf9"
  surface-2-light: "#e8e8e4"
  line-light: "#d4d4cf"
  line-strong-light: "#a9a9a4"
  ink-light: "#0b0b0c"
  ink-2-light: "#35353a"
  muted-light: "#5f5f65"
  signal-light: "#e0281d"
  gasto-light: "#c23a1c"
  ingreso-light: "#2c58c9"
  positive-light: "#0a7d4f"
  warning-light: "#8f6400"
  critical-light: "#c23a1c"
  cat-azul: "#5b8cff"
  cat-verde-agua: "#2fd1a0"
  cat-amarillo: "#ffd23f"
  cat-lima: "#9be15d"
  cat-violeta: "#b98cff"
  cat-rojo: "#ff5a36"
  cat-rosa: "#ff8fb1"
  cat-naranja: "#ff9a3d"
  cat-azul-claro: "#9ec0ff"
  cat-amarillo-claro: "#ffe68a"
  cat-gris: "#8b8b90"
  cat-blanco: "#e6e6e1"
  cat-violeta-claro: "#dcc8ff"
  cat-verde-agua-claro: "#9af0d4"
  cat-azul-intenso: "#3d6bff"
  cat-cobre: "#d08a4e"
  cat-oliva: "#c9d16a"
typography:
  display:
    fontFamily: "Archivo Expanded ExtraBold"
    fontSize: "84px"
    fontWeight: 800
  display-sm:
    fontFamily: "Archivo Expanded ExtraBold"
    fontSize: "30px"
    fontWeight: 800
  headline:
    fontFamily: "Archivo Expanded SemiBold"
    fontSize: "22px"
    fontWeight: 600
  title:
    fontFamily: "Archivo Expanded SemiBold"
    fontSize: "17px"
    fontWeight: 600
  body:
    fontFamily: "Archivo"
    fontSize: "14px"
    fontWeight: 400
  body-strong:
    fontFamily: "Archivo Medium"
    fontSize: "14px"
    fontWeight: 500
  figure:
    fontFamily: "JetBrains Mono"
    fontSize: "13px"
    fontWeight: 400
  figure-strong:
    fontFamily: "JetBrains Mono"
    fontSize: "20px"
    fontWeight: 700
  label:
    fontFamily: "JetBrains Mono"
    fontSize: "10.5px"
    fontWeight: 400
    letterSpacing: "1.4px"
rounded:
  none: "0px"
  hairline: "1px"
  sm: "2px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "14px"
  lg: "16px"
  xl: "22px"
  gutter: "28px"
components:
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.bg}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.sm}"
    padding: "0 14px"
    height: "34px"
  button-secondary:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.sm}"
    padding: "0 14px"
    height: "34px"
  button-secondary-hover:
    backgroundColor: "{colors.surface-2}"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.ink-2}"
    rounded: "{rounded.sm}"
    padding: "0 14px"
    height: "34px"
  button-danger:
    backgroundColor: "transparent"
    textColor: "{colors.critical}"
    rounded: "{rounded.sm}"
    padding: "0 14px"
    height: "34px"
  icon-button:
    textColor: "{colors.ink-2}"
    rounded: "{rounded.sm}"
    size: "32px"
  icon-button-sm:
    textColor: "{colors.ink-2}"
    rounded: "{rounded.sm}"
    size: "26px"
  row:
    rounded: "{rounded.sm}"
    padding: "4px 10px"
  row-selected:
    backgroundColor: "{colors.surface-2}"
  segmented-selected:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.bg}"
    rounded: "{rounded.none}"
    padding: "0 13px"
    height: "32px"
  segmented-idle:
    backgroundColor: "transparent"
    textColor: "{colors.ink-2}"
    rounded: "{rounded.none}"
    padding: "0 13px"
    height: "32px"
  input:
    backgroundColor: "{colors.surface-2}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.sm}"
    padding: "7px 10px"
  input-amount:
    backgroundColor: "{colors.surface-2}"
    textColor: "{colors.ink}"
    typography: "{typography.figure}"
    rounded: "{rounded.sm}"
    padding: "7px 10px"
  nav-item:
    textColor: "{colors.ink-2}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.sm}"
    height: "34px"
  nav-item-active:
    backgroundColor: "{colors.surface-2}"
    textColor: "{colors.ink}"
  panel:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.sm}"
    padding: "16px"
  dialog:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.sm}"
    padding: "22px"
    width: "520px"
  toast:
    backgroundColor: "{colors.surface-2}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sm}"
    padding: "10px 14px"
---

# Design System: FinanZillo

## Overview

**Creative North Star: "Catálogo Factory"**

La app de escritorio trata el dinero del mes como una funda de catálogo de Factory Records: un plano negro mate, una sola cifra gigantesca que lo dice todo y el año de gasto dibujado como un gráfico de pulsos en tinta blanca. Todo lo demás es tipografía, filetes de un píxel y números monoespaciados alineados. La densidad es alta y deliberada: a 1920×1080 el mes entero cabe sin scroll, con barra lateral fija, columna central y columna lateral de cuentas y presupuestos.

El color no decora: tiene función. Gasto, ingreso, estado de presupuesto y la tira de color de cada categoría; y un único rojo de señal reservado para lo activo, el foco y el día de hoy. Las categorías llevan su color en forma de tira o bloque pequeño, como el código de color de una funda, nunca como relleno de superficie. Hay un tema claro que es el mismo sistema sobre papel (#f2f2ef) con los tonos oscurecidos para mantener el contraste.

El mundo rechaza el panel por defecto del sector (tarjetas grises con un acento y gráficas de línea genéricas) y el neobanco de degradados.

**Key Characteristics:**
- Negro mate con dos superficies tonales y filetes de 1 px; casi sin sombras.
- Una cifra protagonista por pantalla en Archivo Expanded ExtraBold.
- Todas las cifras en JetBrains Mono; los rótulos de datos en mono mayúscula con mucho espaciado.
- Esquinas casi rectas (2 px) en todo control y contenedor.
- El estado activo se marca con un bloque rojo en el borde derecho.
- Gráficas dibujadas a mano, sin rejilla de fondo, con la leyenda siempre en texto.

## Colors

Negros y blancos rotos casi neutros, más un juego de tonos industriales saturados que solo aparecen con función.

### Primary
- **Rojo Señal** (signal): el bloque del elemento activo (barra lateral, fila seleccionada, botón de acción en curso), el anillo de foco, el cursor de texto, el punto del día de hoy en el pulso y el punto de lectura de las líneas temporales. La selección de texto lo usa al 45 % (oscuro) o 25 % (claro).

### Secondary
- **Naranja Gasto** (gasto): cifras de gasto destacadas, barras de gasto y la intensidad del calendario de gasto. En las listas, un gasto se escribe en tinta con signo «−»; el naranja queda para totales y gráficas.
- **Azul Ingreso** (ingreso): cifras e importes de ingreso (con signo «+»), barras de ingreso y enlaces.

### Tertiary
- **Verde Estado** (positive): deltas favorables, TAE e intereses del mes, avisos de éxito y la proyección sana.
- **Amarillo Aviso** (warning): presupuesto entre el 75 % y el 100 %.
- **Rojo Crítico** (critical): saldo negativo, presupuesto pasado, errores y acciones destructivas.

### Neutral
- **Negro Mate** (bg): fondo de ventana, barra lateral y paneles centrales.
- **Grafito** (surface): paneles, diálogos y hover de filas.
- **Grafito Alto** (surface-2): campos, fila o sección seleccionada, toasts y hover de botones.
- **Filete** (line): separadores de 1 px, borde de panel, fondo de barra de progreso.
- **Filete Fuerte** (line-strong): borde de controles, ventanas y diálogos; ejes de gráficas.
- **Tinta** (ink): texto principal, trazo del pulso y relleno del botón principal.
- **Tinta Segunda** (ink-2): texto secundario, iconos en reposo, opciones no seleccionadas.
- **Gris Rótulo** (muted): rótulos en mayúsculas, pistas, ejes y estados de guardado.

### Category palette
Diecisiete tonos industriales (cat-*) sacados de los bloques de la funda: los once primeros tienen equivalente entre los colores que se pueden guardar; los seis últimos (blanco de la funda, violeta claro, verde agua claro, azul intenso, cobre y oliva) son huecos extra para cuando hay más categorías que tonos. Los tonos se reparten por categoría: la más usada elige primero y toma el hueco libre más cercano a su color guardado; las archivadas eligen al final. Gastos e ingresos se reparten por separado, así que dentro de cada tipo no hay dos categorías con el mismo tono. Todo punto de pintado pide el color con `cat_color_for(id, color)`; sobre el tema claro se oscurecen un 28 % hacia negro.

### Named Rules
**The Function Only Rule.** Ningún color aparece sin significado: gasto, ingreso, estado, categoría o señal. Las superficies son siempre neutras.

**The One Red Rule.** El rojo señal marca una sola cosa por zona: lo activo, lo enfocado o hoy. No se usa para decorar ni para destacar texto.

**The Never Alone Rule.** El color nunca va solo: cada importe lleva signo, cada estado de presupuesto su porcentaje y cada gráfica de dos series su leyenda en texto.

**The Strip Not Fill Rule.** El color de categoría se dibuja como tira vertical de 3–4 px, tira horizontal de 6–12 px o bloque de 10 px; nunca rellena una superficie grande.

**The One Colour Each Rule.** Dentro de gastos y dentro de ingresos, dos categorías nunca comparten tono: la más usada elige primero el tono industrial libre más cercano a su color guardado y las archivadas eligen al final.

## Typography

**Display Font:** Archivo Expanded (ExtraBold para cifras protagonistas, SemiBold para títulos)
**Body Font:** Archivo (Regular y Medium)
**Label/Mono Font:** JetBrains Mono (Regular y Bold)

**Character:** Una grotesca ancha y pesada que habla como la portada de un disco, una grotesca de UI neutra para leer y una monoespaciada tabular que alinea cada cifra como en un listado de catálogo. Todas las fuentes van incluidas en la app.

### Hierarchy
- **Display** (ExtraBold, 84 px): la cifra del saldo del mes; los céntimos y «€» bajan a 32 px en gris rótulo. Variantes de 30–54 px para la cifra principal de otras secciones (patrimonio, totales).
- **Headline** (SemiBold expandida, 22 px; 24 px en la cabecera de Mes): título de cada sección.
- **Title** (SemiBold expandida, 17 px; 15 px en ventanas de ayuda): título de diálogo.
- **Body** (Regular, 14 px; 11,5–13 px para pistas): texto corriente; Medium a 13,5–15 px en botones, navegación y nombres de fila.
- **Figure** (JetBrains Mono, 11–14 px; Bold a 13–22 px para totales): todos los importes, fechas, porcentajes y ejes.
- **Label** (JetBrains Mono, 10,5 px, espaciado 1,4 px, MAYÚSCULAS, gris rótulo; el rótulo de sistema `caps`): nombre de cada bloque de datos («SALDO DEL MES», «CUENTAS», «GASTO DIARIO · 12 MESES»).

### Named Rules
**The Mono Figures Rule.** Toda cifra (importe, fecha, porcentaje, eje, atajo) va en JetBrains Mono. Ninguna cifra en la grotesca salvo la cifra display.

**The One Giant Rule.** Cada pantalla tiene como mucho una cifra en display. Lo demás se jerarquiza con mono bold y rótulos.

**The Label Names Data Rule.** Los rótulos en mayúsculas nombran el dato o el bloque que tienen debajo; no son antetítulos decorativos sobre un titular.

## Layout

Ventana de escritorio con tres franjas fijas. Barra lateral de 212 px sobre el fondo (margen 20/14 px), separada por un filete vertical de 1 px; en ella, el logotipo (display 19 px), debajo la tira de cinco bloques de 22×6 px (cuatro tonos industriales y tinta), el número de catálogo del mes («FZ 2026·11»), diez secciones de 34 px de alto y, abajo, iconos de privacidad, tema, bloqueo y atajos con el estado de guardado cifrado. Área central con margen de 30 px a la izquierda y 28 px a la derecha.

En Mes, la columna lateral mide 336 px, separada de la central por 28 px con un filete en medio; cada columna tiene su propio scroll. El orden de la columna central es fijo: cabecera del mes, cifra y cuatro datos clave, línea de apunte rápido, rápidos, pulso de 12 meses y últimos movimientos. El pulso ocupa todo el ancho de la columna central y adapta su alto a la ventana (220–360 px) para que los últimos movimientos se vean sin scroll. El orden de la columna lateral también es fijo: cuentas, patrimonio, presupuestos, «en qué se va» (tira apilada de 12 px y las cinco primeras categorías), calendario y dos ideas automáticas.

La pantalla de bloqueo es una columna de 420 px centrada sobre el fondo: el logotipo en display 46 px, justo debajo la tira de cinco bloques de 34×10 px y después los campos; sin antetítulo sobre el logotipo. La ruta del archivo va abajo en mono 10,5 px gris rótulo.

Ritmo de espaciado: 4 px entre elementos ligados, 8 px de espaciado base, 14–16 px entre bloques y dentro de paneles, 22 px dentro de diálogos, 28 px entre columnas. Interactivos de 32–34 px de alto.

## Elevation & Depth

El sistema es plano y tonal: la profundidad se lee por el paso bg → surface → surface-2 y por filetes de 1 px, no por sombras. Solo las ventanas y menús flotantes tienen sombra, y los diálogos modales oscurecen el fondo (negro al 67 % en oscuro, 35 % en claro).

### Shadow Vocabulary
- **Ventana** (desplazamiento 0 10 px, desenfoque 28 px, negro al 63 % en oscuro / 20 % en claro): ventanas flotantes como la ayuda de atajos.
- **Menú** (desplazamiento 0 6 px, desenfoque 18 px, negro al 55 % / 16 %): desplegables y menús emergentes.

### Named Rules
**The Flat Plane Rule.** Paneles, filas, botones y gráficas no llevan sombra. Si algo necesita separarse, se le da un filete de 1 px o un paso tonal.

## Shapes

Rectángulos de esquina casi recta: 2 px en botones, campos, paneles, diálogos, menús, filas y toasts; 1 px en tiras de categoría, barras de progreso y celdas del calendario; 0 px en el control segmentado, las barras de las gráficas y los bloques de estado activo. Los bordes son siempre de 1 px. El bloque activo es un rectángulo macizo pegado al borde derecho (8 px en botones, 5 px en la navegación, 4 px en filas).

## Components

### Buttons
Rectángulos tipográficos, sin peso visual de más.
- **Shape:** esquina de 2 px, 34 px de alto, 14 px de relleno lateral, icono Phosphor Light de 17 px opcional.
- **Primary:** relleno de tinta con texto del fondo; uno por zona (Apuntar, Guardar). Hover y pulsado mezclan la tinta un 12 % y 25 % hacia el fondo.
- **Secondary:** contorno de 1 px en filete fuerte, sin relleno; hover pasa a surface-2 con contorno tinta-2.
- **Ghost:** solo texto en tinta-2 (acciones de fila, «Ver todos», «Deshacer»); hover con surface-2 y tinta.
- **Danger:** contorno y texto en rojo crítico; hover con un velo crítico al 15 %.
- **Activo:** bloque rojo señal de 8 px en el borde derecho.
- **Focus:** anillo de 1,5 px en rojo señal, 2 px por fuera.
- **Icon button:** cuadrado de 32 px con icono de 18 px, sin borde; hover con surface-2 y tinta; foco con filete interior de 1,5 px en rojo señal. La variante compacta (26 px, icono de 16 px) es la de las acciones dentro de filas.

### Chips
- **Segmentado** (Gasto | Ingreso | Transferencia): celdas pegadas de 32 px, esquina 0, contorno de 1 px; la seleccionada se invierte (relleno de tinta, texto del fondo).
- **Rápidos:** botones secundarios con el nombre y el importe del gasto rápido.

### Cards / Containers
- **Corner Style:** 2 px.
- **Background:** surface sobre el fondo.
- **Shadow Strategy:** ninguna (ver Elevation & Depth).
- **Border:** 1 px en filete; la línea de apunte rápido y los diálogos usan filete fuerte.
- **Internal Padding:** 16 px en paneles, 10×8 px en la línea de apunte, 22 px en diálogos.
Los paneles son contenedores de zona, no tarjetas decorativas: la mayoría de bloques van directamente sobre el fondo, separados por filetes y rótulos.

### Inputs / Fields
- **Style:** relleno surface-2, contorno de 1 px en filete fuerte, esquina 2 px, margen 10×7 px. Los importes van en mono de 14 px alineados a la derecha.
- **Focus:** contorno en tinta y cursor de 2 px en rojo señal.
- **Error:** mensaje en rojo crítico bajo el campo; toasts de error con contorno crítico.

### Navigation
Filas de 34 px con icono de 18 px y nombre en Archivo Medium 14 px. Reposo en tinta-2; hover con surface y el atajo «Ctrl+N» en mono gris a la derecha; activa con surface-2, tinta y bloque rojo señal de 5 px en el borde derecho.

### Rows
Filas de lista con margen 10×4 px: fecha en mono, concepto en tinta, tira de categoría de 3 px con su nombre, cuenta e importe con signo en mono a la derecha y acciones con el icon button compacto de 26 px. Hover con surface; seleccionada con surface-2 y bloque señal de 4 px a la derecha. 2 px entre filas y, donde la lista lo pide, filetes de 1 px.

### Progress
Barra fina sobre fondo de filete con esquina de 1 px. Su color sigue el estado del presupuesto: color de la categoría por debajo del 75 %, amarillo aviso del 75 % al 100 %, rojo crítico por encima, siempre con el porcentaje escrito al lado.

### Toasts
Esquina inferior derecha, surface-2 con contorno de 1 px (crítico si es error), icono de check en verde o aviso en rojo, y acción «Deshacer» en ghost cuando aplica.

### Pulso de 12 meses (signature)
Ocupa todo el ancho de la columna central. Un mes por línea, el más antiguo arriba, con el gasto diario suavizado (normalizado con potencia 0,7) como un pulso cuyas crestas, de hasta 0,45 veces el alto del gráfico, invaden las líneas de detrás; cada curva se rellena con el color del fondo para tapar las de atrás. Trazo de tinta de 1,1 px al 55–62 % en reposo, 1,6 px al pasar el ratón y 2,6 px en el mes seleccionado. El mes en curso termina en un punto rojo señal de 4 px (hoy). Mes a la izquierda y total a la derecha en mono de 11 px; eje de días 1·8·15·22·29. Hover lee el total; clic abre ese mes.

### Otras gráficas
Dibujadas a mano, sin rejilla:
- **Barras del año:** parejas de barras finas de 5 px (ingreso en azul, gasto en naranja, 3 px entre ellas), esquina 0, sobre un eje de filete fuerte; leyenda en texto encima con bloques de 10 px y meses en mono 9,5 px.
- **Patrimonio:** trazo de tinta de 1,4 px, marcas cortas en el eje izquierdo en cifras redondas (paso 1-2-5 × 10ⁿ, unas tres marcas), ejes en filete y punto rojo de lectura al pasar el ratón. Las líneas temporales de una categoría usan el mismo dibujo con el tono de la categoría.
- **Calendario de gasto:** celdas con intensidad de naranja gasto, hoy con contorno de tinta, días futuros solo con filete.
- **Resto:** barras por día de la semana en tinta al 80 %, sparklines verdes o naranjas según la tendencia y la tira apilada de categorías del mes (12 px, 2 px entre tramos).

## Do's and Don'ts

### Do:
- **Do** poner todas las cifras en JetBrains Mono y alinear los importes a la derecha.
- **Do** marcar lo activo con el bloque rojo señal en el borde derecho, no con un relleno de color.
- **Do** acompañar cada color con texto: signo en importes, porcentaje en presupuestos, leyenda en gráficas.
- **Do** usar esquinas de 2 px y bordes de 1 px en todo control nuevo.
- **Do** nombrar cada bloque de datos con un rótulo mono en mayúsculas (10,5 px, espaciado 1,4 px, gris rótulo).
- **Do** pintar toda categoría con `cat_color_for(id, color)`, que respeta el reparto de tonos, y oscurecerla un 28 % en el tema claro.

### Don't:
- **Don't** rellenar superficies grandes con color de categoría o de estado; las superficies son neutras.
- **Don't** usar degradados ni tarjetas grises con un único acento.
- **Don't** poner sombras a paneles, filas o botones; solo a ventanas y menús flotantes.
- **Don't** añadir una segunda cifra display en la misma pantalla.
- **Don't** usar el rojo señal para texto decorativo o para algo que no sea activo, foco o hoy.
- **Don't** dibujar rejillas de fondo en las gráficas.
- **Don't** dar a una categoría un color fuera de los 17 tonos industriales ni repetir tono entre categorías del mismo tipo.
