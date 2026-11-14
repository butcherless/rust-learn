# calendario-laboral

Aplicación de consola, `calendario`, que gestiona el calendario laboral de uno o varios años en un
fichero JSON: festivos, vacaciones y jornadas reducidas, vistas de mes y de año con colores, y un
resumen anual con días y horas laborables y el cupo de vacaciones.

## Inicio rápido

Desde la raíz del workspace (lo que hay que instalar está en su [README](../../README.md#requirements)), el alias
`cargo cal` ejecuta el programa sobre `apps/calendario-laboral/calendario.json`, el calendario de uso real, que está
versionado. Los comandos que lo cambian escriben en él; para probarlos, usa otro fichero (ver [Uso](#uso)).

```bash
# Años
cargo cal add-year 2026                            # crea el fichero con el primer cambio
cargo cal add-year 2027 --festivos-de 2026         # copia los festivos de 2026, con la misma fecha
cargo cal remove-year 2027 --confirmar             # `--confirmar` solo hace falta si tiene marcas

# Marcas
cargo cal mark vacaciones 2026-06-05 2026-06-12 --desc Viaje
cargo cal mark festivo 2026-06-24 --desc "San Juan" # un solo día
cargo cal list 2026                                # las marcas, con su id; sin año, todas
cargo cal edit 2 --hasta 2026-06-14                # cambia una marca y conserva su id
cargo cal unmark 2 3                               # borra una o varias marcas

# Consultas
cargo cal day 2026-06-08                           # tipo, horas y origen de un día; sin fecha, hoy
cargo cal month 2026 junio                         # `month junio`: del año actual; sin argumentos, el mes actual
cargo cal year 2026
cargo cal summary 2026

# Configuración
cargo cal config                                   # fin de semana, jornada reducida, cupo y horas
cargo cal weekend sábado domingo                   # fin de semana de todos los años
cargo cal reduced-days viernes                     # jornada reducida de todos los años
cargo cal hours 2026 laborable 8,5                 # horas de una jornada en un año
cargo cal quota 2026 23                            # cupo de vacaciones de un año
cargo cal --help                                   # y `cargo cal <comando> --help`
```

En un terminal, `month`, `year` y `summary` se abren en una ventana a pantalla completa: `↑` `↓`,
`RePág` `AvPág` e `Inicio` `Fin` desplazan la vista si no cabe, `r` la recarga con los cambios del
fichero, y `Esc` o `q` la cierran y vuelven a la consola. En la ventana, `month` y `year` muestran
también los embalses de la Comunidad de Madrid, que se piden de uno en uno a la API de
MapasDeEmbalses.es al abrirla (RN-31). Con `--no-tui`, o al redirigir la salida,
se imprimen en la consola.

Para instalar el binario en `~/.cargo/bin`: `cargo install --path apps/calendario-laboral`.

## Uso

Los comandos, sus valores, el fichero (`--file`, `CALENDARIO_FILE` o `./calendario.json`), los
colores y los códigos de salida están en la
[§5 de los requisitos](requisitos-calendario-laboral.md#5-interfaz-de-consola), y el formato del
JSON en la [§6](requisitos-calendario-laboral.md#6-persistencia).

Para otro fichero: `cargo run -p calendario-laboral -- --file <ruta> <comando>`. Con el alias,
`CALENDARIO_FILE` no tiene efecto y `--file` falla por repetido.

## Desarrollo

`cargo test -p calendario-laboral`. El [README](../../README.md) del workspace tiene el resto de
comandos y la CI. [requisitos-calendario-laboral.md](requisitos-calendario-laboral.md) es la fuente
de verdad de las reglas y requisitos, y [CLAUDE.md](CLAUDE.md) recoge la arquitectura, las
convenciones y el flujo para hacer cambios.
