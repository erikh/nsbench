use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::BenchConfig;
use crate::metrics::{LiveCounters, MergedResults};

pub async fn informer(live: Arc<LiveCounters>, finished: Arc<AtomicBool>, warmup: Duration) {
    let start = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    interval.tick().await; // first tick is immediate, skip it

    let mut prev_successes = 0u64;
    let mut prev_failures = 0u64;

    loop {
        interval.tick().await;
        if finished.load(Ordering::Acquire) {
            break;
        }

        let elapsed = start.elapsed();
        let secs = elapsed.as_secs();

        let successes = live.successes.load(Ordering::Relaxed);
        let failures = live.failures.load(Ordering::Relaxed);

        let delta_success = successes - prev_successes;
        let delta_fail = failures - prev_failures;
        let qps = delta_success + delta_fail;

        prev_successes = successes;
        prev_failures = failures;

        if elapsed < warmup {
            eprintln!(
                "[warmup] {:>3}s | qps: {} | errors: {}",
                secs,
                format_number(qps),
                delta_fail,
            );
        } else {
            eprintln!(
                "{:>9}s | qps: {} | errors: {}",
                secs,
                format_number(qps),
                delta_fail,
            );
        }
    }
}

pub fn print_report(config: &BenchConfig, results: &MergedResults) {
    let successes = results.histogram.len();
    let failures = results.errors.total();
    let total = successes + failures;
    let duration_secs = config.duration.as_secs();
    let qps = if duration_secs > 0 {
        successes / duration_secs
    } else {
        0
    };
    let success_rate = if total > 0 {
        (successes as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    println!("--- nsbench results ---");
    println!("Target:     {}", config.nameserver);
    println!("Hostname:   {}", config.host);
    println!("Record:     {}", config.record_type);
    println!("Protocol:   {}", config.protocol);
    println!("Workers:    {}", config.workers);
    println!(
        "Duration:   {}s ({}s warmup excluded)",
        duration_secs,
        config.warmup.as_secs()
    );
    println!();
    println!("Queries:");
    println!("  Total:      {}", format_number(total));
    println!("  Successful: {}", format_number(successes));
    println!("  Failed:     {}", format_number(failures));
    println!("  QPS:        {}", format_number(qps));
    println!();

    if successes > 0 {
        println!("Latency:");
        println!(
            "  Mean:    {:>10}    Min:   {:>10}",
            format_latency(results.histogram.mean()),
            format_latency(results.histogram.min() as f64),
        );
        println!(
            "  p50:     {:>10}    p95:   {:>10}",
            format_latency(results.histogram.value_at_percentile(50.0) as f64),
            format_latency(results.histogram.value_at_percentile(95.0) as f64),
        );
        println!(
            "  p99:     {:>10}    p99.9: {:>10}",
            format_latency(results.histogram.value_at_percentile(99.0) as f64),
            format_latency(results.histogram.value_at_percentile(99.9) as f64),
        );
        println!(
            "  Max:     {:>10}",
            format_latency(results.histogram.max() as f64),
        );
        println!();
    }

    let e = &results.errors;
    println!("Errors:");
    println!(
        "  Timeouts: {}  SERVFAIL: {}  REFUSED: {}",
        e.timeouts, e.servfail, e.refused,
    );
    println!(
        "  NXDOMAIN: {}   Network: {}   Other: {}",
        e.nxdomain, e.network_errors, e.other,
    );
    println!();
    println!("Success Rate: {:.4}%", success_rate);
}

pub fn print_json_report(config: &BenchConfig, results: &MergedResults) {
    let successes = results.histogram.len();
    let failures = results.errors.total();
    let total = successes + failures;
    let duration_secs = config.duration.as_secs();
    let qps = if duration_secs > 0 {
        successes / duration_secs
    } else {
        0
    };
    let success_rate = if total > 0 {
        (successes as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let e = &results.errors;
    let (mean, min, p50, p95, p99, p999, max) = if successes > 0 {
        (
            results.histogram.mean(),
            results.histogram.min() as f64,
            results.histogram.value_at_percentile(50.0) as f64,
            results.histogram.value_at_percentile(95.0) as f64,
            results.histogram.value_at_percentile(99.0) as f64,
            results.histogram.value_at_percentile(99.9) as f64,
            results.histogram.max() as f64,
        )
    } else {
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
    };

    println!("{{");
    println!("  \"target\": \"{}\",", config.nameserver);
    println!("  \"hostname\": \"{}\",", config.host);
    println!("  \"record_type\": \"{}\",", config.record_type);
    println!("  \"protocol\": \"{}\",", config.protocol);
    println!("  \"workers\": {},", config.workers);
    println!("  \"duration_secs\": {},", duration_secs);
    println!("  \"warmup_secs\": {},", config.warmup.as_secs());
    println!("  \"queries\": {{");
    println!("    \"total\": {},", total);
    println!("    \"successful\": {},", successes);
    println!("    \"failed\": {},", failures);
    println!("    \"qps\": {}", qps);
    println!("  }},");
    println!("  \"latency_us\": {{");
    println!("    \"mean\": {:.1},", mean);
    println!("    \"min\": {:.1},", min);
    println!("    \"p50\": {:.1},", p50);
    println!("    \"p95\": {:.1},", p95);
    println!("    \"p99\": {:.1},", p99);
    println!("    \"p99_9\": {:.1},", p999);
    println!("    \"max\": {:.1}", max);
    println!("  }},");
    println!("  \"errors\": {{");
    println!("    \"timeouts\": {},", e.timeouts);
    println!("    \"servfail\": {},", e.servfail);
    println!("    \"nxdomain\": {},", e.nxdomain);
    println!("    \"refused\": {},", e.refused);
    println!("    \"formerr\": {},", e.formerr);
    println!("    \"network\": {},", e.network_errors);
    println!("    \"other\": {}", e.other);
    println!("  }},");
    println!("  \"success_rate\": {:.4}", success_rate);
    println!("}}");
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

fn format_latency(us: f64) -> String {
    if us < 1000.0 {
        format!("{:.1}\u{00b5}s", us)
    } else if us < 1_000_000.0 {
        format!("{:.2}ms", us / 1000.0)
    } else {
        format!("{:.2}s", us / 1_000_000.0)
    }
}
