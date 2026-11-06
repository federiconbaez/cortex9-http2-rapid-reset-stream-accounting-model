use cortex9_http2_rapid_reset_stream_accounting_model::model::*;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

fn load(p: &Path) -> Scenario {
    serde_json::from_str(&fs::read_to_string(p).unwrap())
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("run") if a.len() == 3 => {
            println!(
                "{}",
                serde_json::to_string_pretty(&run(&load(Path::new(&a[2])))).unwrap()
            )
        }
        Some("matrix") if a.len() == 4 => matrix(Path::new(&a[2]), Path::new(&a[3])),
        _ => {
            eprintln!("usage: h2-model run <scenario.json> | matrix <scenarios_dir> <out_dir>");
            std::process::exit(2)
        }
    }
}

fn matrix(dir: &Path, out: &Path) {
    fs::create_dir_all(out).unwrap();
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    let mut csv = String::from(
        "scenario,scheduler,max_streams,handler_ticks,observes_cancel,verdict,error,error_tick,accepted,refused,resets,handlers_started,skipped,responses,handler_ticks_total,peak_counted,peak_running,peak_queue\n",
    );
    let mut outs = Vec::new();
    for f in &files {
        let sc = load(f);
        let o = run(&sc);
        let m = &o.metrics;
        let (et, e) = o
            .error
            .map(|(t, e)| (t.to_string(), format!("{e:?}")))
            .unwrap_or_default();
        csv.push_str(&format!(
            "{},{:?},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            o.scenario,
            sc.server.scheduler,
            sc.server.max_concurrent_streams,
            sc.server.handler_ticks,
            sc.server.handler_observes_cancel,
            serde_json::to_value(o.verdict).unwrap().as_str().unwrap(),
            e,
            et,
            m.requests_accepted,
            m.refused_streams,
            m.resets_received,
            m.handlers_started,
            m.handlers_skipped_reset_before_start,
            m.responses_sent,
            m.handler_ticks_total,
            m.peak_counted_streams,
            m.peak_running_handlers,
            m.peak_queue
        ));
        outs.push(o);
    }
    fs::write(out.join("scenarios.csv"), &csv).unwrap();
    fs::write(
        out.join("scenarios.json"),
        serde_json::to_string_pretty(&outs).unwrap() + "\n",
    )
    .unwrap();

    // Sweep: requests per tick for open-then-reset vs plain requests, both schedulers.
    let mut sw = String::from(
        "workload,scheduler,per_tick,verdict,error_tick,peak_counted,peak_running,handler_ticks_total\n",
    );
    for wl in ["open_then_reset", "requests"] {
        for sched in [Scheduler::PerRequest, Scheduler::BoundedQueue] {
            for per_tick in [1u32, 10, 50, 100, 200, 400] {
                let workload = if wl == "open_then_reset" {
                    Workload::OpenThenReset {
                        ticks: 20,
                        per_tick,
                    }
                } else {
                    Workload::Requests {
                        ticks: 20,
                        per_tick,
                    }
                };
                let sc = Scenario {
                    name: "sweep".into(),
                    description: String::new(),
                    server: ServerCfg {
                        max_concurrent_streams: 100,
                        scheduler: sched,
                        handler_ticks: 5,
                        handler_observes_cancel: false,
                    },
                    workload,
                    drain_ticks: 10,
                };
                let o = run(&sc);
                sw.push_str(&format!(
                    "{wl},{sched:?},{per_tick},{},{},{},{},{}\n",
                    serde_json::to_value(o.verdict).unwrap().as_str().unwrap(),
                    o.error.map(|(t, _)| t.to_string()).unwrap_or_default(),
                    o.metrics.peak_counted_streams,
                    o.metrics.peak_running_handlers,
                    o.metrics.handler_ticks_total
                ));
            }
        }
    }
    fs::write(out.join("sweep.csv"), &sw).unwrap();
    let mut sums = String::new();
    for n in ["scenarios.csv", "scenarios.json", "sweep.csv"] {
        let d = Sha256::digest(fs::read(out.join(n)).unwrap());
        sums.push_str(&format!(
            "{}  {n}\n",
            d.iter().map(|x| format!("{x:02x}")).collect::<String>()
        ));
    }
    fs::write(out.join("SHA256SUMS"), sums).unwrap();
    eprintln!(
        "wrote {} scenarios + sweep to {}",
        outs.len(),
        out.display()
    );
}
