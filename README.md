# cortex9-http2-rapid-reset-stream-accounting-model

[Español](README.es.md) · [Practice guide](PRACTICE.md)

A discrete-time Rust model of how an HTTP/2 server counts streams
(RFC 9113 §5.1, §5.1.2, §6.4) versus how much handler work it runs, used to
study the Rapid Reset pattern (CVE-2023-44487).

**This is a model, not a reproduction of CVE-2023-44487.** There are no
sockets, no frame encoding, no HTTP parsing, and nothing can be pointed at a
server. It keeps every stream it has seen, including `Closed` ones, and does not measure or bound total server memory. See [LIMITATIONS.md](LIMITATIONS.md).

## What is modelled, and from where

| Element | Source | Code |
|---|---|---|
| Stream states open / half-closed (remote) / closed | RFC 9113 §5.1 | `StreamState` |
| Only open and half-closed streams count toward `SETTINGS_MAX_CONCURRENT_STREAMS`; a HEADERS frame over the limit is a stream error (refused) | RFC 9113 §5.1.2 | `Server::on_frame`, `counts_toward_limit` |
| RST_STREAM moves the stream to closed; RST on an idle stream is a connection error | RFC 9113 §6.4 | `Frame::RstStream` branch |
| Cancelling right after opening keeps the counted streams under the limit, while the server has already started work | GHSA-qppj-fm5r-hxr3 | workload `open_then_reset` |
| Pre-fix scheduler: one handler per accepted request (`go sc.runHandler(...)`) | golang.org/x/net commit b225e7ca6dde, removed lines | `Scheduler::PerRequest` |
| Fixed scheduler: at most `advMaxStreams` running handlers; extra requests queue; a queue longer than `4*advMaxStreams` is connection error ENHANCE_YOUR_CALM; queued entries whose stream was reset are skipped | same commit, `scheduleHandler` / `handlerDone` | `Scheduler::BoundedQueue` |

The Go commit message says `Fixes CVE-2023-39325`, which is Go's identifier
for its instance of the issue; GHSA-qppj-fm5r-hxr3 (CVE-2023-44487) lists
`golang.org/x/net < 0.17.0` as affected. Other servers fixed the same pattern
differently; only the Go change is modelled here.

Verdicts are computed from counters: `work_bounded` (running handlers never
exceeded the stream limit), `work_exceeds_stream_limit` (they did although
counted streams stayed within it), `connection_closed`.

## Usage

```sh
cargo test
cargo run --release --bin h2-model -- run fixtures/scenarios/06_rapid_reset_per_request.json
cargo run --release --bin h2-model -- matrix fixtures/scenarios results
(cd results && sha256sum -c SHA256SUMS)
```

## Results (`results/scenarios.csv`, limit 100, handlers run 5 ticks)

| Scenario | Verdict | Peak counted streams | Peak running handlers |
|---|---|---|---|
| 01/02 benign requests (both schedulers) | work_bounded | 100 | 100 |
| 03/04 benign, 1 in 10 cancelled | work_bounded | 91 | 100 |
| 05 200 req/tick, no resets | work_bounded (1800 refused) | 100 | 100 |
| 06 open-then-reset 200/tick, per-request | work_exceeds_stream_limit | 1 | 1000 |
| 07 same, bounded queue | connection_closed (EnhanceYourCalm, tick 2) | 1 | 100 |
| 08 per-request, handlers stop on reset (counterfactual) | work_exceeds_stream_limit | 1 | 200 |
| 09 open-then-reset 50/tick, bounded queue | work_bounded (300 skipped) | 1 | 100 |
| 10 even stream id | connection_closed (ProtocolErrorStreamId) | 0 | 0 |
| 11 RST on idle stream | connection_closed (ProtocolErrorRstIdle) | 1 | 1 |
| 12 reset while queued | work_bounded (1 skipped) | 2 | 2 |

`results/sweep.csv` varies requests per tick (1–400) for both workloads and
schedulers. With plain requests, both schedulers stay at 100 running
handlers. With open-then-reset, the per-request scheduler reaches 5 × the
per-tick rate (up to 2000), and the bounded queue stays at 100 or closes the
connection at rates of 200 and 400 per tick. These are model numbers, not
measurements of any server.

Built and tested locally with rustc 1.99.1.

## Files to read first

`src/model.rs`, `tests/model.rs`, `fixtures/scenarios/`.
`fixtures/expected.json` holds hand-derived expectations read only by tests.

See [SOURCES.md](SOURCES.md), [LIMITATIONS.md](LIMITATIONS.md). License: Apache-2.0.
