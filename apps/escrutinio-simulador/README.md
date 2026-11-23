# escrutinio-simulador

Dispara el escrutinio: publica en Kafka, una a una, las 60.314 mesas del Congreso del 23 de julio
de 2023, cada una en su momento, a lo largo de 2 horas. Las mesas son las reales de Interior; solo el
orden y la hora de llegada son simulados. Antes de publicar comprueba que los ficheros cuadran.

Las reglas, el formato de los ficheros y el mensaje están en los
[requisitos de escrutinio](../escrutinio/docs/requisitos-escrutinio.html) (secciones «Resultados de
2023 por mesa» y «Backend»).

## Datos

Los ficheros no están en el repositorio. Descárgalos a mano (la web rechaza las descargas
automáticas) desde el
[área de descargas de infoelectoral](https://infoelectoral.interior.gob.es/es/elecciones-celebradas/area-de-descargas/):
Congreso, julio de 2023, nivel mesa. Descomprime `02202307_MESA.zip` en
`apps/escrutinio-simulador/datos/`, que git ignora:

```bash
unzip 02202307_MESA.zip -d apps/escrutinio-simulador/datos
```

## Kafka

```bash
docker compose -f compose/escrutinio.yaml up -d    # Kafka en localhost:9092 y el topic escrutinio.mesas
docker compose -f compose/escrutinio.yaml down     # pararlo
```

## Uso

Desde la raíz del workspace:

```bash
cargo esc-sim -- --solo-comprobar        # solo lee y comprueba los ficheros
cargo esc-sim                            # publica las mesas durante 2 horas
cargo esc-sim -- --duracion 5m           # en 5 minutos
```

| Opción | Por defecto | Qué hace |
|---|---|---|
| `--datos <CARPETA>` | `apps/escrutinio-simulador/datos` con `cargo esc-sim` | Carpeta con los ficheros `09`, `10`, `03` y `07` |
| `--kafka <SERVIDORES>` | `localhost:9092` | Servidores de Kafka |
| `--topic <TOPIC>` | `escrutinio.mesas` | Topic donde se publican las mesas; tiene que existir |
| `--duracion <DURACIÓN>` | `2h` | Cuánto dura el escrutinio: un número con `h`, `m` o `s` |
| `--semilla <N>` | 2023 | La misma semilla da el mismo orden de llegada |
| `--solo-comprobar` | | Solo comprueba los ficheros, sin publicar |

Muestra lo comprobado, el avance cada 10 % y, al terminar, las mesas publicadas y el tiempo. No
guarda estado: si se relanza, publica todo otra vez y el servicio descarta lo ya contado.

## Códigos de salida

0 si termina bien, 1 si fallan los ficheros, sus comprobaciones o Kafka, y 2 si los argumentos no
son válidos.
