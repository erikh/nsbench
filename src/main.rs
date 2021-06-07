use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::{
        atomic::AtomicBool,
        mpsc::{sync_channel, SyncSender},
        Arc,
    },
    time::Duration,
};

use trust_dns_resolver::{
    config::{NameServerConfig, ResolverConfig, ResolverOpts},
    Name, Resolver,
};

struct RunDetails {
    successes: u64,
    failures: u64,
}

fn perform_queries(
    ch: SyncSender<RunDetails>,
    finished: Arc<AtomicBool>,
    nameserver: IpAddr,
    host: Name,
) {
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

    let resolver = Resolver::new(resolver_config, opts).unwrap();

    let mut details = RunDetails {
        successes: 0,
        failures: 0,
    };

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

fn main() {
    let concurrency_factor = 100;
    let time_secs = 60;
    let cpus = num_cpus::get_physical();
    let thread_count = cpus * concurrency_factor;
    let mut handles = Vec::new();
    let (s, r) = sync_channel(thread_count);
    let finished = Arc::new(AtomicBool::new(false));

    let nameserver = IpAddr::from_str("10.147.19.234").expect("cannot parse nameserver IP");
    // let nameserver = IpAddr::from_str("10.0.0.5").expect("cannot parse nameserver IP");
    let host = Name::from_str("seafile.home").expect("Cannot parse DNS name");

    for _ in 0..thread_count {
        let s = s.clone();
        let finished = finished.clone();
        let nameserver = nameserver.clone();
        let host = host.clone();

        handles.push(std::thread::spawn(move || {
            perform_queries(s, finished, nameserver, host)
        }));
    }

    std::thread::sleep(Duration::new(time_secs, 0));
    finished.store(true, std::sync::atomic::Ordering::Relaxed);

    let mut overall = RunDetails {
        successes: 0,
        failures: 0,
    };

    for _ in 0..thread_count {
        if let Ok(details) = r.recv() {
            overall.successes += details.successes;
            overall.failures += details.failures;
        } else {
            break;
        }
    }

    for handle in handles {
        handle.join().unwrap()
    }

    println!("Nameserver: {}", nameserver);
    println!("Host: {}", host);
    println!(
        "Successes: {}, Failures: {}",
        overall.successes, overall.failures
    );
    println!(
        "Success Rate: {:.02}%",
        (overall.successes as f64 / (overall.successes + overall.failures) as f64) * 100.0,
    );
    println!("Runtime: {}s", time_secs);
    println!("Requests: {}/s", overall.successes / time_secs);
}
