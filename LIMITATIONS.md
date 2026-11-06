# Limitations

- **Not a reproduction.** No HTTP/2 implementation runs; no network I/O.
- **Abstract time and cost.** A tick is a scheduling step; a handler costs a
  fixed number of ticks. Real cost includes goroutine/thread creation,
  header decoding (HPACK), memory and backend calls, none of which is
  modelled. Scenario 08 shows running handlers above the limit with zero
  handler ticks because cancellation is observed before the first tick; real
  servers still pay the start cost.
- **One fix modelled.** Only the golang.org/x/net change is reproduced. Other
  mitigations (reset-rate limits, GOAWAY policies in other servers) are not.
- **Simplified stream lifecycle.** No reserved states, no server push, no
  flow control, no PRIORITY, no SETTINGS changes during a connection. A
  refused stream is recorded as closed; the REFUSED_STREAM frame is not
  modelled. "Idle" means a client id greater than the highest one opened.
- **Sweep numbers are model facts,** not server measurements.
- **Closed streams are kept, memory is not bounded or measured.**
  `Server::streams` is a `BTreeMap` that keeps every stream id ever seen,
  including `Closed` (and refused) ones, so the model's own state grows with
  the number of streams opened. It does not model, measure or claim a bound
  on total server memory, and it says nothing about how a real server
  discards closed-stream state. The metrics count streams, handlers, queue
  and ticks only.
- **No real server.** Nothing here is derived from measuring golang.org/x/net
  or any other server; the fix is ported as logic from the cited diff.
- CI is defined in `.github/workflows/ci.yml` (fmt, clippy, tests, results
  regeneration). Results were also reproduced locally with rustc 1.99.1; the
  status of hosted runs is whatever the repository's Actions page shows.
