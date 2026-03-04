use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;
use tokio::time::timeout;

use crate::config::{BenchConfig, Protocol};
use crate::dns::{self, DnsError};
use crate::metrics::{LiveCounters, WorkerMetrics};

pub async fn worker(
    config: Arc<BenchConfig>,
    live: Arc<LiveCounters>,
    finished: Arc<AtomicBool>,
    warmup_deadline: Instant,
) -> WorkerMetrics {
    match config.protocol {
        Protocol::Udp => udp_worker(config, live, finished, warmup_deadline).await,
        Protocol::Tcp => tcp_worker(config, live, finished, warmup_deadline).await,
    }
}

fn record_result(
    result: &Result<(), DnsError>,
    elapsed_us: u64,
    is_warmup: bool,
    metrics: &mut WorkerMetrics,
    live: &LiveCounters,
) {
    match result {
        Ok(()) => {
            if !is_warmup {
                let _ = metrics.histogram.record(elapsed_us);
            }
            live.successes.fetch_add(1, Ordering::Relaxed);
        }
        Err(e) => {
            if !is_warmup {
                match e {
                    DnsError::Timeout => metrics.errors.timeouts += 1,
                    DnsError::ServFail => metrics.errors.servfail += 1,
                    DnsError::NxDomain => metrics.errors.nxdomain += 1,
                    DnsError::Refused => metrics.errors.refused += 1,
                    DnsError::FormErr => metrics.errors.formerr += 1,
                    DnsError::NetworkError(_) => metrics.errors.network_errors += 1,
                    DnsError::Other(_) => metrics.errors.other += 1,
                }
            }
            live.failures.fetch_add(1, Ordering::Relaxed);
        }
    }
}

async fn udp_worker(
    config: Arc<BenchConfig>,
    live: Arc<LiveCounters>,
    finished: Arc<AtomicBool>,
    warmup_deadline: Instant,
) -> WorkerMetrics {
    let mut metrics = WorkerMetrics::new();
    let mut query_buf = dns::build_query_packet(&config.host, config.record_type);
    let mut recv_buf = [0u8; 4096];
    let mut query_id: u16 = 0;

    let socket = UdpSocket::bind("0.0.0.0:0")
        .await
        .expect("failed to bind UDP socket");
    socket
        .connect(config.nameserver)
        .await
        .expect("failed to connect UDP socket");

    while !finished.load(Ordering::Acquire) {
        dns::patch_query_id(&mut query_buf, query_id);
        query_id = query_id.wrapping_add(1);

        let start = Instant::now();
        let result = match socket.send(&query_buf).await {
            Ok(_) => match timeout(config.timeout, socket.recv(&mut recv_buf)).await {
                Ok(Ok(n)) => dns::parse_response(&recv_buf[..n]),
                Ok(Err(e)) => Err(DnsError::NetworkError(e)),
                Err(_) => Err(DnsError::Timeout),
            },
            Err(e) => Err(DnsError::NetworkError(e)),
        };

        let elapsed_us = start.elapsed().as_micros() as u64;
        let is_warmup = Instant::now() < warmup_deadline;
        record_result(&result, elapsed_us, is_warmup, &mut metrics, &live);
    }

    metrics
}

async fn tcp_worker(
    config: Arc<BenchConfig>,
    live: Arc<LiveCounters>,
    finished: Arc<AtomicBool>,
    warmup_deadline: Instant,
) -> WorkerMetrics {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let mut metrics = WorkerMetrics::new();
    let mut query_buf = dns::build_query_packet(&config.host, config.record_type);
    let mut query_id: u16 = 0;

    while !finished.load(Ordering::Acquire) {
        let mut stream = match TcpStream::connect(config.nameserver).await {
            Ok(s) => s,
            Err(e) => {
                let is_warmup = Instant::now() < warmup_deadline;
                record_result(
                    &Err(DnsError::NetworkError(e)),
                    0,
                    is_warmup,
                    &mut metrics,
                    &live,
                );
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };

        while !finished.load(Ordering::Acquire) {
            dns::patch_query_id(&mut query_buf, query_id);
            query_id = query_id.wrapping_add(1);

            let len_bytes = (query_buf.len() as u16).to_be_bytes();
            let start = Instant::now();

            let result = match timeout(config.timeout, async {
                stream.write_all(&len_bytes).await?;
                stream.write_all(&query_buf).await?;

                let mut resp_len_buf = [0u8; 2];
                stream.read_exact(&mut resp_len_buf).await?;
                let resp_len = u16::from_be_bytes(resp_len_buf) as usize;

                let mut resp_buf = vec![0u8; resp_len];
                stream.read_exact(&mut resp_buf).await?;
                Ok::<_, std::io::Error>(resp_buf)
            })
            .await
            {
                Ok(Ok(resp_buf)) => dns::parse_response(&resp_buf),
                Ok(Err(e)) => Err(DnsError::NetworkError(e)),
                Err(_) => Err(DnsError::Timeout),
            };

            let elapsed_us = start.elapsed().as_micros() as u64;
            let is_warmup = Instant::now() < warmup_deadline;
            let is_err = result.is_err();
            record_result(&result, elapsed_us, is_warmup, &mut metrics, &live);

            if is_err {
                break; // drop stream, reconnect via outer loop
            }
        }
    }

    metrics
}
