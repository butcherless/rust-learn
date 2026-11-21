# escrutinio

Maqueta en la terminal del escrutinio de las Elecciones generales España 2026. Muestra las 50
provincias por orden alfabético y después Ceuta y Melilla. Cada una lleva su censo, sus votos
estimados y sus votos escrutados, con una barra de colores. Por ahora el escrutado se simula.

Las reglas y los conceptos están en [docs/requisitos-escrutinio.html](docs/requisitos-escrutinio.html)
(ábrelo en el navegador).

## Uso

Desde la raíz del workspace:

```bash
cargo esc                                   # 70 % de participación estimada, un paso cada 500 ms
cargo esc -- --participacion 65             # otra participación estimada
cargo esc -- --intervalo 200 --semilla 7    # más rápido y con otro escrutinio simulado
```

| Opción | Por defecto | Qué hace |
|---|---|---|
| `--participacion <PORCENTAJE>` | 70 | Participación estimada, en % del censo (1-100) |
| `--intervalo <MS>` | 500 | Milisegundos entre dos avances de la simulación (mínimo 50) |
| `--semilla <N>` | 2026 | La misma semilla da el mismo escrutinio |

## Teclas

`↑`/`↓` (o `k`/`j`) desplazan una fila, `RePág`/`AvPág` diez, `espacio` (o `p`) pausa,
`r` reinicia, `e` cambia entre la escala proporcional y la absoluta, y `q` (o `Esc`) sale.

## Barra

`░` gris: censo · `▒` amarillo: estimado · `█` verde: escrutado · `█` cian: escrutado por encima
de lo estimado.

## Códigos de salida

0 al salir, 1 si falla la terminal y 2 si los argumentos no son válidos.
