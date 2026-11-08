# Calendario laboral — Requisitos

Aplicación de consola en Rust para gestionar el calendario laboral de uno o varios años. Todo día es `Working` salvo que un `Mark` o una regla diga otra cosa.

| | |
|---|---|
| **Fecha** | 2026-11-08 |
| **Estado** | v1 implementada: RF-01 a RF-11, RF-14 y RN-01 a RN-19 |
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
| `DayKind` | Tipo efectivo de un día (§2.2) | nombre, letra y color |
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
| `Working` | `Working` | Por defecto fuera del `Summer` | `Working` (8,5 h) | Laborable | L | Verde |
| `Reduced` | `Working` | `Mark` `Reduced` o regla `ReducedWeekday`, fuera del `Summer` | `Reduced` (6,0 h) | Reducido | R | Amarillo |
| `SummerWorking` | `Working` | Por defecto en `Summer` | `SummerWorking` (8,5 h) | Laborable verano | l | Cian |
| `SummerReduced` | `Working` | `Mark` `Reduced` o regla `ReducedWeekday`, en `Summer` | `SummerReduced` (6,0 h) | Reducido verano | r | Azul |
| `Weekend` | `NonWorking` | Regla `Weekend` | — (0 h) | Fin de semana | S | Gris |
| `Holiday` | `Absence` | `Mark` `Holiday` | — (0 h) | Festivo | F | Rojo |
| `Vacation` | `Absence` | `Mark` `Vacation` o `DefaultVacation`; consume `quota` | — (0 h) | Vacaciones | V | Magenta |

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
- **Fichero:** se indica con `--file <ruta>`; si no, con la variable `CALENDARIO_FILE`; y si tampoco, se usa `./calendario.json`.
- **Salidas:**
  - Los resultados van a stdout, y los errores y avisos a stderr.
  - Códigos de salida: 0 si todo va bien, 1 si la operación falla y 2 si los argumentos no son válidos.
- **Ayuda:** `calendario --help` y `calendario <comando> --help`.

Cada día se muestra con su color y también con su letra, para que se lea igual sin color. Los colores se quitan solos al redirigir la salida o con `NO_COLOR`. Cada mes ocupa 27 columnas, y la vista de año necesita unas 90.

```
        Junio 2026
Lu  Ma  Mi  Ju  Vi  Sá  Do
 1L  2L  3L  4L  5V  6S  7S
 8V  9V 10V 11V 12V 13S 14S
15L 16L 17L 18L 19R 20S 21S
22L 23L 24L 25L 26R 27S 28S
29L 30L

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
| RNF-06 | Tener tests para cada regla RN y cada caso de uso RF, con una cobertura de líneas **≥ 90 %** medida con `cargo llvm-cov --fail-under-lines 90`. |
| RNF-07 | **Idiomas.** En inglés: los conceptos y el código (identificadores, nombres de tests). En español: la documentación (este documento, `CLAUDE.md`, rustdoc `///`/`//!` y comentarios) y todo lo que ve el usuario (mensajes, ayuda y valores de la CLI y del JSON). Los subcomandos se llaman como en §5. |
| RNF-08 | **Enumeraciones de negocio.** Siempre que sea posible, el código usa enumeraciones en lugar de códigos numéricos: `Month` en lugar de 1–12, `DayKind`, `MarkKind`, `Shift`, `Category`, `Origin` o `Weekday`. Los números solo aparecen en los bordes (la entrada de la CLI y las fechas de chrono). |

## 8. Verificación y trazabilidad

Cada requisito tiene al menos un test que actúa como regresión ante cambios futuros.

- **Nombres:** el test lleva en el nombre el ID que comprueba: `rn_05_…`, `rf_02_…`, o `rn_09_rn_10_rf_08_…` si comprueba varios. `cargo test rn_14` ejecuta los de una regla.
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
| 2026-11-08 | Enumeraciones antes que códigos numéricos (RNF-08): el mes es la enumeración `Month` del dominio. RN-14 empieza en `Month::July` y el `Summer` es `Month::July` y `Month::August`. La CLI acepta el mes por nombre o por número. |
| 2026-11-08 | RF-14: horas teóricas, no registradas; columnas de días, horas y acumulado; se calcula en el dominio (`Summary.months`) y se muestra en `summary`, `month` y `year`. No se muestran horas de ausencias, ni objetivo anual, ni CSV (queda para RF-13). Un mes a 0 h por las vacaciones por defecto se muestra tal cual. |
| 2026-11-08 | Todos los viernes son reducidos (RN-18). Se reutilizan los `Shift` `Reduced` y `SummerReduced` en lugar de crear jornadas nuevas, y los días son configurables en el JSON como el `Weekend`. |

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
