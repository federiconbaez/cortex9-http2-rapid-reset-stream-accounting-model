# Sources

Read 2026-11-06. SHA256 values are of the fetched copies (not committed).

- **RFC 9113** https://www.rfc-editor.org/rfc/rfc9113.txt
  (a00ef91b64e111a282e77ec66980f5242e77c0bb5e33e0927e3b6757080506de)
  - §5.1.2: "Streams that are in the "open" state or in either of the
    "half-closed" states count toward the maximum number of streams that an
    endpoint is permitted to open." "An endpoint that receives a HEADERS
    frame that causes its advertised concurrent stream limit to be exceeded
    MUST treat this as a stream error (Section 5.4.2) of type PROTOCOL_ERROR
    or REFUSED_STREAM."
  - §6.4: "The RST_STREAM frame fully terminates the referenced stream and
    causes it to enter the "closed" state." RST_STREAM on an idle stream is a
    connection error.
- **GHSA-qppj-fm5r-hxr3 (CVE-2023-44487)**, `gh api "/advisories?cve_id=CVE-2023-44487"`
  (3d90dd8f38e7d584b30a36d0de8bd69f6757bb64d922801c9317465b56318881):
  "By explicitly canceling the requests, the attacker never exceeds the limit
  on the number of concurrent open streams." Lists golang.org/x/net < 0.17.0
  among affected packages.
- **golang.org/x/net commit b225e7ca6dde1ef5a5ae5ce922861bda011cfabd**
  https://github.com/golang/net/commit/b225e7ca6dde1ef5a5ae5ce922861bda011cfabd
  (diff e9165486b0cd3d49eaff59d98172338028c9c254027a2e2a28e41724c68c876f):
  "When the peer opens a new stream while we have MaxConcurrentStreams handler
  goroutines running, defer starting a handler until one of the existing
  handlers exits." "Fixes CVE-2023-39325". The `scheduleHandler`
  (`len(sc.unstartedHandlers) > int(4*sc.advMaxStreams)` →
  `ErrCodeEnhanceYourCalm`) and `handlerDone` (skip `sc.streams[id] == nil`)
  logic is reproduced in `Scheduler::BoundedQueue`.
- CVE record: https://www.cve.org/CVERecord?id=CVE-2023-44487
