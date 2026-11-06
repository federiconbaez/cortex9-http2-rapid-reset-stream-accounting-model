use cortex9_http2_rapid_reset_stream_accounting_model::model::*;
use std::collections::BTreeMap;
use std::fs;

fn dir() -> String {
    env!("CARGO_MANIFEST_DIR").to_string()
}

fn load(name: &str) -> Scenario {
    serde_json::from_str(
        &fs::read_to_string(format!("{}/fixtures/scenarios/{name}.json", dir())).unwrap(),
    )
    .unwrap()
}

#[test]
fn fixtures_match_expected_file() {
    let exp: BTreeMap<String, serde_json::Value> = serde_json::from_str(
        &fs::read_to_string(format!("{}/fixtures/expected.json", dir())).unwrap(),
    )
    .unwrap();
    let mut n = 0;
    for e in fs::read_dir(format!("{}/fixtures/scenarios", dir())).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_stem().unwrap().to_str().unwrap().to_string();
        let o = run(&load(&name));
        let w = &exp[&name];
        assert_eq!(
            serde_json::to_value(o.verdict).unwrap(),
            w[0],
            "{name} verdict"
        );
        let err = o.error.map(|(_, e)| format!("{e:?}"));
        assert_eq!(err.as_deref(), w[1].as_str(), "{name} error");
        assert_eq!(
            o.metrics.peak_running_handlers as u64,
            w[2].as_u64().unwrap(),
            "{name} peak running"
        );
        n += 1;
    }
    assert_eq!(n, exp.len() - 1);
}

#[test]
fn counted_streams_never_exceed_limit() {
    for per_tick in [1, 50, 100, 300] {
        for sched in [Scheduler::PerRequest, Scheduler::BoundedQueue] {
            for wl in [
                Workload::Requests { ticks: 8, per_tick },
                Workload::OpenThenReset { ticks: 8, per_tick },
            ] {
                let sc = Scenario {
                    name: "p".into(),
                    description: String::new(),
                    server: ServerCfg {
                        max_concurrent_streams: 100,
                        scheduler: sched,
                        handler_ticks: 4,
                        handler_observes_cancel: false,
                    },
                    workload: wl,
                    drain_ticks: 5,
                };
                let o = run(&sc);
                assert!(o.metrics.peak_counted_streams <= 100);
                if sched == Scheduler::BoundedQueue {
                    assert!(
                        o.metrics.peak_running_handlers <= 100,
                        "bounded scheduler exceeded limit"
                    );
                    assert!(o.metrics.peak_queue <= 401);
                }
            }
        }
    }
}

#[test]
fn rst_closes_stream_and_frees_slot() {
    let mut s = Server::new(ServerCfg {
        max_concurrent_streams: 1,
        scheduler: Scheduler::PerRequest,
        handler_ticks: 10,
        handler_observes_cancel: false,
    });
    s.on_frame(Frame::Headers {
        id: 1,
        end_stream: false,
    })
    .unwrap();
    assert_eq!(s.streams[&1], StreamState::Open);
    s.on_frame(Frame::Headers {
        id: 3,
        end_stream: true,
    })
    .unwrap();
    assert_eq!(
        s.metrics.refused_streams, 1,
        "second stream exceeds limit 1"
    );
    s.on_frame(Frame::RstStream { id: 1 }).unwrap();
    assert_eq!(s.streams[&1], StreamState::Closed);
    s.on_frame(Frame::Headers {
        id: 5,
        end_stream: true,
    })
    .unwrap();
    assert_eq!(s.streams[&5], StreamState::HalfClosedRemote);
    assert_eq!(
        s.metrics.handlers_started, 2,
        "slot freed by RST, handler for 1 still running"
    );
}

#[test]
fn reserved_states_not_modelled_and_closed_does_not_count() {
    assert!(StreamState::Open.counts_toward_limit());
    assert!(StreamState::HalfClosedRemote.counts_toward_limit());
    assert!(!StreamState::Closed.counts_toward_limit());
}

#[test]
fn queue_overflow_threshold_is_four_times_limit() {
    let mut s = Server::new(ServerCfg {
        max_concurrent_streams: 2,
        scheduler: Scheduler::BoundedQueue,
        handler_ticks: 100,
        handler_observes_cancel: false,
    });
    let mut err = None;
    for k in 0..20u32 {
        let id = 2 * k + 1;
        if let Err(e) = s.on_frame(Frame::Headers {
            id,
            end_stream: true,
        }) {
            err = Some(e);
            break;
        }
        s.on_frame(Frame::RstStream { id }).unwrap();
    }
    assert_eq!(err, Some(ConnError::EnhanceYourCalm));
    // 2 running + queue of 9 (= 4*2+1) before the next one is rejected.
    assert_eq!(s.metrics.handlers_started, 2);
    assert_eq!(s.metrics.peak_queue, 9);
}
