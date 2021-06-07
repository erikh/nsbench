# nsbench: a tool I cobbled together to load test/benchmark DNS servers

Also a great tutorial on thread-based concurrency in rust!

I don't really guarantee anything about this software. It can generate a lot of load, though! I built it to help me evaluate performance issues with [zeronsd](https://github.com/zerotier/zeronsd).

`nsbench` will allocate N threads (where N defaults to the number of CPUs) to repeatedly hammer your DNS server for a fixed period of time (default 60 seconds). It will periodically report on a number of things, like latency. Once finished, it will offer a detailed report of the run status.

## Output Examples:

```
% ./target/release/nsbench 172.29.194.254 islay.domain -t 10 -l 12
1s latency: 85.576µs | Successes: 253572 | Failures: 0 | Total Req: 253572
1s latency: 95.595µs | Successes: 230841 | Failures: 0 | Total Req: 230841
1s latency: 87.506µs | Successes: 241420 | Failures: 0 | Total Req: 241420
1s latency: 93.204µs | Successes: 251606 | Failures: 0 | Total Req: 251606
1s latency: 98.136µs | Successes: 252773 | Failures: 0 | Total Req: 252773
Nameserver: 172.29.194.254
Host: islay.domain
CPUs Used: 12
Successes: 1319050
Failures: 0
Success Rate: 100.00%
Runtime: 10s
Requests: 131905/s
```

## Installation:

I'll make a crate when it's more useful.

```
cargo install --git https://github.com/erikh/nsbench --branch main
```

## Usage:

```
nsbench <nameserver ip> <host>
```

There are other flags. Use `--help` to access them. As of this writing, that looks like this:

```
Usage: nsbench <nameserver> <host> [-t <time-secs>] [-l <cpus>] [--timeout <timeout>]

Nameserver benchmarking/flooding tool

Options:
  -t, --time-secs   time in seconds to run the test
  -l, --cpus        limit the number of CPUs (default off)
  --timeout         duration to wait (in ns) before considering a request failed
  --help            display usage information
```

## Author

Erik Hollensbe <github@hollensbe.org>
