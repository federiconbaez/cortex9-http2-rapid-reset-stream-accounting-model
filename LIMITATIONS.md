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
- CI is defined in `.github/workflows/ci.yml`; until it runs on a hosted
  runner, only local runs (rustc 1.99.1) back the results.
