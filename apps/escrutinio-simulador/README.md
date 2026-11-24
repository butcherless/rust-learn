# escrutinio-simulador

Dispara el escrutinio: publica en Kafka, una a una, las 60.314 mesas del Congreso del 23 de julio
de 2023, cada una en su momento, a lo largo de 2 horas. Las mesas son las reales de Interior; solo el
orden y la hora de llegada son simulados. Antes de publicar comprueba que los ficheros cuadran.
Publica todas las mesas: validarlas y apartar al dead letter las que no valen (en 2023, 4 con más
votantes que censo) es cosa del [servicio](../escrutinio-servicio/README.md).

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

Antes de repetir una carga, vacía los topics para que no se acumulen los escrutinios: `cargo
esc-borrar` (de [escrutinio-servicio](../escrutinio-servicio/README.md#borrar-el-escrutinio)) borra sus
mensajes y las tablas del servicio. `down` y `up` también los dejan vacíos, porque Kafka no guarda nada
al pararlo.

## Lanzar la carga

Con los datos en `apps/escrutinio-simulador/datos` y Kafka arrancado:

```bash
cargo esc-sim --solo-comprobar           # 1. comprueba los ficheros, sin publicar
cargo esc-sim                            # 2. publica las 60.314 mesas durante 2 horas
cargo esc-sim --duracion 5m              #    o en 5 minutos, para probar
```

Para ver lo publicado en el navegador, usa el [visor web](#visor-web-de-kafka). Desde la terminal,
con las herramientas de Kafka del propio contenedor:

```bash
k() { docker compose -f compose/escrutinio.yaml exec -T kafka /opt/kafka/bin/"$@"; }
k kafka-get-offsets.sh --bootstrap-server localhost:9092 --topic escrutinio.mesas       # mensajes por partición
k kafka-get-offsets.sh --bootstrap-server localhost:9092 --topic escrutinio.mesas.dlq   # 4 al terminar
k kafka-console-consumer.sh --bootstrap-server localhost:9092 --topic escrutinio.mesas.dlq \
  --from-beginning --property print.key=true --timeout-ms 10000                           # el dead letter
```

## Visor web de Kafka

El compose trae un visor opcional, [Kafbat UI](https://github.com/kafbat/kafka-ui), para seguir los
mensajes en el navegador. Solo arranca con el perfil `visor`:

```bash
docker compose -f compose/escrutinio.yaml --profile visor up -d   # Kafka, PostgreSQL y el visor
```

La primera vez recrea el contenedor de Kafka (que publica el puerto del visor) y se pierden sus
mensajes: hazlo antes de lanzar la carga, no durante. Después, abre **http://localhost:8081**:

- **Topics → `escrutinio.mesas` → Messages:** las mesas publicadas, las 60.314, con su clave (el código de
  provincia) y el JSON. Puedes empezar por las más antiguas o las más nuevas, seguirlas en directo
  mientras corre el simulador y filtrar por clave, por ejemplo `28` para Madrid.
- **Topics → `escrutinio.mesas.dlq` → Messages:** las mesas que el servicio rechaza, con su motivo
  (`reason`) y su texto (`detail`). En 2023 son 4, de León.
- **Consumers → `escrutinio-servicio`:** el retraso (*lag*) del servicio en cada partición, es decir,
  las mesas publicadas que aún no ha registrado. Con el servicio al día, 0.

El visor solo lee: no publica ni borra mensajes. Para vaciar los topics, usa `cargo esc-borrar`.

```bash
docker compose -f compose/escrutinio.yaml --profile visor down    # lo para todo, también el visor
```

Para pararlo, usa también `--profile visor`, para que `down` incluya el visor junto a Kafka, cuya red
comparte.

## Opciones

| Opción | Por defecto | Qué hace |
|---|---|---|
| `--datos <CARPETA>` | `apps/escrutinio-simulador/datos` con `cargo esc-sim` | Carpeta con los ficheros `09`, `10`, `03` y `07` |
| `--kafka <SERVIDORES>` | `localhost:9092` | Servidores de Kafka |
| `--topic <TOPIC>` | `escrutinio.mesas` | Topic donde se publican las mesas; tiene que existir |
| `--duracion <DURACIÓN>` | `2h` | Cuánto dura el escrutinio: un número con `h`, `m` o `s` |
| `--semilla <N>` | 2023 | La misma semilla da el mismo orden y los mismos tiempos de llegada |
| `--solo-comprobar` | | Solo comprueba los ficheros, sin publicar |

Muestra lo comprobado (y cuántas mesas rechazará el servicio), el avance cada 10 % y, al terminar,
las mesas publicadas y el tiempo. No guarda estado: si se relanza, publica todo otra vez y el servicio descarta lo ya
contado.

## Códigos de salida

0 si termina bien, 1 si fallan los ficheros, sus comprobaciones o Kafka, y 2 si los argumentos no
son válidos.
