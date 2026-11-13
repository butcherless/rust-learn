# calendario-laboral

Aplicación de consola, `calendario`, que gestiona el calendario laboral de uno o varios años en un
fichero JSON: festivos, vacaciones y jornadas reducidas, vistas de mes y de año con colores, y un
resumen anual con días y horas laborables y el cupo de vacaciones.

## Inicio rápido

Desde la raíz del workspace (lo que hay que instalar está en su [README](../../README.md#requirements)), el alias
`cargo cal` ejecuta el programa sobre `apps/calendario-laboral/calendario.json`, el calendario de uso real, que está versionado:

```bash
cargo cal add-year 2026                            # crea el fichero con el primer cambio
cargo cal mark vacaciones 2026-06-05 2026-06-12
cargo cal month 2026 junio                         # `month junio`: del año actual; sin argumentos, el mes actual
cargo cal year 2026
cargo cal summary 2026
cargo cal list 2026                                # las marcas, con su id
cargo cal edit 2 --hasta 2026-06-14                # cambia una marca; `unmark 2` la borra
cargo cal config                                   # fin de semana, jornada reducida, cupo y horas
cargo cal --help                                   # y `cargo cal <comando> --help`
```

En un terminal, `month`, `year` y `summary` se abren en una ventana a pantalla completa: `↑` `↓`,
`RePág` `AvPág` e `Inicio` `Fin` desplazan la vista si no cabe, `r` la recarga con los cambios del
fichero, y `Esc` o `q` la cierran y vuelven a la consola. Con `--no-tui`, o al redirigir la salida,
se imprimen en la consola.

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
