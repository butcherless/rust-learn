# CLAUDE.md

Este fichero orienta a Claude Code cuando trabaja en el proyecto `escrutinio` de este workspace. Los comandos comunes, la CI y las convenciones están en el `CLAUDE.md` raíz (en inglés).

Maqueta en ratatui del escrutinio de las Elecciones generales España 2026 por provincia. El escrutado se simula (`mock`) hasta que exista el backend.
- `README.md` es la guía de uso.
- La fuente de verdad del comportamiento es `docs/requisitos-escrutinio.html`: conceptos, reglas (RN-xx), datos de mesas, el diseño del backend y lo pendiente. Cada regla nueva se añade allí, en español y en pocas palabras, y el `//!` del módulo que la aplica la cita.
- Las librerías y las convenciones siguen `apps/severance-pay-calculator`.

**Idiomas**, como en calendario-laboral: en español la documentación, el rustdoc, los comentarios y lo que ve el usuario; en inglés los identificadores y los nombres de los tests.

## Comandos

Desde la raíz del workspace:

```bash
cargo esc                                  # ejecutar (alias de run -p escrutinio --)
cargo test -p escrutinio                   # tests unitarios y tests/cli.rs, que ejecuta el binario
cargo llvm-cov -p escrutinio               # cobertura de este proyecto; --open para el HTML
```

`tui/mod.rs` y `main.rs` no tienen tests porque usan la terminal real: la lógica no va ahí.

## Datos

`census.rs` copia el CER por provincia de la tabla del INE que cita su `//!`. Si el INE publica otra, cambia `DISTRICTS` y `TOTAL_CENSUS` juntos: un test comprueba que la suma cuadra.

## Arquitectura

Las dependencias apuntan hacia dentro: `main → cli, tui → mock → census → domain`.

- **`domain`**: `Tally` (censo, estimado, escrutado), `Tally::total`, `sort_key`, `thousands`, `percent` y `ratio`. Sin E/S.
- **`census`**: `District`, `Territory`, `DISTRICTS` y `ordered` (RN-03).
- **`mock`**: `Rng` (xorshift, sin dependencias) y `Scrutiny`, que `start` y `advance` por valor. Cuando llegue el backend, `Scrutiny` pasará a recibir los recuentos de un puerto (trait genérico, no `dyn`).
- **`tui::app`**: The Elm Architecture: `message(key)` y `App::update(self, Message) -> (App, Effect)`. `Message::Tick` lo envía el bucle cada `--intervalo`.
- **`tui::view`**: el dibujo; solo lee `App`. `bar` es pura y se prueba sola. Los textos que comprueban varios tests (`TITLE`, `TOTAL`, `HELP`, `status_text`) son públicos.
