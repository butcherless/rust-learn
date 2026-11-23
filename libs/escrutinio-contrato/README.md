# escrutinio-contrato

Librería compartida por las apps del escrutinio
([escrutinio-simulador](../../apps/escrutinio-simulador), [escrutinio-servicio](../../apps/escrutinio-servicio)
y, más adelante, la TUI [escrutinio](../../apps/escrutinio)): los mensajes de Kafka (la mesa y el dead
letter), la validación de una mesa y el snapshot de `GET /snapshots`.

Es pura, sin E/S: tipos con serde y reglas. El JSON de cada tipo está fijado en sus tests, y el diseño
en los [requisitos de escrutinio](../../apps/escrutinio/docs/requisitos-escrutinio.html), sección
«Backend». Un cambio aquí cambia el contrato entre las apps: actualiza las tres a la vez.

```bash
cargo test -p escrutinio-contrato
```
