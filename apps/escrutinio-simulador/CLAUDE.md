# CLAUDE.md

Este fichero orienta a Claude Code cuando trabaja en el proyecto `escrutinio-simulador` de este workspace. Los comandos comunes, la CI y las convenciones están en el `CLAUDE.md` raíz (en inglés).

Comando que publica en Kafka las mesas del Congreso de 2023, cada una en su momento. Es la primera de las tres apps del backend del escrutinio (simulador, servicio y la TUI `apps/escrutinio`).
- `README.md` es la guía de uso.
- La fuente de verdad es `apps/escrutinio/docs/requisitos-escrutinio.html`: el formato de los ficheros de Interior, las comprobaciones, el mensaje de Kafka y la curva de llegada. Cada cambio de comportamiento se anota allí primero.

**Idiomas**, como en escrutinio: en español la documentación, el rustdoc, los comentarios y lo que ve el usuario; en inglés los identificadores, los nombres de los tests y los campos del JSON de Kafka.

## Comandos

Desde la raíz del workspace:

```bash
cargo esc-sim -- --solo-comprobar             # comprueba los ficheros de apps/escrutinio-simulador/datos
cargo test -p escrutinio-simulador            # unitarios y tests/cli.rs, que ejecuta el binario
cargo llvm-cov -p escrutinio-simulador        # cobertura; --open para el HTML
docker compose -f compose/escrutinio.yaml up -d   # Kafka local con los dos topics, para probarlo de verdad
```

`tests/data` es un extracto real de los ficheros de Interior: las mesas de Ceuta y Melilla (183), con sus filas de CERA y, en `07`, también el total de España, para probar los filtros. Los tests nunca conectan con Kafka.

## Arquitectura

Hexagonal ligera, con las dependencias hacia dentro: `main → adapters → application → domain`. Cada módulo explica en su `//!` qué hace y qué regla cumple.

- **`domain`**, puro: `station` (las filas, `build` con las comprobaciones que rechazan el fichero entero, y `Summary`), `validation` (lo que manda una mesa sola al dead letter), `schedule` (la curva de llegada y el orden con semilla) y `message` (el JSON de una mesa y el del dead letter: el contrato con el servicio).
- **`application`**: `run` valida y publica el plan, cada mesa en su `Topic`; `Publisher` y `Pacer` son los puertos (traits genéricos, sin `dyn`), con fakes en los tests.
- **`adapters`**: `interior` (los ficheros de ancho fijo en Latin-1), `kafka` (el productor), `clock` (duerme hasta cada mesa) y `cli` (clap y los textos).
- **`main.rs`**: la raíz de composición; imprime el error con sus causas.

`kafka.rs` y la parte de `main.rs` que publica no tienen tests, porque necesitan un broker: no les pongas lógica.

## Librerías

- **rdkafka 0.39**, sin las features por defecto (tokio) y con `libz`: el `BaseProducer` síncrono basta. Compila librdkafka desde C (make y un compilador C; sin CMake), unos 15 s la primera vez. La 0.40.0 salió el 21-11-2026: subir tras la espera de Dependabot. Al conectar pide los metadatos del topic con `allow.auto.create.topics=false`, para fallar pronto si Kafka o el topic no están.
- **serde_json**: el mensaje. **clap 4.6** y **thiserror**, como en el resto del workspace.
