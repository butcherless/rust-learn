# escrutinio

El escrutinio por provincia en la terminal. Muestra las 50 provincias por orden alfabético y después
Ceuta y Melilla, cada una con su censo, sus votos estimados y sus votos escrutados, con una barra de
colores. Sin opciones es la maqueta de las Elecciones generales España 2026, con el escrutado
simulado; con `--servicio`, sigue el escrutinio de 2023 que sirve
[escrutinio-servicio](../escrutinio-servicio).

Las reglas y los conceptos están en [docs/requisitos-escrutinio.html](docs/requisitos-escrutinio.html)
(ábrelo en el navegador).

## Uso

Desde la raíz del workspace:

```bash
cargo esc                                   # 70 % de participación estimada, un paso cada 500 ms
cargo esc --participacion 65                # otra participación estimada
cargo esc --intervalo 200 --semilla 7       # más rápido y con otro escrutinio simulado
cargo esc-seguir                            # sigue a escrutinio-servicio en http://localhost:8080
```

## Seguir el escrutinio

Con el servicio arrancado ([escrutinio-servicio](../escrutinio-servicio/README.md#arrancar)), `cargo
esc-seguir` (o `cargo esc --servicio <URL>`) pide el acumulado cada 10 s. El título pasa a 2023 y
el censo es el de 2023: la suma de las mesas de Interior. Lanza la carga con `cargo esc-sim` y verás
crecer el escrutado. Sin servicio, el estado dice «Sin conexión con el servicio», el pie muestra el
error y la ventana sigue con lo último que llegó.

| Opción | Por defecto | Qué hace |
|---|---|---|
| `--participacion <PORCENTAJE>` | 70 | Participación estimada, en % del censo (1-100) |
| `--intervalo <MS>` | 500 | Milisegundos entre dos avances de la simulación (mínimo 50) |
| `--semilla <N>` | 2026 | La misma semilla da el mismo escrutinio |
| `--servicio <URL>` | | Sigue al servicio en lugar de la maqueta; `--intervalo` y `--semilla` no se usan |

## Teclas

`↑`/`↓` (o `k`/`j`) desplazan una fila, `RePág`/`AvPág` diez, `espacio` (o `p`) pausa,
`r` reinicia, `e` cambia entre la escala proporcional y la absoluta, y `q` (o `Esc`) sale. Al seguir
al servicio no hay pausa ni reinicio.

## Barra

`░` gris: censo · `▒` ocre: estimado · `█` verde: escrutado · `█` cian: escrutado por encima
de lo estimado.

## Piloto

El `●` a la derecha de la participación dice qué provincias han cambiado en la última actualización
(cada 10 s con el servicio, cada paso en la maqueta): verde si está completa (el servicio dice que
se escrutaron todas sus mesas posibles; en la maqueta, que llegó a su resultado final), naranja si
su escrutado ha cambiado, azul claro si sigue igual y gris si aún no tiene votos. El del total es
verde cuando el servicio dice que está completo y, si no, naranja si alguna provincia lo es. Si una
petición al servicio falla, los pilotos se quedan como estaban.

Con el servicio, la columna **Mesas** muestra las mesas escrutadas de las posibles, `718/718`, por
provincia y en el total. Las rechazadas (en 2023, 4 de León, que van al dead letter) no cuentan como
posibles, así que León también se completa. Lo calcula el servicio: la TUI solo lo pinta.

## Luz de consulta

Con el servicio, el `●` de «Siguiendo el servicio», arriba a la derecha, hace un doble destello rojo
cada vez que llega una respuesta (cada 10 s): rojo, apagado, rojo y apagado en un segundo. Así ves
cuándo se ha consultado. El resto del tiempo está apagado.

## Códigos de salida

0 al salir, 1 si falla la terminal y 2 si los argumentos no son válidos. Que el servicio no responda
no es un error: la ventana lo muestra y reintenta.
