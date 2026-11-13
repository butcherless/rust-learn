# CLAUDE.md

Este fichero orienta a Claude Code (claude.ai/code) cuando trabaja en el proyecto `calendario-laboral` de este workspace. Los comandos comunes, la CI y las convenciones compartidas están en el `CLAUDE.md` raíz (en inglés); este fichero añade lo propio del proyecto.

Aplicación de consola en Rust, `calendario`, que gestiona el `Calendar` laboral de uno o varios `Year` en un fichero JSON.
- `README.md` es la guía de uso breve y remite a los requisitos.
- La fuente de verdad es `requisitos-calendario-laboral.md`: reglas (RN-xx), requisitos funcionales (RF-xx), decisiones y el glosario de conceptos (§2.1).
- Las librerías y las convenciones siguen `apps/severance-pay-calculator`. Las versiones compartidas están en `[workspace.dependencies]` del `Cargo.toml` raíz.

**Idiomas (RNF-07):**
- En español: la documentación (este fichero, los requisitos, el rustdoc `///`/`//!` y los comentarios) y todo lo que ve el usuario.
- En inglés: los conceptos y el código (identificadores y nombres de tests). En la documentación, los conceptos se nombran como en el código: `Mark`, `Holiday`, `quota`…

## Comandos

Todos los comandos se ejecutan desde la raíz del workspace. El paquete es `calendario-laboral` (para `-p`) y el binario, `calendario`.

### Construir y ejecutar

```bash
cargo build -p calendario-laboral                  # build de desarrollo → target/debug/calendario (target/ de la raíz)
cargo build --release -p calendario-laboral        # build optimizado → target/release/calendario
cargo install --path apps/calendario-laboral       # instala `calendario` en ~/.cargo/bin
cargo cal --help                                   # ayuda del programa; `cargo cal mark --help` para un subcomando
cargo run --release -p calendario-laboral -- --file apps/calendario-laboral/calendario.json month   # optimizado
cargo doc -p calendario-laboral --no-deps --open   # rustdoc del crate
```

El fichero del `Calendar` es `--file <ruta>`; si no, `$CALENDARIO_FILE`; y si tampoco, `./calendario.json`, relativo al directorio actual. Si el fichero no existe, se parte de un `Calendar` vacío y el primer cambio lo crea.

El alias `cargo cal` pasa `--file apps/calendario-laboral/calendario.json`, el calendario de uso real (versionado en git). Como `--file` gana a la variable, con el alias `CALENDARIO_FILE` no tiene efecto, y `cargo cal --file …` falla por repetir la opción. Para otro fichero, usa `cargo run -p calendario-laboral -- --file <ruta> …`.

Las consultas se pueden lanzar sobre el calendario real. En un terminal, `month`, `year` y `summary` se abren en una ventana a pantalla completa que se recarga con `r` (RN-28) y se cierra con `Esc` o `q` (RN-22); `--no-tui` las imprime en la consola:

```bash
cargo cal list                                     # RF-04; `list 2026` para un año
cargo cal day 2026-06-08                           # RF-05; sin fecha, hoy
cargo cal month 2026 junio                         # RF-06, RF-14; o `month 2026 6`; `month junio`, del año actual; sin argumentos, el mes actual
cargo cal year 2026                                # RF-07; necesita 169 columnas con la cuadrícula de vacaciones (en la ventana, lo que no cabe se corta)
cargo cal year 2026 --no-tui                       # en la consola, como antes de RN-22
cargo cal summary 2026                             # RF-08, RF-14: incluye las horas por mes y el acumulado
cargo cal config                                   # RF-15: fin de semana, jornada reducida, y cupo y horas de cada año
cargo cal month 2026 6 > junio.txt                 # al redirigir no hay ventana, ni color, y van letras en lugar de barras; sin color también con NO_COLOR=1
CLICOLOR_FORCE=1 cargo cal month 2026 6 | less -R  # fuerza el color (barras) aunque la salida no sea un terminal
```

Los cambios escriben en el fichero; para probarlos sin tocar el calendario real, usa otro:

```bash
cal() { cargo run -q -p calendario-laboral -- --file /tmp/calendario.json "$@"; }
cal add-year 2026                                  # RF-01; --festivos-de 2025 copia sus festivos (RF-19)
cal remove-year 2026 --confirmar                   # RF-20; --confirmar solo si tiene marcas
cal mark festivo 2026-03-03 --desc "Fiesta local"  # RF-02, un día
cal mark vacaciones 2026-06-05 2026-06-12          # RF-02, un rango
cal mark reducido 2026-12-24
cal edit 2 --hasta 2026-06-15 --desc Viaje         # RF-18: conserva el id; --tipo, --desde, --hasta, --desc "" la quita
cal unmark 1                                       # RF-03, con el id que muestra `list`; varios: `unmark 1 2`
cal hours 2026 laborable-verano 8,5                # RF-09, RN-25: las horas son de cada año; con coma o punto
cal quota 2026 23                                  # RF-10
cal reduced-days lunes viernes                     # RF-16; --ninguno para quitarlos
cal weekend sábado domingo                         # RF-17; también --ninguno
```

### Verificar

```bash
cargo test -p calendario-laboral                   # unitarios, tests/cli.rs (ejecuta el binario) y tests/traceability.rs
cargo test -p calendario-laboral rn_14             # los tests de un requisito
cargo test -p calendario-laboral --lib domain::    # los tests de un módulo
cargo test -p calendario-laboral --test cli        # solo los que ejecutan el binario
cargo llvm-cov -p calendario-laboral --fail-under-lines 90   # requisito de build (RNF-06); --open para el HTML
```

La CI ejecuta fmt y clippy de todo el workspace y, en un job propio de este proyecto, el build en release y el `cargo llvm-cov -p calendario-laboral --fail-under-lines 90` de arriba (RNF-06). Antes de dar un cambio por terminado, ejecuta esos mismos comandos. La cobertura está en torno al 99 %: sube el mínimo en lugar de bajarlo.

## Flujo para hacer cambios

1. Primero, actualiza `requisitos-calendario-laboral.md`: añade o edita la RN/RF, o anota la decisión en §9.
2. Escribe o ajusta los tests con su ID en el nombre: `rn_05_…`, `rf_02_…`, o `rn_09_rn_10_rf_08_…` si cubre varios.
3. Cambia el código.

`tests/traceability.rs` lee las tablas RN y RF del documento y falla si una RN, o una RF que no sea `Could`, no tiene ningún `#[test]` con su ID. Así, un requisito nuevo en el documento rompe el build hasta que tenga test.

Fechas de referencia para los casos de test:
- El 2026-01-01 es jueves, el 2026-06-01 lunes y el 2026-07-01 miércoles.
- 2026 tiene 104 días de `Weekend`, 18 de ellos en julio y agosto, y 52 viernes (`ReducedWeekday`), 9 de ellos en verano.
- Sin `Mark` `Vacation`, los 25 días de `DefaultVacation` de 2026 van del 1 de julio al 4 de agosto.

## Arquitectura

Las dependencias apuntan hacia dentro: `main → adapters → application → domain`.

- **`domain`** es puro: sin E/S y sin mutación. Cada cambio recibe el `Calendar` por valor y devuelve uno nuevo: `with_year`, `without_year`, `with_mark`, `with_replaced_mark`, `without_mark`, `with_quota`, `with_shift_hours`, `with_year_hours`, `with_weekend`, `with_reduced_days` y `restore_mark`.
  - `month`: `Month` es `chrono::Month`; el trait `MonthExt` le añade `ALL`, `of(date)` e `is_summer()`. El dominio no compara números de mes (`number_from_month` solo para construir fechas).
  - `kind`: `MarkKind` (`Working`, `Reduced`, `Holiday`, `Vacation`; `Working` fuerza la jornada completa, RN-26), `DayKind` con su letra, etiqueta, `Category` y `Shift`, y `Shift`.
  - `date`:
    - `parse_date` es ISO estricto (RN-01); chrono por sí solo acepta días de un dígito.
    - `DateRange` garantiza `from ≤ to`, y `year()` rechaza los rangos que cruzan de año.
  - `hours`: `Hours`, las horas de los cuatro `Shift`, entre 0 y 24 (RN-17).
  - `calendar`: `Calendar`, `Year`, `Mark` y `MarkId`.
    - Las `Hours` son de cada `Year` (RN-25). `with_year` las hereda con `inherited_hours`: del año anterior más cercano, si no del siguiente, y si no las de por defecto.
    - `Calendar` guarda dos listas de `Weekday`: `weekend` y `reduced_days` (viernes por defecto, RN-18; se cambian con `weekend` y `reduced-days`, RF-17 y RF-16). Las dos se ordenan desde el lunes con `from_monday`.
    - Dos `Mark` son idénticos si tienen el mismo `MarkKind` y `DateRange`; la descripción no cuenta (RN-06).
    - El id siguiente es el máximo más uno (RN-16).
  - `resolve`:
    - El enum privado `Rule` ordena la prioridad con `Ord`: `Reduced` < `Working` (solo de un `Mark`, RN-26) < `Vacation` < `Weekend` < `Holiday`.
    - Los candidatos van en orden: primero los `Mark`, luego `Weekend` y luego `ReducedWeekday`. Con la misma prioridad gana el primero, así un `Mark` `Reduced` en viernes conserva su `Origin`.
    - La regla `ReducedWeekday` da `Rule::Reduced` con `Origin::ReducedWeekday`. Reutiliza los `Shift` `Reduced` y `SummerReduced`; no hay jornadas propias de los viernes.
    - `resolve_year` añade `DefaultVacation` (RN-14) si el `Year` no tiene `Mark` `Vacation`: los primeros `quota` días de `Category` `Working` desde el 1 de julio, con `Origin::DefaultVacation`. Nunca se guarda.
    - `summarize` devuelve el `Summary`, con los 12 `MonthTotal` (RF-14): días, horas y acumulado, calculados con `scan`. `summary_of` lo calcula a partir de días ya resueltos, para que las vistas de mes y de año resuelvan el año una sola vez.
    - Las horas se suman con `fold(0.0, …)`: `Sum` de `f64` empieza en -0.0 y un mes vacío mostraría «-0,0».
  - `fixtures` (`#[cfg(test)]`): `date`, `range`, `calendar` (solo 2026) y `marked`.
- **`application`**: `execute(Calendar, Command) -> Result<Outcome, DomainError>`. `mod.rs` tiene los tipos, `execute` (solo reparte cada `Command`) y los tests; los casos de uso están en `marks.rs`, `years.rs`, `settings.rs` y `views.rs`.
  - `Outcome::Changed` hay que guardarlo; `Unchanged` (un `Mark` idéntico) no; `Report` es el resultado de una consulta (`Report::Config` lleva el `Calendar`).
  - Los comandos de varios pasos (`unmark` con varios ids, `add-year --festivos-de`, `edit`) encadenan los cambios con `try_fold`: si uno falla, falla todo y no hay nada que guardar (RN-13).
  - `excess` en `Event::Marked`, `Edited` y `QuotaSet` lleva el aviso de RN-11.
  - `remove-year` con `Mark` necesita `confirmed`; si no, devuelve `DomainError::YearHasMarks`.
- **`adapters`**:
  - `cli`: clap derive. `mod.rs` define los argumentos y los convierte en `Command`; `values.rs`, los parsers de valores; y `errors.rs`, los errores y la ayuda en español.
    - `choice` es el único parser de valores cerrados: construye los `PossibleValue` de clap a partir del dominio (`MarkKind::ALL` y `name()`, `Shift::ALL`, y `Month::ALL` con `render::month_name`), así el dominio no depende de clap y los nombres no se repiten. El mes acepta el nombre en español y el número (`6`, `06`) como alias, y los días de la semana el nombre con o sin tilde (`weekday_arg`).
    - `WeekdaysArg` son los días de `weekend` y `reduced-days`, con `--ninguno` para una lista vacía. `month` usa `allow_missing_positional`, así un solo argumento es el mes.
    - La ayuda usa la plantilla `HELP` y encabezados en español, y `display_order` en los posicionales, porque clap los ordena si no.
    - `error_text` da en español los errores de argumentos de clap, montados a partir de su `ErrorKind` y su contexto, y la ayuda, con `spanish_clap` sobre el texto de clap (`[valores posibles: …]`, `[OPCIONES]`…). La versión y los errores poco habituales los muestra clap.
  - `render`: todo el texto de consola, en español. `mod.rs` tiene la API (`report`, `event`, `Marker`) y los textos compartidos, y sus tests; `style.rs`, los colores; `calendar.rs`, las vistas de mes y año; `vacation_grid.rs`, la cuadrícula de vacaciones de las vistas de mes y de año; y `text.rs`, `summary`, `day`, `config` y los mensajes de eventos. Los submódulos usan `use super::*`.
    - Devuelve cadenas con códigos ANSI de `anstyle`. `main` las imprime con `anstream`, que los quita si la salida no es un terminal.
    - `Marker` (RN-20): `Bar` pinta una `▌` del color del tipo y `Letter` usa la letra. En los dos casos la celda es `NN S` (`CELL_WIDTH` 4), con el número sin color, y las celdas se separan con `CELL_GAP` (dos espacios). `main::color` mira `anstream::AutoStream::choice(&stdout())` y `Marker::for_color` elige `Letter` si no habrá color.
    - Los tests de texto usan `Marker::Letter` y `strip_str`; los de color, `Marker::Bar`.
    - Cada mes ocupa `MONTH_WIDTH` = 40 columnas; la vista de año pone 3 por fila (130 columnas), dentro de una rejilla gris punteada con marco (RN-21): `┊` a los lados y entre meses, y líneas `┈` arriba, abajo y entre filas, con esquinas y cruces `┌┬┐├┼┤└┴┘`. Arriba, dentro del marco, la franja `Calendario laboral AAAA` en negrita (RN-23); por eso `month_grid` recibe el título: solo el nombre en la vista de año y `Junio 2026` en la de mes.
    - La vista de año termina con las horas del año y la media mensual, `Summary::monthly_average_hours` (total / 12) y, entre paréntesis, `monthly_average_days` (media / `REFERENCE_DAY_HOURS`, 8 h fijas) (RN-24).
    - `vacation_grid` (RN-27): un cuadro `██` por día del cupo (o por día usado, si pasan del cupo; como mucho 366), cinco por fila y primero los usados, con la leyenda a la derecha centrada en vertical. Usados son `Summary::vacation_used`, todos los días de vacaciones del año aunque aún no hayan llegado, así que no depende de la fecha. Sin color, los restantes son `░░`. `beside_vacations` la pone a la derecha del calendario: en `month`, desde el título, alargando el mes con semanas en blanco si hace falta; en `year`, desde el título de la primera fila de meses (169 columnas en total).
    - Las horas llevan coma decimal (`decimal`).
  - `viewer` (RN-22): la ventana a pantalla completa, con The Elm Architecture como en `severance-pay-calculator`. `r` da `Effect::Reload` (RN-28): `run` llama a la función que recibe, y `Viewer::reloaded` cambia el texto manteniendo la línea, o guarda el error para la línea de ayuda.
    - `opens(report, output)` decide si se abre: solo `Month`, `Year` y `Summary`, y solo con `Output::Window`. `Args::output(terminal)` elige una vez entre `Window` y `Console`: la ventana solo en un terminal y sin `--no-tui`.
    - `Viewer` guarda las líneas del texto de `render` (con sus códigos ANSI) y la primera visible. `message(key)` da el `Message` y `Viewer::update(self, Message, height)` el estado siguiente y el `Effect`; `height` llega en cada llamada porque el terminal cambia de tamaño, y `top` se ajusta para no dejar hueco abajo.
    - `run` es lo único con efectos: modo raw, pantalla alternativa y `DisableLineWrap` (lo que no cabe a lo ancho se corta), y lo restaura todo también si falla. `draw` escribe en cualquier `Write`, así que se testea sobre un `Vec<u8>`.
  - `json`: DTOs con serde; el dominio no conoce serde. `KindDto` repite a propósito los nombres de `MarkKind`, porque son el formato del fichero.
    - `decode` valida a través de los constructores del dominio.
    - `load` devuelve un `Calendar` vacío si el fichero no existe.
    - `weekend` y `reduced_days` son nombres de día en inglés; si faltan, toman el valor por defecto.
    - `save` escribe `<fichero>.tmp` y lo renombra.
    - Escribe la versión 2 (`VERSION`), con `hours` en cada año. Lee también la 1: sus `hours` de la raíz se aplican a los años sin las suyas, y el primer cambio guarda el fichero en la versión 2.
- **`main.rs`** es la raíz de composición y, con `viewer::run`, el único sitio con efectos secundarios: el reloj, el fichero y la consola.
  - `present` muestra un `Report` en la ventana si `viewer::opens` lo decide (con el `Output` que `main` calcula a partir de `stdout().is_terminal()`), y si no lo imprime con anstream. Con `NO_COLOR`, quita los códigos de color antes de pasarlo a la ventana. Para la recarga de RN-28 le pasa `query`, que vuelve a leer el fichero y ejecuta otra vez el mismo `Command`.
  - Códigos de salida: 1 si la operación falla y 2 si los argumentos no son válidos (`argument_error`, con `Args::try_parse`).
  - Los avisos van a stderr.

## Convenciones

Las reglas comunes (sin `unwrap`/`expect`/`panic!`, estilo funcional, `thiserror`, tests con tabla de casos…) están en el `CLAUDE.md` raíz. En este proyecto, además:

- **Enumeraciones antes que números (RNF-08):** modela los conceptos de negocio con enums (`Month`, `DayKind`, `Shift`…), nunca con códigos numéricos. Los números solo aparecen en los bordes: los alias `6`/`06` del mes en la CLI y `number_from_month` para construir fechas con chrono.
- Sin `unsafe`, y sin comodín `_` al hacer `match` sobre enums de la aplicación tampoco.
- La única mutación aceptada es ordenar un `Vec` local y propio.
- Los `Display` usan `f.pad`, para que funcionen anchos como `{:<10}`.
- Las vistas se comprueban sobre el texto sin color (`anstream::adapter::strip_str`); un test comprueba los códigos de color.

## Versiones de las librerías

- **clap 4.6**: features `derive`, `env` y `string` (los nombres y alias de mes se generan en tiempo de ejecución).
- **anstyle / anstream 1.0**: ya son dependencias de clap. `format!("{style}texto{style:#}")` aplica el estilo y lo cierra.
- **crossterm 0.29**: la del workspace, la misma que usa ratatui. `Event::as_key_press_event` filtra las pulsaciones; sin él, en Windows llegarían también las teclas soltadas.
- **chrono 0.4**: `NaiveDate::iter_days` recibe `&self`, así que se mapea con un closure. Los nombres de meses y días en español son tablas en `render` (`month_name`, `weekday_name`), que también usa la CLI, no los locales inestables de chrono.
- **serde_json**: indica los valores de `kind` desconocidos con línea y columna, y un test fija ese texto.

## IntelliJ

- Cómo abrir el workspace y crear configuraciones de ejecución: README raíz, apartado IntelliJ IDEA.
- En las configuraciones de este proyecto pasa siempre `--file` (o la variable `CALENDARIO_FILE`); si no, se crea un `calendario.json` en la raíz del workspace.

## Git

- Las reglas de Git son las del `CLAUDE.md` raíz: se trabaja y se sube directamente en `main`.
- Un commit por cambio de requisito, con el documento, los tests y el código juntos, y solo con fmt, clippy y la cobertura en verde.
- `apps/calendario-laboral/calendario.json`, el calendario de uso real, está versionado: cada cambio con `cargo cal` se sube con un commit. Los tests nunca lo usan (trabajan sobre ficheros temporales).
