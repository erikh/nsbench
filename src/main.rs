use std::{
    net::{IpAddr, SocketAddr},
    ops::AddAssign,
    sync::{
        atomic::AtomicBool,
        mpsc::{channel, sync_channel, Sender, SyncSender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use trust_dns_resolver::{
    config::{NameServerConfig, ResolverConfig, ResolverOpts},
    Name, Resolver,
};

use argh::FromArgs;

#[derive(Debug, Clone)]
struct QueryConfig {
    init_done: SyncSender<()>,
    informer_sender: Sender<RunDetails>,
    finished: Arc<AtomicBool>,
    nameserver: IpAddr,
    host: Name,
    timeout: Duration,
    lock: Arc<Mutex<()>>,
}

#[derive(Clone, Copy, Debug)]
struct RunDetails {
    successes: u64,
    failures: u64,
    duration: u128,
}

impl RunDetails {
    fn reset(&mut self) {
        self.successes = 0;
        self.failures = 0;
        self.duration = 0;
    }
}

impl Default for RunDetails {
    fn default() -> Self {
        Self {
            successes: 0,
            failures: 0,
            duration: 0,
        }
    }
}

impl AddAssign<RunDetails> for RunDetails {
    fn add_assign(&mut self, rhs: RunDetails) {
        self.duration = (rhs.duration + self.duration) / 2;
        self.successes += rhs.successes;
        self.failures += rhs.failures;
    }
}

fn perform_queries(qc: QueryConfig) {
    let mut resolver_config = ResolverConfig::new();
    resolver_config.add_name_server(NameServerConfig {
        socket_addr: SocketAddr::new(qc.nameserver, 53),
        protocol: trust_dns_resolver::config::Protocol::Udp,
        tls_dns_name: None,
        trust_nx_responses: true,
    });

    let mut opts = ResolverOpts::default();
    opts.rotate = false;
    opts.cache_size = 0;
    opts.timeout = qc.timeout;
    opts.positive_min_ttl = Some(Duration::new(0, 0));
    opts.positive_max_ttl = Some(Duration::new(0, 0));
    opts.negative_min_ttl = Some(Duration::new(0, 0));
    opts.negative_max_ttl = Some(Duration::new(0, 0));

    let resolver = Resolver::new(resolver_config, opts).unwrap();

    let ret = RunDetails::default();
    let details = Arc::new(Mutex::new(ret));

    let informer_details = details.clone();
    let informer_finished_parent = Arc::new(AtomicBool::new(false));
    let informer_finished = informer_finished_parent.clone();
    let informer_sender = qc.informer_sender.clone();

    let informer = thread::spawn(move || {
        let tick = std::time::Duration::new(1, 0);
        while !informer_finished.load(std::sync::atomic::Ordering::Relaxed) {
            thread::sleep(tick);
            let mut details = informer_details.lock().unwrap();
            informer_sender.send(details.clone()).unwrap();
            details.reset();
        }
    });

    qc.init_done.send(()).unwrap();
    drop(qc.lock.lock().unwrap());

    while !qc.finished.load(std::sync::atomic::Ordering::Relaxed) {
        let now = Instant::now();
        if resolver
            .lookup(
                qc.host.clone(),
                trust_dns_resolver::proto::rr::RecordType::A,
            )
            .is_ok()
        {
            let mut writer = details.lock().unwrap();
            writer.successes += 1;
            let previous = writer.duration;
            let current = Instant::now().duration_since(now).as_nanos();
            writer.duration = (current + previous) / 2;
        } else {
            let mut writer = details.lock().unwrap();
            writer.failures += 1
        }
    }

    informer_finished_parent.store(true, std::sync::atomic::Ordering::Relaxed);
    informer.join().unwrap();
}

#[derive(FromArgs, Clone, Debug)]
#[argh(description = "Nameserver benchmarking/flooding tool")]
struct CLIArguments {
    #[argh(
        option,
        short = 't',
        description = "time in seconds to run the test",
        default = "60"
    )]
    time_secs: u64,

    #[argh(
        option,
        short = 'l',
        description = "limit the number of CPUs (default off)",
        default = "num_cpus::get()"
    )]
    cpus: usize,

    #[argh(
        option,
        description = "duration to wait (in ns) before considering a request failed",
        default = "500000"
    )]
    timeout: u32,

    #[argh(positional)]
    nameserver: IpAddr,

    #[argh(positional)]
    host: Name,
}

fn main() {
    let args: CLIArguments = argh::from_env();

    let mut handles = Vec::new();
    let (s, r) = sync_channel(args.cpus);
    let (init_s, init_r) = sync_channel(args.cpus);
    let (inf_s, inf_r) = channel();
    let finished = Arc::new(AtomicBool::new(false));
    let lock = Arc::new(Mutex::new(()));

    let mg = lock.lock().unwrap();

    for _ in 0..args.cpus {
        let qc = QueryConfig {
            init_done: init_s.clone(),
            informer_sender: inf_s.clone(),
            finished: finished.clone(),
            nameserver: args.nameserver.clone(),
            host: args.host.clone(),
            timeout: Duration::new(0, args.timeout),
            lock: lock.clone(),
        };

        handles.push(std::thread::spawn(move || perform_queries(qc)));
    }

    for _ in 0..args.cpus {
        init_r.recv().unwrap();
    }

    let informer = thread::spawn(move || {
        let mut totals = RunDetails::default();
        let mut temp_total = RunDetails::default();
        let mut start = Instant::now();
        while let Ok(details) = inf_r.recv() {
            totals += details;
            temp_total += details;

            if Instant::now().duration_since(start).as_secs() > 1 {
                eprintln!(
                    "1s latency: {:?} | Successes: {} | Failures: {} | Total Req: {}",
                    Duration::from_nanos(temp_total.duration as u64),
                    temp_total.successes,
                    temp_total.failures,
                    temp_total.successes + temp_total.failures,
                );

                start = Instant::now();
                temp_total = RunDetails::default();
            }
        }

        s.send(totals).unwrap()
    });

    drop(mg);

    std::thread::sleep(Duration::new(args.time_secs, 0));
    finished.store(true, std::sync::atomic::Ordering::Release);

    for handle in handles {
        handle.join().unwrap()
    }

    drop(inf_s);
    informer.join().unwrap();

    let overall = r.recv().unwrap();

    println!("Nameserver: {}", args.nameserver);
    println!("Host: {}", args.host);
    println!("CPUs Used: {}", args.cpus);
    println!("Successes: {}", overall.successes);
    println!("Failures: {}", overall.failures);
    println!(
        "Success Rate: {:.02}%",
        (overall.successes as f64 / (overall.successes + overall.failures) as f64) * 100.0,
    );
    println!("Runtime: {}s", args.time_secs);
    println!("Requests: {}/s", overall.successes / args.time_secs);
}
