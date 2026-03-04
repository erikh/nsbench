# nsbench

A high-performance DNS server benchmarking tool built in Rust.

`nsbench` spawns async [Tokio](https://tokio.rs/) worker tasks to flood a DNS server with queries for a fixed duration, recording per-query latency in an [HdrHistogram](https://docs.rs/hdrhistogram) and categorizing errors. It uses [hickory-proto](https://docs.rs/hickory-proto) for DNS wire protocol over UDP or TCP.

## Installation

```
cargo install nsbench
```

Or from source:

```
cargo install --git https://github.com/erikh/nsbench --branch main
```

## Usage

```
nsbench <nameserver> <host> [options]
```

A bare IP address defaults to port 53. You can also specify a port explicitly (e.g., `8.8.8.8:5353`).

### Options

| Flag | Description | Default |
|------|-------------|---------|
| `-t <secs>` | Duration in seconds | 60 |
| `-w <n>` | Number of worker tasks | CPU count |
| `--warmup <secs>` | Warmup period (excluded from results) | 3 |
| `--timeout <ms>` | Query timeout in milliseconds | 2000 |
| `-r <type>` | Record type: A, AAAA, MX, TXT, CNAME, SRV, NS, SOA | A |
| `-p <proto>` | Protocol: udp, tcp | udp |
| `--json` | Output results as JSON | off |

### Example

```
$ nsbench 8.8.8.8 example.com -t 10 -w 8
[warmup]   1s | qps: 12,340 | errors: 0
[warmup]   2s | qps: 13,002 | errors: 0
[warmup]   3s | qps: 12,870 | errors: 0
        4s | qps: 13,150 | errors: 0
        5s | qps: 12,980 | errors: 0
        6s | qps: 13,210 | errors: 0
        7s | qps: 12,750 | errors: 0
        8s | qps: 13,100 | errors: 0
        9s | qps: 12,900 | errors: 0
       10s | qps: 13,050 | errors: 0
       11s | qps: 12,800 | errors: 0
       12s | qps: 13,000 | errors: 0
       13s | qps: 12,950 | errors: 0
--- nsbench results ---
Target:     8.8.8.8:53
Hostname:   example.com.
Record:     A
Protocol:   UDP
Workers:    8
Duration:   10s (3s warmup excluded)

Queries:
  Total:      129,890
  Successful: 129,890
  Failed:     0
  QPS:        12,989

Latency:
  Mean:       602.3µs    Min:       98.0µs
  p50:        580.0µs    p95:        1.12ms
  p99:         2.45ms    p99.9:      5.80ms
  Max:         8.32ms

Errors:
  Timeouts: 0  SERVFAIL: 0  REFUSED: 0
  NXDOMAIN: 0   Network: 0   Other: 0

Success Rate: 100.0000%
```

## Author

Erik Hollensbe <github@hollensbe.org>

## License

[MIT](LICENSE)
