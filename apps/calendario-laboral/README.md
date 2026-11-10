# calendario-laboral

Aplicación de consola, `calendario`, que gestiona el calendario laboral de uno o varios años en un
fichero JSON: festivos, vacaciones y jornadas reducidas, vistas de mes y de año con colores, y un
resumen anual con días y horas laborables y el cupo de vacaciones.

## Inicio rápido

Desde la raíz del workspace (los requisitos están en su [README](../../README.md)), el alias
`cargo cal` ejecuta el programa sobre `apps/calendario-laboral/calendario.json`, que git ignora:

```bash
cargo cal add-year 2026                            # crea el fichero con el primer cambio
cargo cal mark vacaciones 2026-06-05 2026-06-12
cargo cal month 2026 junio                         # sin argumentos, el mes actual
cargo cal summary 2026
cargo cal --help                                   # y `cargo cal <comando> --help`
```

Para instalar el binario en `~/.cargo/bin`: `cargo install --path apps/calendario-laboral`.

## Uso

Los comandos, sus valores, el fichero (`--file`, `CALENDARIO_FILE` o `./calendario.json`), los
colores y los códigos de salida están en la
[§5 de los requisitos](requisitos-calendario-laboral.md#5-interfaz-de-consola), y el formato del
JSON en la [§6](requisitos-calendario-laboral.md#6-persistencia).

El alias ya pasa `--file`, así que con él `CALENDARIO_FILE` no tiene efecto y `cargo cal --file …`
falla por repetir la opción. Para otro fichero:
`cargo run -p calendario-laboral -- --file <ruta> <comando>`.

## Desarrollo

`cargo test -p calendario-laboral`. El [README](../../README.md) del workspace tiene el resto de
comandos y la CI. [requisitos-calendario-laboral.md](requisitos-calendario-laboral.md) es la fuente
de verdad de las reglas y requisitos, y [CLAUDE.md](CLAUDE.md) recoge la arquitectura, las
convenciones y el flujo para hacer cambios.
