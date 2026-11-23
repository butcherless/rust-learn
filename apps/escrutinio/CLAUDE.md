# CLAUDE.md

Este fichero orienta a Claude Code cuando trabaja en el proyecto `escrutinio` de este workspace. Los comandos comunes, la CI y las convenciones están en el `CLAUDE.md` raíz (en inglés).

TUI en ratatui del escrutinio por provincia: la maqueta de 2026, con el escrutado simulado (`mock`), o, con `--servicio`, el escrutinio de 2023 que sirve `escrutinio-servicio` (`feed`). Es la tercera app del backend del escrutinio, con el simulador y el servicio.
- `README.md` es la guía de uso.
- La fuente de verdad del comportamiento es `docs/requisitos-escrutinio.html`: conceptos, reglas (RN-xx), datos de mesas, el diseño del backend y lo pendiente. Cada regla nueva se añade allí, en español y en pocas palabras, y el `//!` del módulo que la aplica la cita.
- Las librerías y las convenciones siguen `apps/severance-pay-calculator`.

**Idiomas**, como en calendario-laboral: en español la documentación, el rustdoc, los comentarios y lo que ve el usuario; en inglés los identificadores y los nombres de los tests.

## Comandos

Desde la raíz del workspace:

```bash
cargo esc                                  # la maqueta (alias de run -p escrutinio --)
cargo esc-seguir                           # sigue al servicio en http://localhost:8080
cargo test -p escrutinio                   # tests unitarios y tests/cli.rs, que ejecuta el binario
cargo llvm-cov -p escrutinio               # cobertura de este proyecto; --open para el HTML
```

`tui/mod.rs` y `main.rs` no tienen tests porque usan la terminal real: la lógica no va ahí.

## Datos

`census.rs` tiene, por circunscripción, el código de provincia del INE y dos censos CER: el de 2026, de la tabla del INE que cita su `//!`, y el de 2023, de las mesas de Interior (RN-10). Si cambia uno, cambia `DISTRICTS` y su total (`TOTAL_CENSUS` o `TOTAL_CENSUS_2023`) juntos: un test comprueba que la suma cuadra. `ordered(Election)` da las filas con el censo de una elección.

## Arquitectura

Las dependencias apuntan hacia dentro: `main → cli, tui, feed → mock → census → domain`.

- **`domain`**: `Tally` (censo, estimado, escrutado), `Tally::total`, `sort_key`, `thousands`, `percent` y `ratio`. Sin E/S.
- **`census`**: `District`, `Territory`, `DISTRICTS` y `ordered` (RN-03).
- **`mock`**: `Rng` (xorshift, sin dependencias) y `Scrutiny`, que `start`, `advance` (la maqueta) y `with_counted` (el servicio) por valor.
- **`feed`** (RN-11): el cliente bloqueante de reqwest, `fetch` de `GET /snapshots` y `counted` (votantes por código de provincia). Los tipos son de `libs/escrutinio-contrato`. Se prueba con wiremock dentro de `spawn_blocking`, como el `feed` de calendario-laboral.
- **`tui::app`**: The Elm Architecture: `message(key)` y `App::update(self, Message) -> (App, Effect)`. `Source` dice si la app sigue la maqueta o el servicio. Cada `interval`, el bucle de `tui::run` envía el mensaje de su `on_tick`: `Message::Tick` con la maqueta, cada `--intervalo`, o `Message::Snapshots` con el servicio, cada 10 s.
- **`tui::view`**: el dibujo; solo lee `App`. `bar` es pura y se prueba sola. Los textos que comprueban varios tests (`title`, `TOTAL`, `HELP`, `HELP_SERVICE`, `status_text`) son públicos.
