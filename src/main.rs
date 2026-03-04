mod config;
mod dns;
mod metrics;
mod report;
mod worker;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use argh::FromArgs;
use hickory_proto::rr::{Name, RecordType};

use crate::config::{BenchConfig, Protocol};
use crate::metrics::{merge_worker_results, LiveCounters};

#[derive(FromArgs)]
#[argh(description = "DNS benchmarking tool")]
struct CliArgs {
    #[argh(
        option,
        short = 't',
        description = "time in seconds to run the benchmark",
        default = "60"
    )]
    time_secs: u64,

    #[argh(
        option,
        short = 'w',
        description = "number of worker tasks (default: CPU count)",
        default = "default_workers()"
    )]
    workers: usize,

    #[argh(
        option,
        description = "query timeout in milliseconds",
        default = "2000"
    )]
    timeout: u64,

    #[argh(
        option,
        description = "warmup period in seconds",
        default = "3"
    )]
    warmup: u64,

    #[argh(
        option,
        short = 'r',
        description = "record type: A, AAAA, MX, TXT, CNAME, SRV, NS, SOA",
        default = "\"A\".to_string()"
    )]
    record_type: String,

    #[argh(
        option,
        short = 'p',
        description = "protocol: udp, tcp",
        default = "Protocol::Udp"
    )]
    protocol: Protocol,

    #[argh(switch, description = "output results as JSON")]
    json: bool,

    #[argh(positional, description = "nameserver address (e.g., 8.8.8.8:53)")]
    nameserver: SocketAddr,

    #[argh(positional, description = "hostname to query (e.g., example.com)")]
    host: Name,
}

fn default_workers() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

fn parse_record_type(s: &str) -> RecordType {
    match s.to_uppercase().as_str() {
        "A" => RecordType::A,
        "AAAA" => RecordType::AAAA,
        "MX" => RecordType::MX,
        "TXT" => RecordType::TXT,
        "CNAME" => RecordType::CNAME,
        "SRV" => RecordType::SRV,
        "NS" => RecordType::NS,
        "SOA" => RecordType::SOA,
        _ => {
            eprintln!("unsupported record type: {s}");
            std::process::exit(1);
        }
    }
}

#[tokio::main]
async fn main() {
    let args: CliArgs = argh::from_env();

    let config = Arc::new(BenchConfig {
        nameserver: args.nameserver,
        host: args.host,
        record_type: parse_record_type(&args.record_type),
        protocol: args.protocol,
        duration: Duration::from_secs(args.time_secs),
        timeout: Duration::from_millis(args.timeout),
        workers: args.workers,
        warmup: Duration::from_secs(args.warmup),
        json: args.json,
    });

    let finished = Arc::new(AtomicBool::new(false));
    let live = Arc::new(LiveCounters::new());
    let warmup_deadline = Instant::now() + config.warmup;

    let mut worker_handles = Vec::new();
    for _ in 0..config.workers {
        let config = config.clone();
        let live = live.clone();
        let finished = finished.clone();
        worker_handles.push(tokio::spawn(worker::worker(
            config,
            live,
            finished,
            warmup_deadline,
        )));
    }

    let informer_handle = tokio::spawn(report::informer(
        live.clone(),
        finished.clone(),
        config.warmup,
    ));

    tokio::time::sleep(config.warmup + config.duration).await;
    finished.store(true, Ordering::Release);

    let mut worker_results = Vec::new();
    for handle in worker_handles {
        worker_results.push(handle.await.expect("worker task panicked"));
    }

    informer_handle.abort();
    let _ = informer_handle.await;

    let merged = merge_worker_results(worker_results);

    if config.json {
        report::print_json_report(&config, &merged);
    } else {
        report::print_report(&config, &merged);
    }
}
