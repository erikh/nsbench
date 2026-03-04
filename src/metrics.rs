use std::sync::atomic::AtomicU64;

use hdrhistogram::Histogram;

pub struct WorkerMetrics {
    pub histogram: Histogram<u64>,
    pub errors: ErrorCounts,
}

impl WorkerMetrics {
    pub fn new() -> Self {
        let mut histogram = Histogram::<u64>::new_with_max(60_000_000, 3)
            .expect("failed to create histogram");
        histogram.auto(true);
        Self {
            histogram,
            errors: ErrorCounts::default(),
        }
    }
}

#[derive(Default, Debug)]
pub struct ErrorCounts {
    pub timeouts: u64,
    pub servfail: u64,
    pub nxdomain: u64,
    pub refused: u64,
    pub formerr: u64,
    pub network_errors: u64,
    pub other: u64,
}

impl ErrorCounts {
    pub fn total(&self) -> u64 {
        self.timeouts
            + self.servfail
            + self.nxdomain
            + self.refused
            + self.formerr
            + self.network_errors
            + self.other
    }
}

pub struct LiveCounters {
    pub successes: AtomicU64,
    pub failures: AtomicU64,
}

impl LiveCounters {
    pub fn new() -> Self {
        Self {
            successes: AtomicU64::new(0),
            failures: AtomicU64::new(0),
        }
    }
}

pub struct MergedResults {
    pub histogram: Histogram<u64>,
    pub errors: ErrorCounts,
}

pub fn merge_worker_results(workers: Vec<WorkerMetrics>) -> MergedResults {
    let mut combined = Histogram::<u64>::new_with_max(60_000_000, 3)
        .expect("failed to create histogram");
    combined.auto(true);
    let mut errors = ErrorCounts::default();

    for w in workers {
        combined
            .add(&w.histogram)
            .expect("failed to merge histogram");
        errors.timeouts += w.errors.timeouts;
        errors.servfail += w.errors.servfail;
        errors.nxdomain += w.errors.nxdomain;
        errors.refused += w.errors.refused;
        errors.formerr += w.errors.formerr;
        errors.network_errors += w.errors.network_errors;
        errors.other += w.errors.other;
    }

    MergedResults {
        histogram: combined,
        errors,
    }
}
