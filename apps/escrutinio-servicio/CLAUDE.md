# CLAUDE.md

Este fichero orienta a Claude Code cuando trabaja en el proyecto `escrutinio-servicio` de este workspace. Los comandos comunes, la CI y las convenciones están en el `CLAUDE.md` raíz (en inglés).

Servicio que consume las mesas de Kafka, las guarda en PostgreSQL y sirve los snapshots por provincia en `GET /snapshots`. Es la segunda de las tres apps del backend del escrutinio (simulador, servicio y la TUI `apps/escrutinio`).
- `README.md` es la guía de uso.
- La fuente de verdad es `apps/escrutinio/docs/requisitos-escrutinio.html`, sección «Backend»: tablas, transacción, offsets, dead letter y API. Cada cambio de comportamiento se anota allí primero.
- Los mensajes, la validación y el snapshot son de `libs/escrutinio-contrato`, compartido con el simulador y la TUI.

**Idiomas**, como en escrutinio: en español la documentación, el rustdoc, los comentarios, lo que ve el usuario y las tablas de PostgreSQL (como en los requisitos); en inglés los identificadores, los nombres de los tests y los campos del JSON.

## Comandos

Desde la raíz del workspace:

```bash
cargo test -p escrutinio-servicio                          # unitarios, con fakes; los de integración se saltan
docker compose -f compose/escrutinio.yaml up -d --wait     # Kafka, topics y PostgreSQL
ESCRUTINIO_INTEGRACION=1 cargo test -p escrutinio-servicio      # también los de integración, contra el compose
ESCRUTINIO_INTEGRACION=1 cargo llvm-cov -p escrutinio-servicio  # la cobertura que mide la CI (98 %); sin la variable, ~65 %
cargo esc-srv                                              # el servicio contra el compose
cargo esc-borrar                                           # vacía los topics y las tablas antes de otra carga
cargo esc-sim --duracion 1m                                # la carga completa en un minuto
curl -s localhost:8080/snapshots                           # el resultado
docker compose -f compose/escrutinio.yaml down             # lo para y borra los datos
```

Los tests de integración (`tests/postgres.rs`, `tests/kafka.rs` y el recorrido completo de `tests/cli.rs`) son la excepción a la regla de no usar la red del `CLAUDE.md` raíz: usan el compose en localhost y solo corren con `ESCRUTINIO_INTEGRACION=1`, que la CI activa en el job de este paquete tras levantar el compose. Cada test crea sus topics, su grupo y su esquema de PostgreSQL (`tests/common`), así que pueden ir en paralelo. El de `cli.rs` para el servicio con SIGINT, como Ctrl+C, para que la cobertura de `main.rs` se escriba.

Tras tocar `postgres.rs`, `kafka.rs` o `main.rs`, prueba además la carga completa con los comandos de arriba. Al terminar, `/snapshots` debe sumar 60.310 mesas, 35.140.508 de censo y 24.742.466 votantes (los totales oficiales sin las 4 mesas de León), y relanzar el simulador no debe cambiarlos.

## Arquitectura

Hexagonal ligera, con las dependencias hacia dentro: `main → adapters → application → domain`. Cada módulo explica en su `//!` qué hace y qué regla cumple.

- **`domain`**, puro: `classify` decide si un mensaje se registra o va al dead letter.
- **`application`**: `handle` (registrar una mesa), `wipe` (borrar el escrutinio: Kafka primero, después PostgreSQL), `Tally` y los puertos `StationStore`, `DeadLetters`, `SnapshotSource` y `Wipe`. Son traits genéricos con métodos que devuelven `impl Future + Send`, sin `dyn` ni `async-trait`; las implementaciones usan `async fn`. Los fakes están en `application::fakes`.
- **`adapters`**: `postgres` (las tablas, la transacción y la lectura en una sentencia; `SnapshotRow` convierte las filas y se prueba sola), `kafka` (el consumidor, el productor del dead letter, el bucle `consume` y `KafkaTopics`, que vacía los topics con `delete_records`), `http` (axum, probado con `tower::ServiceExt::oneshot`) y `cli`.
- **`main.rs`**: la raíz de composición; sin subcomando sirve y con `borrar` vacía el escrutinio. Dos conexiones a PostgreSQL (una escribe, otra lee para la API), el bucle de Kafka y la API en el mismo runtime, con `tokio::select!`.

`postgres.rs`, `kafka.rs` y `main.rs` solo se prueban con los tests de integración: no les pongas lógica que se pueda sacar a una función pura.

## Librerías

- **tokio-postgres 0.7**, sin TLS (`NoTls`): PostgreSQL es local. Necesita PostgreSQL 17 o posterior por `transaction_timeout`.
- **rdkafka 0.39**, la del workspace con la feature `tokio`: `StreamConsumer` y `FutureProducer`. Con autocommit, pero guardando el offset a mano tras el `COMMIT` (`enable.auto.offset.store=false` y `store_offset_from_message`). No uses `commit_message` por mensaje: con 60.000 confirmaciones asíncronas el `StreamConsumer` dejó de despertarse.
- **axum 0.8**: el router y `axum::serve`. **tower 0.5** solo en los tests.
