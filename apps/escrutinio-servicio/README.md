# escrutinio-servicio

Consume las mesas que publica [escrutinio-simulador](../escrutinio-simulador) en Kafka, las guarda en
PostgreSQL y sirve el acumulado de cada circunscripción en `GET /snapshots`, para la TUI. Cada mesa
se cuenta una sola vez, aunque llegue repetida, y lo que no es una mesa válida va al dead letter.

El diseño (tablas, transacción, offsets y API) está en los
[requisitos de escrutinio](../escrutinio/docs/requisitos-escrutinio.html), sección «Backend».

## Arrancar

`compose/escrutinio.yaml` levanta Kafka, con sus dos topics, y PostgreSQL 17. Desde la raíz del
workspace:

```bash
docker compose -f compose/escrutinio.yaml up -d --wait   # Kafka, los topics y PostgreSQL
cargo esc-srv                                            # el servicio, en http://localhost:8080
```

En otra terminal, la carga y la consulta:

```bash
cargo esc-sim -- --duracion 5m                           # publica las mesas en 5 minutos
curl -s localhost:8080/snapshots                         # el acumulado por circunscripción
```

`Ctrl+C` para el servicio. `docker compose -f compose/escrutinio.yaml down` para Kafka y PostgreSQL
y borra sus datos; sin `down`, al volver a arrancar el servicio sigue donde lo dejó.

## API

`GET /snapshots` devuelve las circunscripciones con alguna mesa escrutada, por código de provincia:

```json
[{"province":"28","stations":7118,"census":4848057,"voters":3594693,"blank":28700,"invalid":30244,
  "votes":[{"candidacy":5,"acronym":"PP","votes":1444177}, …]}]
```

`stations` son las mesas escrutadas y `census`, su censo. Los votos van de más a menos. Si
PostgreSQL no responde, contesta 503.

## Opciones

| Opción | Por defecto | Qué hace |
|---|---|---|
| `--kafka <SERVIDORES>` | `localhost:9092` | Servidores de Kafka |
| `--topic <TOPIC>` | `escrutinio.mesas` | Topic de las mesas |
| `--topic-dlq <TOPIC_DLQ>` | `escrutinio.mesas.dlq` | Dead letter |
| `--grupo <GRUPO>` | `escrutinio-servicio` | Grupo de consumidores; uno nuevo empieza desde el principio |
| `--postgres <URL>` | `postgres://escrutinio:escrutinio@localhost:5432/escrutinio` | PostgreSQL, el del compose |
| `--puerto <PUERTO>` | 8080 | Puerto de la API |

Muestra el avance cada 5.000 mensajes: mesas registradas, repetidas y enviadas al dead letter.

## Códigos de salida

0 al pulsar `Ctrl+C`, 1 si fallan PostgreSQL, Kafka o la API (al arrancarlo de nuevo, recibe otra vez
lo que no llegó a confirmar), y 2 si los argumentos no son válidos.
