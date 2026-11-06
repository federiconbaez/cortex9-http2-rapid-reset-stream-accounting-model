use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

/// RFC 9113 §5.1 states reachable from a client-initiated stream in this model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamState {
    Open,
    HalfClosedRemote,
    Closed,
}

impl StreamState {
    /// §5.1.2: open and both half-closed states count toward the limit.
    pub fn counts_toward_limit(self) -> bool {
        matches!(self, StreamState::Open | StreamState::HalfClosedRemote)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scheduler {
    /// One handler started per accepted request, independent of how many
    /// handlers are already running (the pre-fix shape).
    PerRequest,
    /// x/net b225e7c scheduleHandler: at most `max_concurrent_streams` running handlers;
    /// extra requests queue; queue longer than 4x the limit is a connection
    /// error ENHANCE_YOUR_CALM; queued entries whose stream was reset are
    /// skipped when a slot frees.
    BoundedQueue,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ServerCfg {
    pub max_concurrent_streams: u32,
    pub scheduler: Scheduler,
    /// Ticks a handler runs before it completes.
    pub handler_ticks: u32,
    /// If true, a handler whose stream was reset stops at the next tick.
    /// If false, it runs to completion (work is not cancelled).
    pub handler_observes_cancel: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "f", rename_all = "snake_case")]
pub enum Frame {
    Headers { id: u32, end_stream: bool },
    RstStream { id: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnError {
    /// §5.1.1: new stream id must be odd and greater than all previous ones.
    ProtocolErrorStreamId,
    /// §6.4: RST_STREAM on an idle stream.
    ProtocolErrorRstIdle,
    /// x/net b225e7c: too many queued handlers.
    EnhanceYourCalm,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Metrics {
    pub requests_accepted: u64,
    pub refused_streams: u64,
    pub resets_received: u64,
    pub handlers_started: u64,
    pub handlers_skipped_reset_before_start: u64,
    pub responses_sent: u64,
    pub handler_ticks_total: u64,
    pub peak_counted_streams: u32,
    pub peak_running_handlers: u32,
    pub peak_queue: u32,
}

struct Handler {
    stream: u32,
    remaining: u32,
}

pub struct Server {
    pub cfg: ServerCfg,
    pub streams: BTreeMap<u32, StreamState>,
    highest_id: u32,
    running: Vec<Handler>,
    queue: VecDeque<u32>,
    pub metrics: Metrics,
    pub error: Option<(u32, ConnError)>,
}

impl Server {
    pub fn new(cfg: ServerCfg) -> Self {
        Server {
            cfg,
            streams: BTreeMap::new(),
            highest_id: 0,
            running: Vec::new(),
            queue: VecDeque::new(),
            metrics: Metrics::default(),
            error: None,
        }
    }

    pub fn counted_streams(&self) -> u32 {
        self.streams
            .values()
            .filter(|s| s.counts_toward_limit())
            .count() as u32
    }

    fn observe(&mut self) {
        let c = self.counted_streams();
        let m = &mut self.metrics;
        m.peak_counted_streams = m.peak_counted_streams.max(c);
        m.peak_running_handlers = m.peak_running_handlers.max(self.running.len() as u32);
        m.peak_queue = m.peak_queue.max(self.queue.len() as u32);
    }

    fn start(&mut self, id: u32) {
        self.running.push(Handler {
            stream: id,
            remaining: self.cfg.handler_ticks,
        });
        self.metrics.handlers_started += 1;
    }

    fn schedule(&mut self, id: u32) -> Result<(), ConnError> {
        match self.cfg.scheduler {
            Scheduler::PerRequest => {
                self.start(id);
                Ok(())
            }
            Scheduler::BoundedQueue => {
                let max = self.cfg.max_concurrent_streams as usize;
                if self.running.len() < max {
                    self.start(id);
                    return Ok(());
                }
                if self.queue.len() > 4 * max {
                    return Err(ConnError::EnhanceYourCalm);
                }
                self.queue.push_back(id);
                Ok(())
            }
        }
    }

    /// Process one frame. Returns Err on a connection error.
    pub fn on_frame(&mut self, f: Frame) -> Result<(), ConnError> {
        match f {
            Frame::Headers { id, end_stream } => {
                if id.is_multiple_of(2) || id <= self.highest_id {
                    return Err(ConnError::ProtocolErrorStreamId);
                }
                self.highest_id = id;
                if self.counted_streams() >= self.cfg.max_concurrent_streams {
                    // §5.1.2 stream error; the stream is refused, not opened.
                    self.metrics.refused_streams += 1;
                    self.streams.insert(id, StreamState::Closed);
                    return Ok(());
                }
                let st = if end_stream {
                    StreamState::HalfClosedRemote
                } else {
                    StreamState::Open
                };
                self.streams.insert(id, st);
                self.metrics.requests_accepted += 1;
                self.schedule(id)?;
            }
            Frame::RstStream { id } => {
                if id > self.highest_id || id.is_multiple_of(2) {
                    return Err(ConnError::ProtocolErrorRstIdle);
                }
                self.metrics.resets_received += 1;
                // §6.4: RST_STREAM moves the stream to closed.
                if let Some(s) = self.streams.get_mut(&id) {
                    *s = StreamState::Closed;
                }
            }
        }
        self.observe();
        Ok(())
    }

    fn stream_alive(&self, id: u32) -> bool {
        self.streams
            .get(&id)
            .map(|s| s.counts_toward_limit())
            .unwrap_or(false)
    }

    /// Advance running handlers by one tick.
    pub fn tick(&mut self) {
        let mut done = Vec::new();
        for (i, h) in self.running.iter_mut().enumerate() {
            let alive = self
                .streams
                .get(&h.stream)
                .map(|s| s.counts_toward_limit())
                .unwrap_or(false);
            if !alive && self.cfg.handler_observes_cancel {
                done.push(i);
                continue;
            }
            self.metrics.handler_ticks_total += 1;
            h.remaining = h.remaining.saturating_sub(1);
            if h.remaining == 0 {
                done.push(i);
            }
        }
        for i in done.into_iter().rev() {
            let h = self.running.swap_remove(i);
            if self.stream_alive(h.stream) {
                self.streams.insert(h.stream, StreamState::Closed);
                self.metrics.responses_sent += 1;
            }
            if self.cfg.scheduler == Scheduler::BoundedQueue {
                self.handler_done();
            }
        }
        self.observe();
    }

    /// x/net b225e7c handlerDone: start queued handlers, skipping streams
    /// that were reset before their handler started.
    fn handler_done(&mut self) {
        let max = self.cfg.max_concurrent_streams as usize;
        while let Some(&id) = self.queue.front() {
            if !self.stream_alive(id) {
                self.queue.pop_front();
                self.metrics.handlers_skipped_reset_before_start += 1;
                continue;
            }
            if self.running.len() >= max {
                break;
            }
            self.queue.pop_front();
            self.start(id);
        }
    }
}

/// A scenario: per tick, a list of frames; then the server advances one tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub description: String,
    pub server: ServerCfg,
    pub workload: Workload,
    /// Extra ticks with no frames, to drain handlers.
    pub drain_ticks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Workload {
    /// Each tick: open `per_tick` requests and reset each one right away.
    OpenThenReset { ticks: u32, per_tick: u32 },
    /// Each tick: open `per_tick` requests (END_STREAM), never reset.
    Requests { ticks: u32, per_tick: u32 },
    /// Each tick: open `per_tick` requests; reset one in every `reset_every`.
    OccasionalCancel {
        ticks: u32,
        per_tick: u32,
        reset_every: u32,
    },
    /// Explicit frame list per tick.
    Explicit { ticks: Vec<Vec<Frame>> },
}

impl Workload {
    pub fn frames(&self) -> Vec<Vec<Frame>> {
        let mut id = 1u32;
        let mut next = || {
            let x = id;
            id += 2;
            x
        };
        match self {
            Workload::OpenThenReset { ticks, per_tick } => (0..*ticks)
                .map(|_| {
                    let mut v = Vec::new();
                    for _ in 0..*per_tick {
                        let i = next();
                        v.push(Frame::Headers {
                            id: i,
                            end_stream: true,
                        });
                        v.push(Frame::RstStream { id: i });
                    }
                    v
                })
                .collect(),
            Workload::Requests { ticks, per_tick } => (0..*ticks)
                .map(|_| {
                    (0..*per_tick)
                        .map(|_| Frame::Headers {
                            id: next(),
                            end_stream: true,
                        })
                        .collect()
                })
                .collect(),
            Workload::OccasionalCancel {
                ticks,
                per_tick,
                reset_every,
            } => {
                let mut n = 0u32;
                (0..*ticks)
                    .map(|_| {
                        let mut v = Vec::new();
                        for _ in 0..*per_tick {
                            let i = next();
                            v.push(Frame::Headers {
                                id: i,
                                end_stream: true,
                            });
                            n += 1;
                            if *reset_every > 0 && n.is_multiple_of(*reset_every) {
                                v.push(Frame::RstStream { id: i });
                            }
                        }
                        v
                    })
                    .collect()
            }
            Workload::Explicit { ticks } => ticks.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Running handlers never exceeded the advertised stream limit.
    WorkBounded,
    /// Running handlers exceeded the advertised limit although counted
    /// streams did not: the stream limit stopped bounding work.
    WorkExceedsStreamLimit,
    /// The server closed the connection.
    ConnectionClosed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outcome {
    pub scenario: String,
    pub verdict: Verdict,
    pub error: Option<(u32, ConnError)>,
    pub metrics: Metrics,
    pub ticks_run: u32,
}

pub fn run(sc: &Scenario) -> Outcome {
    let mut s = Server::new(sc.server);
    let frames = sc.workload.frames();
    let mut t = 0u32;
    'outer: for tick_frames in &frames {
        for f in tick_frames {
            if let Err(e) = s.on_frame(*f) {
                s.error = Some((t, e));
                break 'outer;
            }
        }
        s.tick();
        t += 1;
    }
    if s.error.is_none() {
        for _ in 0..sc.drain_ticks {
            s.tick();
            t += 1;
        }
    }
    let verdict = if s.error.is_some() {
        Verdict::ConnectionClosed
    } else if s.metrics.peak_running_handlers > sc.server.max_concurrent_streams {
        Verdict::WorkExceedsStreamLimit
    } else {
        Verdict::WorkBounded
    };
    Outcome {
        scenario: sc.name.clone(),
        verdict,
        error: s.error,
        metrics: s.metrics,
        ticks_run: t,
    }
}
