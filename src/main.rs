use std::{
    net::{IpAddr, SocketAddr},
    sync::{
        atomic::AtomicBool,
        mpsc::{sync_channel, SyncSender},
        Arc, Mutex,
    },
    time::Duration,
};

use trust_dns_resolver::{
    config::{NameServerConfig, ResolverConfig, ResolverOpts},
    Name, Resolver,
};

use argh::FromArgs;

struct RunDetails {
    successes: u64,
    failures: u64,
}

fn perform_queries(
    ch: SyncSender<RunDetails>,
    init_done: SyncSender<()>,
    finished: Arc<AtomicBool>,
    nameserver: IpAddr,
    host: Name,
    timeout: Duration,
    lock: Arc<Mutex<()>>,
) {
    eprintln!("Spawning thread");
    let mut resolver_config = ResolverConfig::new();
    resolver_config.add_name_server(NameServerConfig {
        socket_addr: SocketAddr::new(nameserver, 53),
        protocol: trust_dns_resolver::config::Protocol::Udp,
        tls_dns_name: None,
        trust_nx_responses: true,
    });

    let mut opts = ResolverOpts::default();
    opts.rotate = false;
    opts.cache_size = 0;
    opts.timeout = timeout;

    let resolver = Resolver::new(resolver_config, opts).unwrap();

    let mut details = RunDetails {
        successes: 0,
        failures: 0,
    };

    init_done.send(()).unwrap();
    drop(lock.lock().unwrap());
    eprintln!("Initiating requests");

    while !finished.load(std::sync::atomic::Ordering::Relaxed) {
        if resolver
            .lookup(host.clone(), trust_dns_resolver::proto::rr::RecordType::A)
            .is_ok()
        {
            details.successes += 1
        } else {
            details.failures += 1
        }
    }

    ch.send(details).unwrap()
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
    let finished = Arc::new(AtomicBool::new(false));
    let lock = Arc::new(Mutex::new(()));

    let mg = lock.lock().unwrap();

    for _ in 0..args.cpus {
        let s = s.clone();
        let init_s = init_s.clone();
        let finished = finished.clone();
        let nameserver = args.nameserver.clone();
        let host = args.host.clone();
        let timeout = Duration::new(0, args.timeout);
        let lock = lock.clone();

        handles.push(std::thread::spawn(move || {
            perform_queries(s, init_s, finished, nameserver, host, timeout, lock)
        }));
    }

    for _ in 0..args.cpus {
        init_r.recv().unwrap();
    }

    drop(mg);

    std::thread::sleep(Duration::new(args.time_secs, 0));
    finished.store(true, std::sync::atomic::Ordering::Relaxed);

    let mut overall = RunDetails {
        successes: 0,
        failures: 0,
    };

    for _ in 0..args.cpus {
        if let Ok(details) = r.recv() {
            overall.successes += details.successes;
            overall.failures += details.failures;
        } else {
            eprintln!("Some statistics never made it to the main thread!");
        }
    }

    for handle in handles {
        handle.join().unwrap()
    }

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
