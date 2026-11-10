# CLAUDE.md

Este fichero orienta a Claude Code (claude.ai/code) cuando trabaja con el código de este repositorio.

Aplicación de consola en Rust, `calendario`, que gestiona el `Calendar` laboral de uno o varios `Year` en un fichero JSON.
- La fuente de verdad es `requisitos-calendario-laboral.md`: reglas (RN-xx), requisitos funcionales (RF-xx), decisiones y el glosario de conceptos (§2.1).
- Las librerías y las convenciones siguen el proyecto hermano `../severance-pay-calculator`.

**Idiomas (RNF-07):**
- En español: la documentación (este fichero, los requisitos, el rustdoc `///`/`//!` y los comentarios) y todo lo que ve el usuario.
- En inglés: los conceptos y el código (identificadores y nombres de tests). En la documentación, los conceptos se nombran como en el código: `Mark`, `Holiday`, `quota`…

## Comandos

### Construir y ejecutar

```bash
cargo build                                        # build de desarrollo → target/debug/calendario
cargo build --release                              # build optimizado → target/release/calendario
cargo install --path .                             # instala `calendario` en ~/.cargo/bin
cargo run -- --help                                # ayuda del programa; `cargo run -- mark --help` para un subcomando
cargo run --release -- month                       # cualquier `cargo run`, optimizado
cargo doc --no-deps --open                         # rustdoc del crate
```

El fichero del `Calendar` es `--file <ruta>`; si no, `$CALENDARIO_FILE`; y si tampoco, `./calendario.json` (ignorado por git). Si el fichero no existe, se parte de un `Calendar` vacío y el primer cambio lo crea.

```bash
export CALENDARIO_FILE=/tmp/calendario.json        # o pasa --file en cada comando
cargo run -- add-year 2026                         # RF-01
cargo run -- mark festivo 2026-03-03 --desc "Fiesta local"           # RF-02, un día
cargo run -- mark vacaciones 2026-06-05 2026-06-12                   # RF-02, un rango
cargo run -- mark reducido 2026-12-24
cargo run -- list                                  # RF-04; --year 2026 para un año
cargo run -- unmark 1                              # RF-03, con el id que muestra `list`
cargo run -- day 2026-06-08                        # RF-05
cargo run -- month 2026 junio                      # RF-06, RF-14; o `month 2026 6`; sin argumentos, el mes actual
cargo run -- year 2026                             # RF-07; necesita 130 columnas
cargo run -- summary 2026                          # RF-08, RF-14: incluye las horas por mes y el acumulado
cargo run -- hours laborable-verano 8,5            # RF-09; con coma o punto
cargo run -- quota 2026 23                         # RF-10
cargo run -- month 2026 6 > junio.txt              # sin color al redirigir y con letras en lugar de puntos; también con NO_COLOR=1
CLICOLOR_FORCE=1 cargo run -- month 2026 6 | less -R   # fuerza el color (puntos) aunque la salida no sea un terminal
```

### Verificar

```bash
cargo test                                         # unitarios, tests/cli.rs (ejecuta el binario) y tests/traceability.rs
cargo test rn_14                                   # los tests de un requisito
cargo test --lib domain::                          # los tests de un módulo
cargo test --test cli                              # solo los que ejecutan el binario
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo llvm-cov --fail-under-lines 90               # requisito de build (RNF-06); --open para el HTML
```

No hay CI. Antes de dar un cambio por terminado, ejecuta `cargo fmt --check`, clippy y `cargo llvm-cov --fail-under-lines 90`. La cobertura está en torno al 99 %: sube el mínimo en lugar de bajarlo.

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

- **`domain`** es puro: sin E/S y sin mutación. Cada cambio recibe el `Calendar` por valor y devuelve uno nuevo: `with_year`, `with_mark`, `without_mark`, `with_quota`, `with_shift_hours` y `restore_mark`.
  - `month`: `Month`, de `January` a `December`, con `ALL`, `of(date)`, `number()` (solo para chrono) e `is_summer()`. El dominio no compara números de mes.
  - `kind`: `MarkKind` (`Reduced`, `Holiday`, `Vacation`; no existe un `Mark` laborable), `DayKind` con su letra, etiqueta, `Category` y `Shift`, y `Shift`.
  - `date`:
    - `parse_date` es ISO estricto (RN-01); chrono por sí solo acepta días de un dígito.
    - `DateRange` garantiza `from ≤ to`, y `year()` rechaza los rangos que cruzan de año.
  - `calendar`: `Calendar`, `Year`, `Mark`, `MarkId` y `Hours`.
    - `Calendar` guarda dos listas de `Weekday`: `weekend` y `reduced_days` (viernes por defecto, RN-18). Las dos se ordenan desde el lunes con `from_monday`.
    - Dos `Mark` son idénticos si tienen el mismo `MarkKind` y `DateRange`; la descripción no cuenta (RN-06).
    - El id siguiente es el máximo más uno (RN-16).
  - `resolve`:
    - El enum privado `Rule` ordena la prioridad con `Ord`: `Reduced` < `Vacation` < `Weekend` < `Holiday`.
    - Los candidatos van en orden: primero los `Mark`, luego `Weekend` y luego `ReducedWeekday`. Con la misma prioridad gana el primero, así un `Mark` `Reduced` en viernes conserva su `Origin`.
    - La regla `ReducedWeekday` da `Rule::Reduced` con `Origin::ReducedWeekday`. Reutiliza los `Shift` `Reduced` y `SummerReduced`; no hay jornadas propias de los viernes.
    - `resolve_year` añade `DefaultVacation` (RN-14) si el `Year` no tiene `Mark` `Vacation`: los primeros `quota` días de `Category` `Working` desde el 1 de julio, con `Origin::DefaultVacation`. Nunca se guarda.
    - `summarize` devuelve el `Summary`, con los 12 `MonthTotal` (RF-14): días, horas y acumulado, calculados con `scan`.
    - Las horas se suman con `fold(0.0, …)`: `Sum` de `f64` empieza en -0.0 y un mes vacío mostraría «-0,0».
  - `fixtures` (`#[cfg(test)]`): `date`, `range`, `calendar` (solo 2026) y `marked`.
- **`application`**: `execute(Calendar, Command) -> Result<Outcome, DomainError>`.
  - `Outcome::Changed` hay que guardarlo; `Unchanged` (un `Mark` idéntico) no; `Report` es el resultado de una consulta.
  - `Event::Marked.excess` lleva el aviso de RN-11.
- **`adapters`**:
  - `cli`: clap derive.
    - `MarkKindArg`, `ShiftArg` y `MonthArg` mantienen clap fuera del dominio. `MonthArg` acepta el nombre en español y el número como alias.
    - La ayuda usa la plantilla `HELP` y encabezados en español, y `display_order` en los posicionales, porque clap los ordena si no.
    - Los textos de error propios de clap siguen en inglés.
  - `render`: todo el texto de consola, en español.
    - Devuelve cadenas con códigos ANSI de `anstyle`. `main` las imprime con `anstream`, que los quita si la salida no es un terminal.
    - `Marker` (RN-20): `Bar` pinta una `▌` del color del tipo y `Letter` usa la letra. En los dos casos la celda es `NN S` (`CELL_WIDTH` 4), con el número sin color, y las celdas se separan con `CELL_GAP` (dos espacios). `main::marker` elige con `anstream::AutoStream::choice(&stdout())`: `Letter` si será `Never`.
    - Los tests de texto usan `Marker::Letter` y `strip_str`; los de color, `Marker::Bar`.
    - Cada mes ocupa `MONTH_WIDTH` = 40 columnas; la vista de año pone 3 por fila (130 columnas), dentro de una rejilla gris punteada con marco (RN-21): `┊` a los lados y entre meses, y líneas `┈` arriba, abajo y entre filas, con esquinas y cruces `┌┬┐├┼┤└┴┘`.
    - Las horas llevan coma decimal (`decimal`).
  - `json`: DTOs con serde; el dominio no conoce serde.
    - `decode` valida a través de los constructores del dominio.
    - `load` devuelve un `Calendar` vacío si el fichero no existe.
    - `weekend` y `reduced_days` son nombres de día en inglés; si faltan, toman el valor por defecto.
    - `save` escribe `<fichero>.tmp` y lo renombra.
- **`main.rs`** es la raíz de composición y el único sitio con efectos secundarios: el reloj, el fichero y la consola.
  - Códigos de salida: 1 si la operación falla y 2 si los argumentos no son válidos (lo hace clap).
  - Los avisos van a stderr.

## Convenciones

- **Enumeraciones antes que números (RNF-08):** modela los conceptos de negocio con enums (`Month`, `DayKind`, `Shift`…), nunca con códigos numéricos. Los números solo aparecen en los bordes: los alias `6`/`06` de `MonthArg` en la CLI y `Month::number` para chrono.
- Sin `unwrap`, `expect` ni `panic!` fuera de los tests, sin `unsafe`, y sin comodín `_` al hacer `match` sobre enums del dominio o de la aplicación.
- Estilo funcional: cadenas de iteradores, combinadores de `Option`/`Result` y funciones que devuelven valores nuevos. La única mutación aceptada es ordenar un `Vec` local y propio.
- Los `Display` usan `f.pad`, para que funcionen anchos como `{:<10}`.
- Tests:
  - Preferir tests con tabla de casos: un array recorrido con `for_each` y un mensaje de `assert_eq!` que nombre el caso.
  - Las vistas se comprueban sobre el texto sin color (`anstream::adapter::strip_str`); un test comprueba los códigos de color.

## Versiones de las librerías

- **clap 4.6**: features `derive` y `env`.
- **anstyle / anstream 1.0**: ya son dependencias de clap. `format!("{style}texto{style:#}")` aplica el estilo y lo cierra.
- **chrono 0.4**: `NaiveDate::iter_days` recibe `&self`, así que se mapea con un closure. Los nombres de meses y días en español son tablas en `render`, no los locales inestables de chrono.
- **serde_json**: indica los valores de `kind` desconocidos con línea y columna, y un test fija ese texto.

## IntelliJ

- Se importa abriendo la carpeta del proyecto (`Cargo.toml` en la raíz) con el plugin de Rust.
- `.idea/` no está en `.gitignore`, igual que en el proyecto hermano.
- Para ejecutar desde el IDE, crea una configuración Cargo con `run -- <subcomando>`. Conviene añadir la variable `CALENDARIO_FILE`, para no crear `calendario.json` en la raíz.

## Git

- Repositorio local en la rama `main`, todavía sin remoto; se subirá más adelante. La identidad (`user.name`, `user.email`) está configurada solo en este repositorio.
- Un commit por cambio de requisito, con el documento, los tests y el código juntos, y solo con `cargo fmt --check`, clippy y `cargo llvm-cov --fail-under-lines 90` en verde.
- No se versionan `target/`, `.idea/` ni el `calendario.json` de la raíz, que es el calendario de uso real.
