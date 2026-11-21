# CLAUDE.md

Este fichero orienta a Claude Code (claude.ai/code) cuando trabaja en el proyecto `calendario-laboral` de este workspace. Los comandos comunes, la CI y las convenciones compartidas están en el `CLAUDE.md` raíz (en inglés); este fichero añade lo propio del proyecto.

Aplicación de consola en Rust, `calendario`, que gestiona el `Calendar` laboral de uno o varios `Year` en un fichero JSON.
- `README.md` es la guía de uso breve y remite a los requisitos.
- La fuente de verdad es `requisitos-calendario-laboral.md`: reglas (RN-xx), requisitos funcionales (RF-xx), decisiones y el glosario de conceptos (§2.1).
- Las librerías y las convenciones siguen `apps/severance-pay-calculator`.

**Idiomas (RNF-07):**
- En español: la documentación (este fichero, los requisitos, el rustdoc `///`/`//!` y los comentarios) y todo lo que ve el usuario.
- En inglés: los conceptos y el código (identificadores y nombres de tests). En la documentación, los conceptos se nombran como en el código: `Mark`, `Holiday`, `quota`…

## Comandos

Desde la raíz del workspace. El paquete es `calendario-laboral` (para `-p`) y el binario, `calendario`. El README tiene un ejemplo de cada comando, y la §5 de los requisitos, su sintaxis y su RF.

```bash
cargo build -p calendario-laboral                  # → target/debug/calendario
cargo doc -p calendario-laboral --no-deps --open   # rustdoc del crate
cargo cal month 2026 6 > junio.txt                 # al redirigir no hay ventana ni color, y van letras en lugar de barras; igual con NO_COLOR=1
CLICOLOR_FORCE=1 cargo cal month 2026 6 | less -R  # fuerza el color aunque la salida no sea un terminal
```

`cargo cal` trabaja sobre el calendario real (versionado). Para probar cambios, usa otro fichero:

```bash
cal() { cargo run -q -p calendario-laboral -- --file /tmp/calendario.json "$@"; }
```

### Verificar

```bash
cargo test -p calendario-laboral                   # unitarios, tests/cli.rs (ejecuta el binario) y tests/traceability.rs
cargo test -p calendario-laboral rn_14             # los tests de un requisito
cargo test -p calendario-laboral --lib domain::    # los tests de un módulo
cargo test -p calendario-laboral --test cli        # solo los que ejecutan el binario
cargo llvm-cov -p calendario-laboral --fail-under-lines 90   # lo que exige la CI (RNF-06); --open para el HTML
```

Antes de dar un cambio por terminado, pasa fmt, clippy y la cobertura. Ronda el 98 %.

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

Las dependencias apuntan hacia dentro: `main → adapters → application → domain`. El detalle de cada módulo (tipos, decisiones de implementación, qué regla cumple cada función) está en su rustdoc `//!`: léelo antes de cambiarlo, o míralo con `cargo doc -p calendario-laboral --no-deps --open`. Al cambiar un módulo, actualiza su `//!`; este mapa solo cambia si se añade, quita o mueve un módulo.

- **`domain`**, puro: sin E/S y sin mutación; cada cambio recibe el `Calendar` por valor y devuelve uno nuevo.
  - `month` (`Month`, la enumeración propia), `kind` (`MarkKind`, `DayKind`, `Category`, `Shift`), `date` (`parse_date`, `DateRange`), `hours`, `calendar` (`Calendar`, `Year`, `Mark`), `error`.
  - `resolve`: el `DayKind` de cada día por prioridad de reglas, `DefaultVacation` (RN-14) y el `Summary`.
  - `absence` (RN-30), `reservoir` (RN-31) y `market` (RN-32).
  - `fixtures` (`#[cfg(test)]`): `date`, `range`, `today` (el 2026-06-01, para las vistas), `calendar` (solo 2026) y `marked`.
- **`application`**: `execute(Calendar, Command) -> Result<Outcome, DomainError>`, con los casos de uso en `marks`, `years`, `settings` y `views`.
- **`adapters`**:
  - `cli`: clap derive (`mod.rs`), los parsers de valores (`values.rs`) y los errores y la ayuda en español (`errors.rs`).
  - `render`: todo el texto de consola, con códigos ANSI de `anstyle`. `calendar.rs` (mes y año), `vacation_grid.rs` (RN-27), `next_absence.rs` (RN-30), `reservoirs.rs` (RN-31), `markets.rs` (RN-32), `text.rs` (`summary`, `day`, `config` y los eventos) y `style.rs` (colores).
  - `viewer` (RN-22): la ventana a pantalla completa, con The Elm Architecture como en `severance-pay-calculator`.
  - `feed`, `reservoirs`, `markets` y `panel` (RN-31, RN-32): el cliente HTTP bloqueante, los dos puertos con sus clientes y tablas, y el hilo que hace las peticiones de una en una.
  - `json`: el fichero (DTOs con serde, versiones 1 y 2).
- **`main.rs`**: la raíz de composición y, con `viewer::run`, el único sitio con efectos (reloj, fichero, consola y red).

## Convenciones

Las reglas comunes (sin `unwrap`/`expect`/`panic!`, estilo funcional, `thiserror`, tests con tabla de casos…) están en el `CLAUDE.md` raíz. En este proyecto, además:

- **Enumeraciones antes que números (RNF-08):** modela los conceptos de negocio con enums (`Month`, `DayKind`, `Shift`…), nunca con códigos numéricos. Los números solo aparecen en los bordes: los alias `6`/`06` del mes en la CLI y `Month::number` para construir fechas con jiff.
- Sin `unsafe`, y sin comodín `_` al hacer `match` sobre enums de la aplicación tampoco.
- La única mutación aceptada es ordenar un `Vec` local y propio.
- Los `Display` usan `f.pad`, para que funcionen anchos como `{:<10}`.
- Las vistas se comprueban sobre el texto sin color (`anstream::adapter::strip_str`); un test comprueba los códigos de color.

## Versiones de las librerías

- **clap 4.6**: features `derive`, `env` y `string` (los nombres y alias de mes se generan en tiempo de ejecución).
- **anstyle / anstream 1.0**: ya son dependencias de clap. `format!("{style}texto{style:#}")` aplica el estilo y lo cierra.
- **crossterm 0.29**: la del workspace, la misma que usa ratatui. `Event::as_key_press_event` filtra las pulsaciones; sin él, en Windows llegarían también las teclas soltadas.
- **jiff 1.0** (`civil::Date`, `Time` y `Weekday`), en lugar de chrono, que ya no se mantiene (RUSTSEC-2026-0543). Los años son `i16`, el tipo de jiff, que cubre el rango 1–9999 que admite `check_year`. jiff no tiene enumeración de meses, así que `Month` es propia. `series(1.day())` recorre los días, `last_of_month` da el último del mes y `with().year(año).build()` falla con el 29 de febrero de un año no bisiesto, como el `with_year` de chrono. `parse_date` sigue comprobando los dígitos antes de `FromStr`. Los nombres de meses y días en español son tablas en `render` (`month_name`, `weekday_name`), que también usa la CLI.
- **reqwest 0.13**: la del workspace con la feature `blocking`, porque la aplicación no es async. El cliente bloqueante no puede crearse ni usarse dentro de un runtime: los tests con **wiremock** (que sí es async, con **tokio** como dev-dependency) lo llaman dentro de `tokio::task::spawn_blocking`. Como en pr-checker, la URL se monta con `format!` (`RequestBuilder::query` necesita otra feature).
- **serde_json**: indica los valores de `kind` desconocidos con línea y columna, y un test fija ese texto.

## IntelliJ

En las configuraciones de ejecución (README raíz) pasa siempre `--file` o `CALENDARIO_FILE`; si no, se crea un `calendario.json` en la raíz del workspace.

## Git

- Un commit por cambio de requisito, con el documento, los tests y el código juntos, y solo con fmt, clippy y la cobertura en verde.
- `apps/calendario-laboral/calendario.json`, el calendario de uso real, está versionado: cada cambio con `cargo cal` se sube con un commit. Los tests nunca lo usan (trabajan sobre ficheros temporales).
