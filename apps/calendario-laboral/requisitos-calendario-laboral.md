# Calendario laboral — Requisitos

Aplicación de consola en Rust para gestionar el calendario laboral de uno o varios años. Todo día es `Working` salvo que un `Mark` o una regla diga otra cosa.

| | |
|---|---|
| **Fecha** | 2026-11-13 |
| **Estado** | Implementados RF-01 a RF-11, RF-14 a RF-20 y RN-01 a RN-30; pendientes RF-12 y RF-13 (§9) |
| **Binario** | `calendario` (crate `calendario-laboral`) |
| **Guía de desarrollo** | [CLAUDE.md](CLAUDE.md): comandos para construir, ejecutar y verificar, arquitectura y convenciones |

Los conceptos se nombran en inglés, igual que en el código. Lo que ve el usuario (textos, subcomandos, valores de la CLI y del JSON) está en español; el glosario (§2.1) relaciona ambos.

---

## 1. Alcance

- **Incluye:** varios `Year` en un mismo fichero, `Mark` de días y rangos, consulta, vistas de mes y año con colores, `Summary` anual con horas y `quota` de vacaciones.
- **No incluye:** varios usuarios, interfaz gráfica, nóminas.

## 2. Modelo de dominio

### 2.1 Glosario

| Concepto | Significado | Para el usuario |
|---|---|---|
| `Calendar` | El contenido del fichero: configuración común y años | fichero `calendario.json`, `config` |
| `Year` | Un año con su `quota`, sus `Hours` y sus `Mark` | `add-year`, `remove-year`, `year` |
| `Mark` | Un `MarkKind` sobre un `DateRange`, con id y descripción opcional | «marca», `mark`, `edit`, `unmark`, `list` |
| `MarkKind` | Lo que fija un `Mark`: `Working`, `Reduced`, `Holiday` o `Vacation` | `laborable`, `reducido`, `festivo`, `vacaciones` |
| `DateRange` | Rango de días `[from, to]`, ambos incluidos | `<fecha> [<hasta>]` |
| `DayKind` | Tipo efectivo de un día (§2.2) | nombre, color y barra `▌` o letra |
| `Marker` | Cómo se identifica el tipo junto al número del día: `Bar` (barra de color) o `Letter` (RN-20) | `15 ▌` o `15 F` |
| `Category` | Clase de un `DayKind`: `Working`, `NonWorking` o `Absence` | laborable, no laborable, ausencia |
| `Shift` | Una de las cuatro jornadas con horas configurables | `<jornada>` de `hours` |
| `Hours` | Las horas de los cuatro `Shift` de un `Year` | `hours` |
| `quota` | Días de vacaciones de un `Year` | «cupo», `quota` |
| `Weekend` | Regla que calcula el fin de semana a partir del día de la semana (RN-04) | «fin de semana», `weekend` |
| `ReducedWeekday` | Regla que hace `Reduced` ciertos días de la semana, el viernes por defecto (RN-18) | «jornada reducida de los viernes», `reduced-days` |
| `Month` | Un mes del año, de `January` a `December` (`chrono::Month`) | «junio» o «6» en `month` |
| `Summer` | Periodo fijo formado por `Month::July` y `Month::August` | «verano» |
| `DefaultVacation` | Vacaciones calculadas de un `Year` sin `Vacation` marcadas (RN-14) | «vacaciones por defecto» |
| `ResolvedDay` | Fecha con su `DayKind`, horas, `Origin` y descripción | `day` |
| `Origin` | Qué fijó el `DayKind`: `Mark(id)`, `Weekend`, `ReducedWeekday`, `Default` o `DefaultVacation` | «marca #1», «por defecto»… |
| `Summary` | Totales de un `Year`, con sus 12 `MonthTotal` | `summary` |
| `MonthTotal` | Días y horas laborables de un mes y horas acumuladas del año hasta ese mes (RF-14) | «Horas por mes» |

### 2.2 `DayKind`

| `DayKind` | `Category` | Origen | `Shift` (horas por defecto) | Se muestra como | Letra | Color |
|---|---|---|---|---|---|---|
| `Working` | `Working` | Por defecto o `Mark` `Working`, fuera del `Summer` | `Working` (8,5 h) | Laborable | L | Azul marino |
| `Reduced` | `Working` | `Mark` `Reduced` o regla `ReducedWeekday`, fuera del `Summer` | `Reduced` (6,0 h) | Reducido | R | Cian |
| `SummerWorking` | `Working` | Por defecto o `Mark` `Working`, en `Summer` | `SummerWorking` (8,5 h) | Laborable verano | l | Verde |
| `SummerReduced` | `Working` | `Mark` `Reduced` o regla `ReducedWeekday`, en `Summer` | `SummerReduced` (6,0 h) | Reducido verano | r | Verde pálido |
| `Weekend` | `NonWorking` | Regla `Weekend` | — (0 h) | Fin de semana | S | Gris |
| `Holiday` | `Absence` | `Mark` `Holiday` | — (0 h) | Festivo | F | Rojo |
| `Vacation` | `Absence` | `Mark` `Vacation` o `DefaultVacation`; consume `quota` | — (0 h) | Vacaciones | V | Naranja en negrita |

### 2.3 Resolución del `DayKind` efectivo

El `DayKind` de un día es el de **mayor prioridad** entre los `Mark` que lo cubren y las reglas `Weekend` y `ReducedWeekday`. Si no aplica ninguno, el día es `Working`, o `SummerWorking` si cae en `Summer`.

**Prioridad:** `Holiday` > `Weekend` > `Vacation` > `Mark` `Working` > `Reduced` (`Mark` o regla `ReducedWeekday`) > `Working` por defecto

- Así, una `Vacation` de lunes a domingo no consume fines de semana ni festivos.
- Si dos `Mark` del mismo `MarkKind` cubren el día, gana el más antiguo, y la descripción es la suya.
- Un `Mark` `Reduced` sobre un viernes gana a la regla `ReducedWeekday` (mismo `DayKind`), así el `Origin` y la descripción son los del `Mark`.
- Un `Mark` `Working` sobre un viernes le devuelve la jornada completa (RN-26), pero no convierte en laborable un fin de semana, un festivo ni unas vacaciones.
- `DefaultVacation` (RN-14) se aplica después, solo sobre los días que hayan quedado en `Category` `Working`.

### 2.4 Entidades

- **`Calendar`**: contiene la configuración común (días de `Weekend` y días de `ReducedWeekday`) y uno o varios `Year`.
- **`Year`**: tiene un número único entre 1 y 9999, una `quota` (25 días por defecto), sus `Hours` (RN-25) y sus `Mark`.
- **`Mark`**: tiene un `id` único en el fichero, un `MarkKind`, un `DateRange` y una descripción opcional. Un día suelto es un `DateRange` con `from` = `to`.
- **`ResolvedDay`**: es la fecha con su `DayKind` efectivo, sus horas, su `Origin` y la descripción.

## 3. Reglas de negocio

| ID | Regla |
|---|---|
| RN-01 | Las fechas usan formato ISO `YYYY-MM-DD` y deben existir: `2026-02-30` y `2026-6-5` se rechazan. |
| RN-02 | Todo `Mark` debe pertenecer a un `Year` existente en el fichero, y los `DateRange` que cruzan de año se rechazan. Consultar un `Year` que no está en el fichero también es un error. |
| RN-03 | En un `DateRange`, `from` ≤ `to`. |
| RN-04 | El `Weekend` no se marca: se calcula a partir del día de la semana. Por defecto es sábado y domingo, y se cambia con `weekend` (RF-17) para todo el calendario; sin días, no hay fin de semana. |
| RN-05 | Cuando hay `Mark` solapados se conservan todos, y el `DayKind` efectivo lo decide la prioridad (§2.3). |
| RN-06 | Un `Mark` con el mismo `MarkKind` y `DateRange` que otro existente no se añade, aunque cambie la descripción. Se avisa con el id del existente y no es un error. |
| RN-07 | Los años bisiestos tienen 366 días. |
| RN-08 | El `Summer` es fijo, del 1 de julio al 31 de agosto. En ese periodo, `Working` pasa a `SummerWorking` y `Reduced` pasa a `SummerReduced`. |
| RN-09 | Los días laborables son los de `Category` `Working`. Las horas laborables son la suma de las horas de su `Shift`. |
| RN-10 | La `quota` es de 25 días por `Year` por defecto y se configura para cada `Year` (RF-10). Solo cuentan los días cuyo `DayKind` efectivo es `Vacation`. |
| RN-11 | Si un `Mark` `Vacation` supera la `quota`, o si la `quota` baja por debajo de las vacaciones ya usadas, el cambio se guarda igualmente y se muestra un aviso con los días excedidos. |
| RN-12 | La semana empieza en lunes, que es la primera columna de las vistas. |
| RN-13 | Una operación inválida muestra un error claro y no modifica el fichero. |
| RN-14 | Si un `Year` no tiene ningún `Mark` `Vacation`, se aplica `DefaultVacation`: desde el 1 de julio, solo en días de `Category` `Working` (se saltan fines de semana y festivos), hasta agotar la `quota` o el año. No se guarda en el fichero y desaparece en cuanto se marca alguna `Vacation`. |
| RN-15 | Las horas se muestran con coma decimal (`8,5`) y se aceptan con coma o con punto. |
| RN-16 | El id de un `Mark` nuevo es el mayor id del fichero más uno, o 1 si no hay `Mark`. |
| RN-17 | Las horas de un `Shift` están entre 0 y 24. |
| RN-18 | Los días de la semana de la regla `ReducedWeekday` (por defecto, todos los viernes del año) son `Reduced`, o `SummerReduced` en `Summer`, con las horas de esos `Shift`: un valor para el `Summer` y otro para el resto del año. Tienen la prioridad de `Reduced` (§2.3). Se cambian con `reduced-days` (RF-16) para todo el calendario; sin días, no hay días reducidos por regla. |
| RN-19 | Las horas por mes (RF-14) son horas teóricas, como las de RN-09: la suma de las horas de los días laborables del mes. La suma de los 12 meses es exactamente el total anual de RF-08, en días y en horas. El acumulado de un mes es la suma desde enero hasta ese mes, así que el de diciembre es el total anual. Un mes sin días laborables muestra 0. |
| RN-20 | En las vistas, el número del día va sin color, seguido de un espacio y de un símbolo con el color del `DayKind`. Entre un día y el siguiente hay dos espacios, para que se vea a qué número pertenece cada símbolo. Si la salida tiene color, el símbolo es una barra gruesa `▌` (media celda, `15 ▌`), que deja leer mejor los números y muestra bastante color para distinguir los tonos. Sin color (al redirigir, con `NO_COLOR` o en un terminal sin color), la barra no distinguiría nada, así que en su lugar va la letra del `DayKind` (`15 F`). La leyenda, `day` y `summary` usan el mismo símbolo. |
| RN-21 | En la vista de año, los meses quedan dentro de una rejilla de líneas finas punteadas en gris, que no compite con los colores de los días: una línea vertical `┊` entre dos meses de la misma fila y a la izquierda y a la derecha, con un espacio a cada lado, y una línea horizontal `┈` entre dos filas de meses, arriba y abajo. Los cruces y las esquinas usan `┼`, `┬`, `┴`, `├`, `┤`, `┌`, `┐`, `└` y `┘`. La rejilla ocupa 130 columnas; con la cuadrícula de vacaciones a su derecha (RN-27), la vista de año ocupa 169. Sin color se mantienen las mismas líneas, sin el gris. |
| RN-22 | Si la salida es un terminal, `month`, `year` y `summary` se muestran en una ventana a pantalla completa (la pantalla alternativa del terminal), con el mismo texto y colores que en la consola y una línea de ayuda abajo. `Esc`, `q` o `Ctrl+C` cierran la ventana y la consola vuelve tal como estaba, sin la vista. Si la vista no cabe, `↑` `↓` (o `k` `j`), `RePág` `AvPág` e `Inicio` `Fin` la desplazan; lo que no cabe a lo ancho se corta. Al redirigir la salida, o con `--no-tui`, la vista se imprime en la consola. `day`, `list` y los comandos que cambian el fichero siempre imprimen en la consola. |
| RN-23 | La vista de año empieza con una franja, dentro del marco de la rejilla (RN-21), con el título `Calendario laboral AAAA` centrado y en negrita; una línea `┈` la separa de la primera fila de meses. Como el año ya está en la franja, cada mes se titula solo con su nombre (`Enero`). La vista de mes mantiene el año en su título (`Junio 2026`). |
| RN-24 | La vista de año termina, bajo los días trabajados (RN-29), con las horas laborables del año (las de RF-08) y la media mensual: esas horas divididas entre 12, aunque algún mes no tenga horas, como medida del esfuerzo mensual medio. Entre paréntesis, esa media en días de 8 h (media / 8), una jornada de referencia fija que no depende de las horas configuradas (RF-09): `Horas laborables del año: 1888,5 h` y `Media mensual: 157,38 h (19,67 días de 8 h)`. |
| RN-25 | Las `Hours` son de cada `Year`: cambiarlas en un año no altera las horas ni los totales de los demás. Un `Year` nuevo hereda las `Hours` del año anterior más cercano del fichero; si no hay ninguno anterior, las del siguiente; y si el fichero no tiene años, las de por defecto (8,5 h las jornadas completas y 6 h las reducidas). |
| RN-26 | Un `Mark` `Working` (`laborable`) fuerza la jornada completa, `Working` o `SummerWorking` en `Summer`, en días que serían `Reduced` por la regla `ReducedWeekday` o por un `Mark` `Reduced`. Su prioridad está entre `Vacation` y `Reduced` (§2.3), así que no cambia fines de semana, festivos ni vacaciones. |
| RN-27 | Las vistas de mes y de año muestran, a la derecha del calendario, una cuadrícula con las vacaciones del año: un cuadro por día del cupo, cinco por fila (5×5 con 25 días), primero los **usados** en naranja (el color de `Vacation`) y después los **restantes** en verde. Usados son todos los días de `DayKind` `Vacation` del año, ya disfrutados o reservados con una marca para más adelante, también los de `DefaultVacation` (RN-14): es la misma cuenta que «Vacaciones restantes» (RN-10), así que un día solo vuelve a verde si se libera, quitando o cambiando su marca. Restantes, el cupo menos los usados, nunca por debajo de cero. Si los usados superan el cupo, hay un cuadro por cada uno, todos naranjas; como mucho hay 366 cuadros. Cada cuadro son dos columnas (`██`), separados por un espacio. A la derecha, centrados en vertical, el título `Vacaciones` y los días de cada estado (`██ Usadas     20`, `░░ Restantes   5`). Sin color, los restantes se dibujan con `░░`. En la vista de mes, la cuadrícula empieza en la línea del título, y la vista cabe en 80 columnas; si la cuadrícula es más alta que el mes (cupo de más de 30 días), el mes se alarga con líneas en blanco. En la de año empieza en el título de la primera fila de meses, a 4 espacios del marco, y la vista ocupa 169 columnas. |
| RN-28 | En la ventana de RN-22, `r` o `R` recargan la vista: vuelven a leer el fichero y a lanzar la misma consulta, y la vista se redibuja desde la misma línea (o desde la última que deje la pantalla llena, si el texto nuevo es más corto). Así se ven los cambios hechos desde otra consola sin cerrar la ventana. Si el fichero no se puede leer, la vista sigue como estaba y la línea de ayuda muestra `No se ha podido recargar: <error>` hasta la siguiente recarga. La línea de ayuda incluye `r: recargar`. |
| RN-29 | Bajo «Vacaciones restantes», las vistas de mes, de año y `summary` muestran los días trabajados del año entre los días del año, y su porcentaje con un decimal: `Días trabajados: 224 / 365 (61,4%)`. Días trabajados son los laborables del año (RN-09: `Working`, `Reduced`, `SummerWorking` y `SummerReduced`), los mismos de «Días laborables»; días del año, 365, o 366 si es bisiesto. Es el año entero, también los días que aún no han llegado. Un año sin días da 0. |
| RN-30 | Las vistas de mes y de año muestran, debajo de la cuadrícula de vacaciones (RN-27) y tras una línea en blanco, la **próxima ausencia** desde la fecha actual. Empieza en el primer día de `Category` `Absence` (`Holiday` o `Vacation`, también las de RN-14) desde hoy, incluido, y sigue por los días seguidos que no se trabajan (fines de semana, festivos y vacaciones) hasta el último festivo o día de vacaciones del tramo; puede pasar al año siguiente si está en el calendario. Se busca en los años del calendario desde el actual, sea cual sea el que se muestra. Son cuatro líneas: el título `Próxima ausencia` en negrita; la barra del color de cada tipo de ausencia del tramo (o su letra, sin color, RN-20) con su etiqueta, en el orden en que aparecen (`▌ Festivo · ▌ Vacaciones`); las fechas (`el 8 de diciembre`, `del 7 al 8 de diciembre`, `del 28 de diciembre al 6 de enero`); y los días de ausencia del tramo con cuándo empieza (`2 días laborables · en 23 días`, `· hoy`, `· mañana`). Si no hay ninguna, debajo del título va `No hay más ausencias`. En la vista de mes, el mes se alarga con líneas en blanco si hace falta, y la vista sigue cabiendo en 80 columnas. En la de año, la sección empieza en el título de la segunda fila de meses, para no quedar junto a la línea de la rejilla. |

## 4. Requisitos funcionales

| ID | Requisito | Prioridad |
|---|---|---|
| RF-01 | Añadir un `Year` al fichero | Must |
| RF-02 | Marcar un día o un `DateRange` con un `MarkKind` y una descripción opcional | Must |
| RF-03 | Eliminar uno o varios `Mark`; si alguno no existe, no se elimina ninguno (RN-13) | Must |
| RF-04 | Listar los `Mark`, todos o los de un `Year`, ordenados por fecha | Must |
| RF-05 | Consultar el `ResolvedDay` de una fecha: `DayKind`, horas, `Origin` y descripción | Must |
| RF-06 | Vista de mes en cuadrícula de lunes a domingo, con colores, `Mark` del mes, leyenda y vacaciones restantes | Must |
| RF-07 | Vista del `Year` completo, tres meses por fila | Should |
| RF-08 | `Summary` anual: días por `DayKind`, días y horas laborables, y vacaciones usadas y restantes | Must |
| RF-09 | Configurar las `Hours` de los cuatro `Shift` de un `Year` (RN-25) | Must |
| RF-10 | Configurar la `quota` de un `Year` | Must |
| RF-11 | Persistir el `Calendar` en JSON y cargarlo al arrancar | Must |
| RF-12 | Importar festivos desde un fichero | Could |
| RF-13 | Exportar el calendario resuelto en CSV o JSON | Could |
| RF-14 | Sumatorio mensual (`MonthTotal`): días laborables, horas laborables y horas acumuladas del año. Se muestra como tabla de 12 meses con fila de total en `summary`, como una línea en `month` y bajo cada mes en `year` (solo horas y días, por el ancho) | Must |
| RF-15 | Consultar la configuración: los días de `Weekend` y de `ReducedWeekday`, y de cada `Year` su `quota` y sus `Hours` | Should |
| RF-16 | Configurar los días de la semana de la regla `ReducedWeekday`, o ninguno (RN-18) | Should |
| RF-17 | Configurar los días de la semana del `Weekend`, o ninguno (RN-04) | Should |
| RF-18 | Modificar un `Mark` (su `MarkKind`, sus fechas o su descripción) conservando su `id`. Se aplican las reglas de `mark` (RN-01 a RN-03 y RN-11), y si queda idéntico a otro `Mark` (RN-06) es un error | Should |
| RF-19 | Al añadir un `Year`, copiar los `Mark` `Holiday` de otro `Year` con el mismo día, mes y descripción; los que no existen en el año nuevo (29 de febrero) se saltan, y se avisa de revisar los festivos que cambian de fecha cada año | Could |
| RF-20 | Eliminar un `Year` con su configuración y sus `Mark`. Si tiene `Mark`, hay que confirmarlo; si no, es un error que dice cuántos tiene (RN-13) | Could |

## 5. Interfaz de consola

El binario `calendario` funciona por subcomandos. Para ejecutarlo, consulta el [README](README.md).

| Comando | RF | Ejemplo |
|---|---|---|
| `add-year <año> [--festivos-de <año>]` | 01, 19 | `calendario add-year 2027 --festivos-de 2026` |
| `remove-year <año> [--confirmar]` | 20 | `calendario remove-year 2025 --confirmar` |
| `mark <tipo> <fecha> [<hasta>] [--desc <texto>]` | 02 | `calendario mark vacaciones 2026-06-05 2026-06-12` |
| `unmark <id>…` | 03 | `calendario unmark 4 5` |
| `edit <id> [--tipo <tipo>] [--desde <fecha>] [--hasta <fecha>] [--desc <texto>]` | 18 | `calendario edit 4 --hasta 2026-06-15` |
| `list [<año>]` | 04 | `calendario list 2026` |
| `day [<fecha>]` | 05 | `calendario day 2026-06-08` |
| `month [[<año>] <mes>]` | 06, 14 | `calendario month 2026 junio` |
| `year [<año>]` | 07, 14 | `calendario year 2026` |
| `summary [<año>]` | 08, 14 | `calendario summary 2026` |
| `hours <año> <jornada> <horas>` | 09 | `calendario hours 2026 laborable-verano 8,5` |
| `quota <año> <días>` | 10 | `calendario quota 2026 25` |
| `config` | 15 | `calendario config` |
| `reduced-days <día>… \| --ninguno` | 16 | `calendario reduced-days viernes` |
| `weekend <día>… \| --ninguno` | 17 | `calendario weekend sábado domingo` |

- **`<tipo>`** (`MarkKind`): `laborable`, `reducido`, `festivo` y `vacaciones`.
- **`<jornada>`** (`Shift`): `laborable`, `reducido`, `laborable-verano` y `reducido-verano`.
- **`<día>`** (`Weekday`): el nombre en español, con o sin tilde (`lunes`… `domingo`, `miercoles`, `sabado`).
- **`<mes>`** (`Month`): el nombre en español (`enero`… `diciembre`, también `setiembre`) o el número, con o sin cero delante (`6`, `06`).
- **Argumentos opcionales:** sin año, `month`, `year` y `summary` usan el año actual; `month` sin mes usa también el mes actual, y con solo el mes (`month junio`), el del año actual. `day` sin fecha muestra el día de hoy.
- **Ventana (RN-22):** en un terminal, `month`, `year` y `summary` se abren a pantalla completa, se recargan con `r` (RN-28) y se cierran con `Esc` o `q`. `--no-tui` los imprime en la consola.
- **Fichero:** se indica con `--file <ruta>`; si no, con la variable `CALENDARIO_FILE`; y si tampoco, se usa `./calendario.json`.
- **Salidas:**
  - Los resultados van a stdout, y los errores y avisos a stderr.
  - Códigos de salida: 0 si todo va bien, 1 si la operación falla y 2 si los argumentos no son válidos.
- **Ayuda:** `calendario --help` y `calendario <comando> --help`.

Cada día se muestra con su número, un espacio y una barra `▌` del color de su tipo (`15 ▌`). Sin color (al redirigir la salida o con `NO_COLOR`), la barra se sustituye por la letra del tipo (`15 F`, RN-20). Cada mes ocupa 40 columnas, y a la derecha del calendario va la cuadrícula de vacaciones (RN-27); la vista de año, con 3 meses por fila dentro de una rejilla gris punteada con marco (RN-21) y bajo la franja con el año (RN-23), necesita 130, y 169 con la cuadrícula de vacaciones. El ejemplo muestra la salida sin color:

```
               Junio 2026                   ██ ██ ██ ██ ██
Lu    Ma    Mi    Ju    Vi    Sá    Do      ██ ░░ ░░ ░░ ░░   Vacaciones
 1 L   2 L   3 L   4 L   5 V   6 S   7 S    ░░ ░░ ░░ ░░ ░░   ██ Usadas      6
 8 V   9 V  10 V  11 V  12 V  13 S  14 S    ░░ ░░ ░░ ░░ ░░   ░░ Restantes  19
15 L  16 L  17 L  18 L  19 R  20 S  21 S    ░░ ░░ ░░ ░░ ░░
22 L  23 L  24 L  25 L  26 R  27 S  28 S
29 L  30 L                                  Próxima ausencia
                                            V Vacaciones
                                            del 5 al 12 de junio
                                            6 días laborables · en 4 días

Marcas:
  #1   vacaciones 2026-06-05 → 2026-06-12  Viaje

Horas laborables de junio: 131,0 h en 16 días · acumulado del año: 985,5 h
Leyenda: L Laborable · R Reducido · l Laborable verano · r Reducido verano · S Fin de semana · F Festivo · V Vacaciones
Vacaciones restantes: 19 / 25
Días trabajados: 255 / 365 (69,9%)
```

Tabla de `summary` (RF-14):

```
Horas por mes    Días     Horas  Acumulado
  Enero            22     174,5      174,5
  …
  Total           255    2042,5
```

## 6. Persistencia

En el fichero JSON solo se guardan la configuración y los `Mark`. El `Weekend`, los días `ReducedWeekday`, los días por defecto, el `Summer` y `DefaultVacation` se calculan al cargar.

- **Fichero inexistente:** se parte de un `Calendar` vacío, y el fichero se crea con el primer cambio (normalmente `add-year`).
- **Validación:** al cargar se comprueban todas las reglas. Un fichero inválido da error y no se modifica.
- **Escritura:** se escribe en `<fichero>.tmp` y después se renombra, así un fallo nunca deja un fichero a medias.
- **Campos opcionales:** `weekend`, `reduced_days`, `hours`, `vacation_quota` y `marks` toman los valores por defecto si faltan. Un fichero anterior sin `reduced_days` pasa a tener los viernes reducidos.
- **Versiones:** se escribe la versión 2, con las `hours` dentro de cada año (RN-25). Se lee también la versión 1, que tenía unas `hours` comunes en la raíz: se aplican a cada año que no tenga las suyas, y al guardar el fichero pasa a la versión 2.
- **Valores de `kind`:** van en español (`laborable`, `reducido`, `festivo`, `vacaciones`), igual que en la CLI.

```json
{
  "version": 2,
  "weekend": ["saturday", "sunday"],
  "reduced_days": ["friday"],
  "years": [
    {
      "year": 2026,
      "vacation_quota": 25,
      "hours": { "laborable": 8.5, "reducido": 6.0, "laborable_verano": 8.5, "reducido_verano": 6.0 },
      "marks": [
        { "id": 1, "kind": "festivo", "from": "2026-03-03", "to": "2026-03-03", "description": "Fiesta local" },
        { "id": 2, "kind": "vacaciones", "from": "2026-06-05", "to": "2026-06-12" }
      ]
    }
  ]
}
```

## 7. Requisitos técnicos y de calidad

| ID | Requisito |
|---|---|
| RNF-01 | Usar Rust, edición 2024, con Cargo. El código debe funcionar en macOS y Linux. |
| RNF-02 | Seguir las guías oficiales: *The Rust Programming Language*, *Rust API Guidelines*, *Rust Style Guide* y la documentación de cada crate. Si una guía oficial no cubre algo, consultar Context7. |
| RNF-03 | Aplicar buenas prácticas: `rustfmt`, `cargo clippy -- -D warnings`, errores tipados con `thiserror`, sin `unwrap`/`expect` fuera de los tests, sin `unsafe`, documentación `///` en la API pública. |
| RNF-04 | Usar programación funcional al máximo: un núcleo de dominio puro e inmutable, sin E/S, y una capa imperativa fina en los bordes (CLI y fichero). Preferir iteradores y combinadores (`map`, `filter`, `fold`), `Option`/`Result` y funciones que devuelvan valores nuevos frente a la mutación. Evitar el estado mutable compartido. |
| RNF-05 | Organizar el código por capas: `domain` (tipos, reglas, resolución), `application` (casos de uso), `adapters` (CLI, consola y JSON). |
| RNF-06 | Tener tests para cada regla RN y cada caso de uso RF, con una cobertura de líneas **≥ 90 %** medida con `cargo llvm-cov -p calendario-laboral --fail-under-lines 90`. |
| RNF-07 | **Idiomas.** En inglés: los conceptos y el código (identificadores, nombres de tests). En español: la documentación (este documento, `CLAUDE.md`, rustdoc `///`/`//!` y comentarios) y todo lo que ve el usuario (mensajes, ayuda y valores de la CLI y del JSON). Los subcomandos se llaman como en §5. |
| RNF-08 | **Enumeraciones de negocio.** Siempre que sea posible, el código usa enumeraciones en lugar de códigos numéricos: `Month` en lugar de 1–12, `DayKind`, `MarkKind`, `Shift`, `Category`, `Origin` o `Weekday`. Los números solo aparecen en los bordes (la entrada de la CLI y las fechas de chrono). |

## 8. Verificación y trazabilidad

Cada requisito tiene al menos un test que actúa como regresión ante cambios futuros.

- **Nombres:** el test lleva en el nombre el ID que comprueba: `rn_05_…`, `rf_02_…`, o `rn_09_rn_10_rf_08_…` si comprueba varios. `cargo test -p calendario-laboral rn_14` ejecuta los de una regla.
- **Comprobación automática:** `tests/traceability.rs` lee las tablas de §3 y §4. Falla si una RN, o una RF que no sea *Could*, no tiene ningún test con su ID.
- **Niveles de test:**
  - Unitarios en cada módulo.
  - `tests/cli.rs` ejecuta el binario sobre un fichero temporal, como lo haría un usuario.
  - Las vistas se comprueban carácter a carácter sobre el texto sin color.
- **Cómo cambiar un requisito:**
  1. Editar este documento.
  2. Escribir o ajustar los tests con su ID.
  3. Cambiar el código.

  Los comandos de verificación están en [CLAUDE.md](CLAUDE.md#verificar).

## 9. Decisiones

Las decisiones vigentes que no se deducen de las reglas, con su motivo.

| Decisión | Motivo |
|---|---|
| CLI por subcomandos con clap, sin modo interactivo. La ventana de RN-22 solo muestra, desplaza y recarga la vista. | |
| La ventana se dibuja con `crossterm` a partir del mismo texto con códigos ANSI que la consola, no con ratatui. | Las vistas se escriben una sola vez; redirigir y los tests no cambian. |
| Colores con `anstyle` y `anstream`. | Ya son dependencias de clap y quitan el color al redirigir. |
| Documentación y textos en español; conceptos y código en inglés (RNF-07). Los errores y la ayuda de clap se traducen sobre su texto, y los errores poco habituales quedan con el de clap. | clap no se puede traducir. |
| Los `DateRange` que cruzan de año se rechazan, no se dividen (RN-02). | Cada `Mark` pertenece a un solo `Year`. |
| **RF-12** y **RF-13** quedan fuera de la v1. | |
| El mes es `chrono::Month` (RNF-08), con `MonthExt` para `ALL`, `of` e `is_summer`. | Ya es una enumeración ordenada, como `Weekday`. |
| La CLI construye sus valores (`MarkKind`, `Shift` y el mes) a partir del dominio con un único parser; el DTO JSON los repite a propósito. | Los nombres no se repiten en la CLI; los del JSON son el formato del fichero. |
| **RN-20**: barra `▌` del color del tipo, separada del número por un espacio, y dos espacios entre días. | Media celda de color basta para distinguir el rojo de `Holiday` del naranja de `Vacation`; el número se lee sin color. |
| Colores: `Working` azul y `Reduced` cian; en verano, `SummerWorking` verde y `SummerReduced` verde pálido (151 de la paleta de 256); `Weekend` gris, `Holiday` rojo y `Vacation` naranja en negrita (208). | Cada tipo tiene su color, y la jornada completa es más oscura que la reducida. No hay naranja entre los 16 colores básicos. |
| **RN-18**: los viernes reducidos usan los `Shift` `Reduced` y `SummerReduced`, sin jornadas propias. | |
| **RN-26**: `Working` va entre `Vacation` y `Reduced` en la prioridad. Trabajar un sábado queda fuera. | Sirve para forzar la jornada completa en un viernes, sin anular ausencias ni fines de semana. |
| **RF-14**: horas teóricas, no registradas. Un mes a 0 h por las vacaciones por defecto se muestra tal cual. | |
| **RN-24**: la media mensual divide siempre entre 12, y los días son de 8 h fijas. | Mide el esfuerzo mensual medio y no cambia al configurar las horas. |
| **RN-25**: las `Hours` son de cada `Year`, y un año nuevo hereda las del anterior. El JSON es la versión 2; la 1 se sigue leyendo. | Cambiar la jornada de un año no debe reescribir los pasados, y la jornada suele mantenerse de un año a otro. |
| **RF-16 y RF-17**: dejar sin días la jornada reducida o el fin de semana exige `--ninguno`. | Un comando sin días no los borra por error. |
| **RF-06**: con un solo argumento, `month` recibe el mes; `month 2026` no es válido. | |
| **RF-18**: `edit` conserva el `id`; si el resultado es idéntico a otro `Mark`, es un error. | Dos marcas idénticas no se pueden fusionar. |
| **RF-19**: `add-year --festivos-de` copia solo los `Holiday`, con la misma fecha, y siempre avisa. | Los festivos móviles hay que revisarlos con `edit`. |
| **RF-20**: `remove-year` con `Mark` exige `--confirmar`. | No se puede deshacer, y un error al escribir el año borraría sus marcas. |
| **RN-27**: usados son todos los días de vacaciones marcados, los mismos que cuenta «Vacaciones restantes»; la cuadrícula no depende de la fecha. La vista de año ocupa 169 columnas, y en la ventana se corta lo que no cabe. | Un día reservado ya no está disponible; solo vuelve a verde si se libera. |
| **RN-28**: `viewer` no conoce el fichero: `run` recibe una función que devuelve el texto nuevo, y `main` la construye con la ruta y el `Command`. | La ventana sigue sin efectos que no sean del terminal. |
| **RN-29**: los días trabajados son los laborables de todo el año, también los que no han llegado; el porcentaje va sin espacio (`61,4%`). | Como el resto de totales, mide el año planificado. |
| **RN-30**: la próxima ausencia depende de la fecha del reloj, a diferencia del resto de la vista, y une los fines de semana que quedan entre ausencias. | Una semana de vacaciones con su fin de semana, o un puente, es una sola ausencia. |

### Pendiente o posibles ampliaciones

- [ ] RF-12: importar festivos. Hay que definir antes el formato del fichero de entrada.
- [ ] RF-13: exportar en CSV o JSON.
- [ ] ¿Subcomandos en español (`marcar`, `anadir-anio`…), o alias? Ahora siguen los nombres de §5.
- [ ] Navegar entre meses y años en la ventana de RN-22.
- [ ] Objetivo de horas anuales del convenio y diferencia con las horas del calendario.
- [ ] Horas de ausencias por mes (vacaciones y festivos valorados en horas).
