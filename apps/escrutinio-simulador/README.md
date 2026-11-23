# escrutinio-simulador

Dispara el escrutinio: publica en Kafka, una a una, las 60.314 mesas del Congreso del 23 de julio
de 2023, cada una en su momento, a lo largo de 2 horas. Las mesas son las reales de Interior; solo el
orden y la hora de llegada son simulados. Antes de publicar comprueba que los ficheros cuadran, y
manda al dead letter las mesas que no pasan la validación (en 2023, 4 con más votantes que censo).

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

`compose/escrutinio.yaml` levanta un Kafka 4.1.0 de un solo nodo en `localhost:9092` y crea los dos
topics: `escrutinio.mesas` (6 particiones) y su dead letter, `escrutinio.mesas.dlq` (1). Hace falta
Docker en marcha. Desde la raíz del workspace:

```bash
docker compose -f compose/escrutinio.yaml up -d      # arranca Kafka y crea los topics (unos 10 s)
docker compose -f compose/escrutinio.yaml ps -a      # kafka «healthy» y topics «Exited (0)»: listo
docker compose -f compose/escrutinio.yaml down       # lo para y borra los mensajes
```

Kafka no guarda nada al pararlo: `down` y `up` vuelven a dejar los topics vacíos, que es lo que hace
falta antes de repetir una carga.

## Lanzar la carga

Con los datos en `apps/escrutinio-simulador/datos` y Kafka arrancado:

```bash
cargo esc-sim -- --solo-comprobar        # 1. comprueba los ficheros, sin publicar
cargo esc-sim                            # 2. publica las 60.314 mesas durante 2 horas
cargo esc-sim -- --duracion 5m           #    o en 5 minutos, para probar
```

Para ver lo publicado, con las herramientas de Kafka del propio contenedor:

```bash
k() { docker compose -f compose/escrutinio.yaml exec -T kafka /opt/kafka/bin/"$@"; }
k kafka-get-offsets.sh --bootstrap-server localhost:9092 --topic escrutinio.mesas       # mensajes por partición
k kafka-get-offsets.sh --bootstrap-server localhost:9092 --topic escrutinio.mesas.dlq   # 4 al terminar
k kafka-console-consumer.sh --bootstrap-server localhost:9092 --topic escrutinio.mesas.dlq \
  --from-beginning --property print.key=true --timeout-ms 10000                           # el dead letter
```

## Opciones

| Opción | Por defecto | Qué hace |
|---|---|---|
| `--datos <CARPETA>` | `apps/escrutinio-simulador/datos` con `cargo esc-sim` | Carpeta con los ficheros `09`, `10`, `03` y `07` |
| `--kafka <SERVIDORES>` | `localhost:9092` | Servidores de Kafka |
| `--topic <TOPIC>` | `escrutinio.mesas` | Topic donde se publican las mesas; tiene que existir |
| `--topic-dlq <TOPIC_DLQ>` | `escrutinio.mesas.dlq` | Dead letter: las mesas que no pasan la validación; tiene que existir |
| `--duracion <DURACIÓN>` | `2h` | Cuánto dura el escrutinio: un número con `h`, `m` o `s` |
| `--semilla <N>` | 2023 | La misma semilla da el mismo orden de llegada |
| `--solo-comprobar` | | Solo comprueba los ficheros, sin publicar |

Muestra lo comprobado, el avance cada 10 % y, al terminar, las mesas publicadas, las del dead letter
y el tiempo. No guarda estado: si se relanza, publica todo otra vez y el servicio descarta lo ya
contado.

## Códigos de salida

0 si termina bien, 1 si fallan los ficheros, sus comprobaciones o Kafka, y 2 si los argumentos no
son válidos.
