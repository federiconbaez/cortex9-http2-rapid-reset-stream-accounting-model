# cortex9-http2-rapid-reset-stream-accounting-model

[English](README.md) · [Guía práctica](PRACTICE.md)

Modelo en Rust, de tiempo discreto, de cómo un servidor HTTP/2 cuenta streams
(RFC 9113 §5.1, §5.1.2, §6.4) frente a cuánto trabajo de handlers ejecuta,
para estudiar el patrón Rapid Reset (CVE-2023-44487).

**Es un modelo, no una reproducción de CVE-2023-44487.** No hay sockets,
codificación de frames ni HTTP, y no puede apuntarse a ningún servidor.

- Estados y conteo de streams según RFC 9113: solo open y half-closed cuentan
  para el límite; RST_STREAM cierra el stream.
- Planificador previo al fix (un handler por petición) y el del commit
  golang.org/x/net b225e7ca6dde (máximo de handlers = límite, cola, error
  ENHANCE_YOUR_CALM si la cola supera 4× el límite, se saltan los reseteados).
  Go identifica su caso como CVE-2023-39325; el GHSA de CVE-2023-44487 lista
  `golang.org/x/net < 0.17.0`.
- Veredictos calculados de contadores: `work_bounded`,
  `work_exceeds_stream_limit`, `connection_closed`.

Resultado clave (límite 100): abrir y resetear 200 por tick deja los streams
contados en 1 y, con el planificador por petición, llega a 1000 handlers
simultáneos; con la cola acotada se mantiene en 100 y cierra la conexión en el
tick 2. Son cifras del modelo, no mediciones.

```sh
cargo test
cargo run --release --bin h2-model -- matrix fixtures/scenarios results
```

Alcance: el modelo guarda en memoria todos los streams vistos, incluidos los
`Closed`; no mide ni acota la memoria total de un servidor y no es un
servidor real ni una medición de uno.

Ver [SOURCES.md](SOURCES.md), [LIMITATIONS.md](LIMITATIONS.md). Licencia Apache-2.0.
