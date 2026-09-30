use std::fmt::Write as _;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    OnceLock,
};
use std::time::{Duration, Instant};

struct Metrics {
    started_at: Instant,
    requests_total: AtomicU64,
    requests_in_flight: AtomicU64,
    duration_nanos_total: AtomicU64,
    responses_2xx: AtomicU64,
    responses_3xx: AtomicU64,
    responses_4xx: AtomicU64,
    responses_5xx: AtomicU64,
}

impl Metrics {
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
            requests_total: AtomicU64::new(0),
            requests_in_flight: AtomicU64::new(0),
            duration_nanos_total: AtomicU64::new(0),
            responses_2xx: AtomicU64::new(0),
            responses_3xx: AtomicU64::new(0),
            responses_4xx: AtomicU64::new(0),
            responses_5xx: AtomicU64::new(0),
        }
    }

    fn response_counter(&self, status: u16) -> &AtomicU64 {
        match status / 100 {
            2 => &self.responses_2xx,
            3 => &self.responses_3xx,
            4 => &self.responses_4xx,
            _ => &self.responses_5xx,
        }
    }
}

static METRICS: OnceLock<Metrics> = OnceLock::new();

fn metrics() -> &'static Metrics {
    METRICS.get_or_init(Metrics::new)
}

pub fn request_started() {
    let metrics = metrics();
    metrics.requests_total.fetch_add(1, Ordering::Relaxed);
    metrics.requests_in_flight.fetch_add(1, Ordering::Relaxed);
}

pub fn request_finished(status: u16, latency: Duration) {
    let metrics = metrics();
    metrics
        .requests_in_flight
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_sub(1)
        })
        .ok();
    metrics.duration_nanos_total.fetch_add(
        latency.as_nanos().min(u64::MAX as u128) as u64,
        Ordering::Relaxed,
    );
    metrics
        .response_counter(status)
        .fetch_add(1, Ordering::Relaxed);
}

pub fn render(anonymous_users: i64) -> String {
    let metrics = metrics();
    let requests_total = metrics.requests_total.load(Ordering::Relaxed);
    let requests_in_flight = metrics.requests_in_flight.load(Ordering::Relaxed);
    let duration_seconds = metrics.duration_nanos_total.load(Ordering::Relaxed) as f64 / 1e9;
    let uptime_seconds = metrics.started_at.elapsed().as_secs_f64();

    let mut output = String::new();
    output.push_str(
        "# HELP pay3flow_up Whether the Pay3Flow backend is accepting requests.\n\
# TYPE pay3flow_up gauge\n\
pay3flow_up 1\n\
# HELP pay3flow_build_info Static information about the Pay3Flow backend.\n\
# TYPE pay3flow_build_info gauge\n\
pay3flow_build_info{service=\"backend\"} 1\n\
# HELP process_uptime_seconds Seconds since the backend process started.\n\
# TYPE process_uptime_seconds gauge\n",
    );
    let _ = writeln!(output, "process_uptime_seconds {uptime_seconds}");
    output.push_str(
        "# HELP pay3flow_http_requests_total Total HTTP requests received by the backend.\n\
# TYPE pay3flow_http_requests_total counter\n",
    );
    let _ = writeln!(output, "pay3flow_http_requests_total {requests_total}");
    output.push_str(
        "# HELP pay3flow_http_requests_in_flight Current HTTP requests being handled.\n\
# TYPE pay3flow_http_requests_in_flight gauge\n",
    );
    let _ = writeln!(
        output,
        "pay3flow_http_requests_in_flight {requests_in_flight}"
    );
    output.push_str(
        "# HELP pay3flow_http_request_duration_seconds_total Sum of HTTP request durations.\n\
# TYPE pay3flow_http_request_duration_seconds_total counter\n",
    );
    let _ = writeln!(
        output,
        "pay3flow_http_request_duration_seconds_total {duration_seconds}"
    );
    output.push_str(
        "# HELP pay3flow_http_responses_total HTTP responses grouped by status class.\n\
# TYPE pay3flow_http_responses_total counter\n",
    );
    for (status_class, counter) in [
        ("2xx", &metrics.responses_2xx),
        ("3xx", &metrics.responses_3xx),
        ("4xx", &metrics.responses_4xx),
        ("5xx", &metrics.responses_5xx),
    ] {
        let _ = writeln!(
            output,
            "pay3flow_http_responses_total{{status_class=\"{status_class}\"}} {}",
            counter.load(Ordering::Relaxed)
        );
    }
    output.push_str(
        "# HELP pay3flow_anonymous_users_total Number of pseudonymous browser IDs seen by the application.\n\
# TYPE pay3flow_anonymous_users_total gauge\n",
    );
    let _ = writeln!(output, "pay3flow_anonymous_users_total {anonymous_users}");

    output
}
