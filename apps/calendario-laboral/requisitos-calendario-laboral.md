# Calendario laboral — Requisitos

Aplicación de consola en Rust para gestionar el calendario laboral de uno o varios años. Todo día es `Working` salvo que un `Mark` o una regla diga otra cosa.

| | |
|---|---|
| **Fecha** | 2026-11-11 |
| **Estado** | v1 implementada: RF-01 a RF-11, RF-14 y RN-01 a RN-24 |
| **Binario** | `calendario` (crate `calendario-laboral`) |
| **Guía de desarrollo** | `CLAUDE.md`: comandos para construir, ejecutar y verificar, arquitectura y convenciones |

Los conceptos se nombran en inglés, igual que en el código. Lo que ve el usuario (textos, subcomandos, valores de la CLI y del JSON) está en español; el glosario (§2.1) relaciona ambos.

---

## 1. Alcance

- **Incluye:** varios `Year` en un mismo fichero, `Mark` de días y rangos, consulta, vistas de mes y año con colores, `Summary` anual con horas y `quota` de vacaciones.
- **No incluye:** varios usuarios, interfaz gráfica, nóminas.

## 2. Modelo de dominio

### 2.1 Glosario

| Concepto | Significado | Para el usuario |
|---|---|---|
| `Calendar` | El contenido del fichero: configuración común y años | fichero `calendario.json` |
| `Year` | Un año con su `quota` y sus `Mark` | `add-year`, `year` |
| `Mark` | Un `MarkKind` sobre un `DateRange`, con id y descripción opcional | «marca», `mark`, `unmark` |
| `MarkKind` | Lo que fija un `Mark`: `Reduced`, `Holiday` o `Vacation` | `reducido`, `festivo`, `vacaciones` |
| `DateRange` | Rango de días `[from, to]`, ambos incluidos | `<fecha> [<hasta>]` |
| `DayKind` | Tipo efectivo de un día (§2.2) | nombre, color y barra `▌` o letra |
| `Marker` | Cómo se identifica el tipo junto al número del día: `Bar` (barra de color) o `Letter` (RN-20) | `15 ▌` o `15 F` |
| `Category` | Clase de un `DayKind`: `Working`, `NonWorking` o `Absence` | laborable, no laborable, ausencia |
| `Shift` | Una de las cuatro jornadas con horas configurables | `<jornada>` de `hours` |
| `Hours` | Las horas de los cuatro `Shift` | `hours` |
| `quota` | Días de vacaciones de un `Year` | «cupo», `quota` |
| `Weekend` | Regla que calcula el fin de semana a partir del día de la semana | «fin de semana» |
| `ReducedWeekday` | Regla que hace `Reduced` ciertos días de la semana, el viernes por defecto (RN-18) | «jornada reducida de los viernes» |
| `Month` | Un mes del año, de `January` a `December` | «junio» o «6» en `month` |
| `Summer` | Periodo fijo formado por `Month::July` y `Month::August` | «verano» |
| `DefaultVacation` | Vacaciones calculadas de un `Year` sin `Vacation` marcadas (RN-14) | «vacaciones por defecto» |
| `ResolvedDay` | Fecha con su `DayKind`, horas, `Origin` y descripción | `day` |
| `Origin` | Qué fijó el `DayKind`: `Mark(id)`, `Weekend`, `ReducedWeekday`, `Default` o `DefaultVacation` | «marca #1», «por defecto»… |
| `Summary` | Totales de un `Year`, con sus 12 `MonthTotal` | `summary` |
| `MonthTotal` | Días y horas laborables de un mes y horas acumuladas del año hasta ese mes (RF-14) | «Horas por mes» |

### 2.2 `DayKind`

| `DayKind` | `Category` | Origen | `Shift` (horas por defecto) | Se muestra como | Letra | Color |
|---|---|---|---|---|---|---|
| `Working` | `Working` | Por defecto fuera del `Summer` | `Working` (8,5 h) | Laborable | L | Azul marino |
| `Reduced` | `Working` | `Mark` `Reduced` o regla `ReducedWeekday`, fuera del `Summer` | `Reduced` (6,0 h) | Reducido | R | Cian |
| `SummerWorking` | `Working` | Por defecto en `Summer` | `SummerWorking` (8,5 h) | Laborable verano | l | Verde |
| `SummerReduced` | `Working` | `Mark` `Reduced` o regla `ReducedWeekday`, en `Summer` | `SummerReduced` (6,0 h) | Reducido verano | r | Verde pálido |
| `Weekend` | `NonWorking` | Regla `Weekend` | — (0 h) | Fin de semana | S | Gris |
| `Holiday` | `Absence` | `Mark` `Holiday` | — (0 h) | Festivo | F | Rojo |
| `Vacation` | `Absence` | `Mark` `Vacation` o `DefaultVacation`; consume `quota` | — (0 h) | Vacaciones | V | Naranja en negrita |

### 2.3 Resolución del `DayKind` efectivo

El `DayKind` de un día es el de **mayor prioridad** entre los `Mark` que lo cubren y las reglas `Weekend` y `ReducedWeekday`. Si no aplica ninguno, el día es `Working`, o `SummerWorking` si cae en `Summer`.

**Prioridad:** `Holiday` > `Weekend` > `Vacation` > `Reduced` (`Mark` o regla `ReducedWeekday`) > `Working`

- Así, una `Vacation` de lunes a domingo no consume fines de semana ni festivos.
- Si dos `Mark` del mismo `MarkKind` cubren el día, gana el más antiguo, y la descripción es la suya.
- Un `Mark` `Reduced` sobre un viernes gana a la regla `ReducedWeekday` (mismo `DayKind`), así el `Origin` y la descripción son los del `Mark`.
- `DefaultVacation` (RN-14) se aplica después, solo sobre los días que hayan quedado en `Category` `Working`.

### 2.4 Entidades

- **`Calendar`**: contiene la configuración común (días de `Weekend`, días de `ReducedWeekday` y `Hours`) y uno o varios `Year`.
- **`Year`**: tiene un número único entre 1 y 9999, una `quota` (25 días por defecto) y sus `Mark`.
- **`Mark`**: tiene un `id` único en el fichero, un `MarkKind`, un `DateRange` y una descripción opcional. Un día suelto es un `DateRange` con `from` = `to`.
- **`ResolvedDay`**: es la fecha con su `DayKind` efectivo, sus horas, su `Origin` y la descripción.

## 3. Reglas de negocio

| ID | Regla |
|---|---|
| RN-01 | Las fechas usan formato ISO `YYYY-MM-DD` y deben existir: `2026-02-30` y `2026-6-5` se rechazan. |
| RN-02 | Todo `Mark` debe pertenecer a un `Year` existente en el fichero, y los `DateRange` que cruzan de año se rechazan. Consultar un `Year` que no está en el fichero también es un error. |
| RN-03 | En un `DateRange`, `from` ≤ `to`. |
| RN-04 | El `Weekend` no se marca: se calcula a partir del día de la semana. Por defecto es sábado y domingo, y solo se cambia editando `weekend` en el fichero. |
| RN-05 | Cuando hay `Mark` solapados se conservan todos, y el `DayKind` efectivo lo decide la prioridad (§2.3). |
| RN-06 | Un `Mark` con el mismo `MarkKind` y `DateRange` que otro existente no se añade, aunque cambie la descripción. Se avisa con el id del existente y no es un error. |
| RN-07 | Los años bisiestos tienen 366 días. |
| RN-08 | El `Summer` es fijo, del 1 de julio al 31 de agosto. En ese periodo, `Working` pasa a `SummerWorking` y `Reduced` pasa a `SummerReduced`. |
| RN-09 | Los días laborables son los de `Category` `Working`. Las horas laborables son la suma de las horas de su `Shift`. |
| RN-10 | La `quota` es de 25 días por `Year` por defecto y se configura para cada `Year` (RF-10). Solo cuentan los días cuyo `DayKind` efectivo es `Vacation`. |
| RN-11 | Si un `Mark` `Vacation` supera la `quota`, se guarda igualmente y se muestra un aviso con los días excedidos. |
| RN-12 | La semana empieza en lunes, que es la primera columna de las vistas. |
| RN-13 | Una operación inválida muestra un error claro y no modifica el fichero. |
| RN-14 | Si un `Year` no tiene ningún `Mark` `Vacation`, se aplica `DefaultVacation`: desde el 1 de julio, solo en días de `Category` `Working` (se saltan fines de semana y festivos), hasta agotar la `quota` o el año. No se guarda en el fichero y desaparece en cuanto se marca alguna `Vacation`. |
| RN-15 | Las horas se muestran con coma decimal (`8,5`) y se aceptan con coma o con punto. |
| RN-16 | El id de un `Mark` nuevo es el mayor id del fichero más uno, o 1 si no hay `Mark`. |
| RN-17 | Las horas de un `Shift` están entre 0 y 24. |
| RN-18 | Los días de la semana de la regla `ReducedWeekday` (por defecto, todos los viernes del año) son `Reduced`, o `SummerReduced` en `Summer`, con las horas de esos `Shift`: un valor para el `Summer` y otro para el resto del año. Tienen la prioridad de `Reduced` (§2.3). Solo se cambian editando `reduced_days` en el fichero; con una lista vacía no hay días reducidos por regla. |
| RN-19 | Las horas por mes (RF-14) son horas teóricas, como las de RN-09: la suma de las horas de los días laborables del mes. La suma de los 12 meses es exactamente el total anual de RF-08, en días y en horas. El acumulado de un mes es la suma desde enero hasta ese mes, así que el de diciembre es el total anual. Un mes sin días laborables muestra 0. |
| RN-20 | En las vistas, el número del día va sin color, seguido de un espacio y de un símbolo con el color del `DayKind`. Entre un día y el siguiente hay dos espacios, para que se vea a qué número pertenece cada símbolo. Si la salida tiene color, el símbolo es una barra gruesa `▌` (media celda, `15 ▌`), que deja leer mejor los números y muestra bastante color para distinguir los tonos. Sin color (al redirigir, con `NO_COLOR` o en un terminal sin color), la barra no distinguiría nada, así que en su lugar va la letra del `DayKind` (`15 F`). La leyenda, `day` y `summary` usan el mismo símbolo. |
| RN-21 | En la vista de año, los meses quedan dentro de una rejilla de líneas finas punteadas en gris, que no compite con los colores de los días: una línea vertical `┊` entre dos meses de la misma fila y a la izquierda y a la derecha, con un espacio a cada lado, y una línea horizontal `┈` entre dos filas de meses, arriba y abajo. Los cruces y las esquinas usan `┼`, `┬`, `┴`, `├`, `┤`, `┌`, `┐`, `└` y `┘`. La vista de año ocupa 130 columnas. Sin color se mantienen las mismas líneas, sin el gris. |
| RN-22 | Si la salida es un terminal, `month`, `year` y `summary` se muestran en una ventana a pantalla completa (la pantalla alternativa del terminal), con el mismo texto y colores que en la consola y una línea de ayuda abajo. `Esc`, `q` o `Ctrl+C` cierran la ventana y la consola vuelve tal como estaba, sin la vista. Si la vista no cabe, `↑` `↓` (o `k` `j`), `RePág` `AvPág` e `Inicio` `Fin` la desplazan; lo que no cabe a lo ancho se corta. Al redirigir la salida, o con `--no-tui`, la vista se imprime en la consola como hasta ahora. `day`, `list` y los comandos que cambian el fichero siempre imprimen en la consola. |
| RN-23 | La vista de año empieza con una franja, dentro del marco de la rejilla (RN-21), con el título `Calendario laboral AAAA` centrado y en negrita; una línea `┈` la separa de la primera fila de meses. Como el año ya está en la franja, cada mes se titula solo con su nombre (`Enero`). La vista de mes mantiene el año en su título (`Junio 2026`). |
| RN-24 | La vista de año termina, bajo las vacaciones restantes, con las horas laborables del año (las de RF-08) y la media mensual: esas horas divididas entre 12, aunque algún mes no tenga horas, como medida del esfuerzo mensual medio. `Horas laborables del año: 1888,5 h` y `Media mensual: 157,38 h (1888,5 h / 12 meses)`. |

## 4. Requisitos funcionales

| ID | Requisito | Prioridad |
|---|---|---|
| RF-01 | Añadir un `Year` al fichero | Must |
| RF-02 | Marcar un día o un `DateRange` con un `MarkKind` y una descripción opcional | Must |
| RF-03 | Eliminar un `Mark` | Must |
| RF-04 | Listar los `Mark`, todos o los de un `Year`, ordenados por fecha | Must |
| RF-05 | Consultar el `ResolvedDay` de una fecha: `DayKind`, horas, `Origin` y descripción | Must |
| RF-06 | Vista de mes en cuadrícula de lunes a domingo, con colores, `Mark` del mes, leyenda y vacaciones restantes | Must |
| RF-07 | Vista del `Year` completo, tres meses por fila | Should |
| RF-08 | `Summary` anual: días por `DayKind`, días y horas laborables, y vacaciones usadas y restantes | Must |
| RF-09 | Configurar las `Hours` de los cuatro `Shift` | Must |
| RF-10 | Configurar la `quota` de un `Year` | Must |
| RF-11 | Persistir el `Calendar` en JSON y cargarlo al arrancar | Must |
| RF-12 | Importar festivos desde un fichero | Could |
| RF-13 | Exportar el calendario resuelto en CSV o JSON | Could |
| RF-14 | Sumatorio mensual (`MonthTotal`): días laborables, horas laborables y horas acumuladas del año. Se muestra como tabla de 12 meses con fila de total en `summary`, como una línea en `month` y bajo cada mes en `year` (solo horas y días, por el ancho) | Must |

## 5. Interfaz de consola

El binario `calendario` funciona por subcomandos. Para construirlo y ejecutarlo, consulta `CLAUDE.md`.

| Comando | RF | Ejemplo |
|---|---|---|
| `add-year <año>` | 01 | `calendario add-year 2027` |
| `mark <tipo> <fecha> [<hasta>] [--desc <texto>]` | 02 | `calendario mark vacaciones 2026-06-05 2026-06-12` |
| `unmark <id>` | 03 | `calendario unmark 4` |
| `list [--year <año>]` | 04 | `calendario list --year 2026` |
| `day <fecha>` | 05 | `calendario day 2026-06-08` |
| `month [<año> <mes>]` | 06, 14 | `calendario month 2026 junio` |
| `year [<año>]` | 07, 14 | `calendario year 2026` |
| `summary [<año>]` | 08, 14 | `calendario summary 2026` |
| `hours <jornada> <horas>` | 09 | `calendario hours laborable-verano 8,5` |
| `quota <año> <días>` | 10 | `calendario quota 2026 25` |

- **`<tipo>`** (`MarkKind`): `reducido`, `festivo` y `vacaciones`.
- **`<jornada>`** (`Shift`): `laborable`, `reducido`, `laborable-verano` y `reducido-verano`.
- **`<mes>`** (`Month`): el nombre en español (`enero`… `diciembre`, también `setiembre`) o el número, con o sin cero delante (`6`, `06`).
- **Argumentos opcionales:** sin año, `month`, `year` y `summary` usan el año actual, y `month` también el mes actual.
- **Ventana (RN-22):** en un terminal, `month`, `year` y `summary` se abren a pantalla completa y se cierran con `Esc` o `q`. `--no-tui` los imprime en la consola.
- **Fichero:** se indica con `--file <ruta>`; si no, con la variable `CALENDARIO_FILE`; y si tampoco, se usa `./calendario.json`.
- **Salidas:**
  - Los resultados van a stdout, y los errores y avisos a stderr.
  - Códigos de salida: 0 si todo va bien, 1 si la operación falla y 2 si los argumentos no son válidos.
- **Ayuda:** `calendario --help` y `calendario <comando> --help`.

Cada día se muestra con su número, un espacio y una barra `▌` del color de su tipo (`15 ▌`). Sin color (al redirigir la salida o con `NO_COLOR`), la barra se sustituye por la letra del tipo (`15 F`, RN-20). Cada mes ocupa 40 columnas, y la vista de año, con 3 meses por fila dentro de una rejilla gris punteada con marco (RN-21) y bajo la franja con el año (RN-23), necesita 130. El ejemplo muestra la salida sin color:

```
               Junio 2026
Lu    Ma    Mi    Ju    Vi    Sá    Do
 1 L   2 L   3 L   4 L   5 V   6 S   7 S
 8 V   9 V  10 V  11 V  12 V  13 S  14 S
15 L  16 L  17 L  18 L  19 R  20 S  21 S
22 L  23 L  24 L  25 L  26 R  27 S  28 S
29 L  30 L

Marcas:
  #1   vacaciones 2026-06-05 → 2026-06-12  Viaje

Horas laborables de junio: 131,0 h en 16 días · acumulado del año: 985,5 h
Leyenda: L Laborable · R Reducido · l Laborable verano · r Reducido verano · S Fin de semana · F Festivo · V Vacaciones
Vacaciones restantes: 19 / 25
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
- **Valores de `kind`:** van en español (`reducido`, `festivo`, `vacaciones`), igual que en la CLI.

```json
{
  "version": 1,
  "weekend": ["saturday", "sunday"],
  "reduced_days": ["friday"],
  "hours": { "laborable": 8.5, "reducido": 6.0, "laborable_verano": 8.5, "reducido_verano": 6.0 },
  "years": [
    {
      "year": 2026,
      "vacation_quota": 25,
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

  Los comandos de verificación están en `CLAUDE.md`.

## 9. Decisiones

| Fecha | Decisión |
|---|---|
| 2026-11-08 | CLI por subcomandos con clap; se descarta el modo interactivo (REPL). |
| 2026-11-08 | Librerías tomadas de `../severance-pay-calculator`: `clap` 4 (derive, env), `chrono`, `thiserror` 2, más `serde`/`serde_json`. Para los colores, `anstyle` + `anstream` en lugar de `owo-colors`: ya son dependencias de clap y quitan el color al redirigir. |
| 2026-11-08 | Se elimina el `MarkKind` `laborable`: con la prioridad más baja nunca cambiaba el `DayKind`. |
| 2026-11-08 | `DefaultVacation` desde el 1 de julio en los `Year` sin `Mark` `Vacation`; se calcula y no se guarda (RN-14). |
| 2026-11-08 | Los `DateRange` que cruzan de año se rechazan; no se dividen automáticamente (RN-02). |
| 2026-11-08 | Los ids se reutilizan si se borra el `Mark` con el id más alto (RN-16). |
| 2026-11-08 | RF-12 y RF-13 quedan fuera de la v1. |
| 2026-11-08 | Documentación en español; conceptos y código en inglés (RNF-07). |
| 2026-11-08 | RN-20: símbolo de color a la derecha del número en lugar de la letra pegada (`15F`), para leer mejor los números. Se probó un punto pegado (`15●`) y se eligió una barra separada por un espacio, con dos espacios entre días (`15 ▎  16 ▎`). La letra se mantiene sin color, en el mismo sitio. La vista de año sigue con 3 meses por fila (~126 columnas). |
| 2026-11-08 | Enumeraciones antes que códigos numéricos (RNF-08): el mes es la enumeración `Month` del dominio. RN-14 empieza en `Month::July` y el `Summer` es `Month::July` y `Month::August`. La CLI acepta el mes por nombre o por número. |
| 2026-11-08 | RF-14: horas teóricas, no registradas; columnas de días, horas y acumulado; se calcula en el dominio (`Summary.months`) y se muestra en `summary`, `month` y `year`. No se muestran horas de ausencias, ni objetivo anual, ni CSV (queda para RF-13). Un mes a 0 h por las vacaciones por defecto se muestra tal cual. |
| 2026-11-08 | Todos los viernes son reducidos (RN-18). Se reutilizan los `Shift` `Reduced` y `SummerReduced` en lugar de crear jornadas nuevas, y los días son configurables en el JSON como el `Weekend`. |
| 2026-11-10 | RN-21: rejilla gris punteada entre los meses de la vista de año (`┊`, `┈` y `┼`). Se probó primero solo con líneas interiores y se añadió el marco exterior (arriba, abajo, izquierda y derecha), aunque la vista pasa de ~126 a 130 columnas. |
| 2026-11-10 | RN-20: la barra fina `▎` pasa a `▌` (media celda), porque con tan poca superficie de color se confundían el rojo de `Holiday` y el magenta de `Vacation`; se descartaron `║` (igual de fina) y `▎▎` (la vista de año pasaba a ~151 columnas). `Vacation` pasa de magenta a blanco brillante, el único color ANSI básico libre; en un terminal de fondo claro se verá poco. |
| 2026-11-10 | Colores de los `DayKind` laborables por parejas, con la jornada completa más oscura que la reducida: `Working` en azul marino (el azul ANSI, que cada tema pinta a su manera; se descartó el azul marino de la paleta de 256, que apenas se ve sobre fondo oscuro) y `Reduced` en cian; en `Summer`, `SummerWorking` en verde y `SummerReduced` en un verde pálido, más tenue (color 151 de la paleta de 256; el verde brillante ANSI resultaba más intenso que el normal), para que ningún tipo comparta color. |
| 2026-11-11 | RN-22: `month`, `year` y `summary` se ven en una ventana a pantalla completa que se cierra con `Esc`, como `severance-pay-calculator`. No es el modo interactivo descartado el 2026-11-08: solo muestra y desplaza la vista. Se abre sola si la salida es un terminal, así que redirigir y los tests no cambian, y `--no-tui` la evita. Se dibuja con `crossterm` (la versión que ya usa el workspace) a partir del mismo texto con códigos ANSI que la consola, en lugar de reescribir las vistas en ratatui; las líneas se cortan a lo ancho con `DisableLineWrap`. La navegación entre meses queda pendiente. |
| 2026-11-11 | `Vacation` pasa de blanco brillante a amarillo en negrita: el blanco apenas se distinguía, y el amarillo es el color libre más alejado del rojo de `Holiday`, también con daltonismo rojo-verde (el magenta se confundía con él). La negrita lo resalta en los terminales que la muestran más brillante. En un fondo claro el amarillo se ve poco. |
| 2026-11-11 | `Vacation` pasa de amarillo a naranja en negrita, el 208 de la paleta de 256 (como el verde pálido de `SummerReduced`), porque no hay naranja entre los 16 colores básicos. Se ve mejor que el amarillo en un fondo claro, pero está más cerca del rojo de `Holiday`. |
| 2026-11-11 | Refactorización sin cambios de comportamiento. `Month` pasa a ser `chrono::Month`, que ya es una enumeración ordenada como `Weekday` (RNF-08); el dominio solo le añade `ALL`, `of` e `is_summer` (`MonthExt`). La CLI construye sus valores (`MarkKind`, `Shift` y el mes) a partir del dominio con un único parser, en lugar de repetirlos en enums propios; el DTO JSON los mantiene aparte, porque es el formato del fichero. Las vistas de mes y de año resuelven el año una sola vez (antes, la de año lo resolvía 13 veces). |
| 2026-11-11 | RN-23: el año se repetía en el título de los 12 meses de la vista de año; ahora aparece una vez, en una franja superior `Calendario laboral AAAA` dentro del marco, y los meses llevan solo el nombre. La vista de año pasa de 44 a 46 líneas. |
| 2026-11-11 | RN-24: la vista de año termina con el total de horas del año y la media mensual (total / 12), para ver el esfuerzo mensual medio sin abrir `summary`. La media divide siempre entre 12, no entre los meses con horas. La vista de año pasa de 46 a 48 líneas. |

### Pendiente o posibles ampliaciones

- [ ] RF-12: importar festivos. Hay que definir antes el formato del fichero de entrada.
- [ ] RF-13: exportar en CSV o JSON.
- [ ] ¿Subcomandos en español (`marcar`, `anadir-anio`…), o alias? Ahora siguen los nombres de §5.
- [ ] Avisar cuando `quota` baja por debajo de las vacaciones ya usadas. RN-11 solo cubre los `Mark`.
- [ ] Los textos que genera clap («invalid value», «Possible values», «[default: …]») siguen en inglés.
- [ ] Interfaz ratatui para navegar meses, como en el proyecto hermano.
- [ ] Objetivo de horas anuales del convenio y diferencia con las horas del calendario.
- [ ] Horas de ausencias por mes (vacaciones y festivos valorados en horas).
- [ ] Forzar jornada completa en un viernes concreto: haría falta recuperar un `MarkKind` laborable con prioridad entre `Reduced` y `Vacation`.
