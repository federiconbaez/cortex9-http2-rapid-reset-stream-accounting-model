# Guía práctica / Practice guide

Ejercicios **offline y locales** sobre el modelo. No se abre ningún socket ni
se envía tráfico a ningún servidor. *English summary at the end.*

## Objetivos

1. Distinguir "streams contados" de "trabajo en curso" con RFC 9113 §5.1.2.
2. Explicar por qué abrir y resetear de inmediato no choca con el límite.
3. Leer el fix de golang.org/x/net (b225e7c) y verlo funcionar en
   `Scheduler::BoundedQueue`.
4. Diseñar y probar una mitigación propia con veredictos calculados.

## Prerrequisitos

Rust estable. Lectura: RFC 9113 §5.1, §5.1.2, §6.4; GHSA-qppj-fm5r-hxr3; el
diff de b225e7c (enlaces en [SOURCES.md](SOURCES.md)).

## Comandos locales

```sh
cargo test
cargo run --release --bin h2-model -- run fixtures/scenarios/06_rapid_reset_per_request.json
cargo run --release --bin h2-model -- matrix fixtures/scenarios /tmp/out && diff -r results /tmp/out
```

## Ejercicios graduados

**Nivel 1 — contar.** Compara `peak_counted_streams` y `peak_running_handlers`
en 05 y 06. ¿Por qué 05 queda acotado sin ningún fix? (§5.1.2 rechaza los
HEADERS sobre el límite.)

**Nivel 2 — la regla.** Encuentra en `src/model.rs` la línea que implementa
"open y half-closed cuentan, closed no". Cambia `counts_toward_limit` para que
closed también cuente y explica qué escenarios cambian y por qué ese cambio
**no** sería conforme con la RFC.

**Nivel 3 — el fix.** En 07, ¿en qué tick se cierra la conexión y con qué
longitud de cola? Relaciónalo con `4*sc.advMaxStreams` del diff.

**Nivel 4 — contrafactual.** 08 activa `handler_observes_cancel`. ¿Por qué
el veredicto sigue siendo `work_exceeds_stream_limit` si el trabajo es 0? ¿Qué
costo real no está en el modelo? (Ver LIMITATIONS.)

**Nivel 5 — umbral.** Con el planificador acotado, busca en `sweep.csv` la
tasa más baja que cierra la conexión. Deriva a mano el tick de cierre para
200/tick.

**Nivel 6 — mitigación propia (código).** Agrega un `Scheduler::ResetRateLimit`
que cierre la conexión si los RST_STREAM por tick superan un umbral. Añade
fixtures, expectativas en `fixtures/expected.json` y tests que comprueben que
01–05 siguen `work_bounded` (sin falsos positivos).

**Nivel 7 — defensa en tu stack.** Escribe una tabla de qué contadores
(streams contados, handlers activos, cola, resets/s) expondrías como métricas
en un servidor propio para detectar este patrón.

## Vínculo modelo ↔ CVE

| Caso real | En el código |
|---|---|
| Límite de concurrencia anunciado por el servidor | `ServerCfg.max_concurrent_streams` |
| RST_STREAM unilateral cierra el stream de inmediato | `Frame::RstStream` → `StreamState::Closed` |
| El servidor ya empezó a trabajar en cada petición | `Scheduler::PerRequest` |
| Fix de Go: handlers acotados, cola, ENHANCE_YOUR_CALM | `Scheduler::BoundedQueue` |

---

## English summary

Offline exercises only. Levels: (1) counted streams vs running handlers in
05 and 06; (2) the §5.1.2 counting rule and why changing it would not
conform; (3) the tick and queue length at which the Go fix closes the
connection; (4) the cancellation counterfactual and the start cost the model
omits; (5) find the closing threshold in the sweep; (6) implement a
reset-rate scheduler without false positives on 01–05; (7) design server
metrics for this pattern.
