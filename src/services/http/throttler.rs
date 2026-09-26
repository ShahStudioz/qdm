//! # Download Bandwidth Throttler Module
//!
//! Provides virtual-clock reservation rate limiting to constrain throughput per download
//! across multiple concurrent chunk workers accurately without throughput multiplication.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct Throttler {
    state: Arc<Mutex<ThrottlerState>>,
}

#[derive(Debug)]
struct ThrottlerState {
    rate_bps: u64,
    /// The virtual timeline timestamp when the bandwidth queue becomes free next.
    available_at: Instant,
}

impl Throttler {
    pub fn new(rate_bps: u64) -> Self {
        Self {
            state: Arc::new(Mutex::new(ThrottlerState {
                rate_bps,
                available_at: Instant::now(),
            })),
        }
    }

    /// Dynamically updates the speed limit in bytes per second.
    #[allow(dead_code)]
    pub async fn set_rate_bps(&self, new_rate: u64) {
        let mut s = self.state.lock().await;
        s.rate_bps = new_rate;
        s.available_at = Instant::now();
    }

    /// Asynchronously acquires permission to transfer `bytes` amount of data.
    /// Employs virtual-clock reservation scheduling so that multiple concurrent
    /// chunk streams queue their bandwidth allowances consecutively without multiplying throughput.
    pub async fn acquire(&self, bytes: usize) {
        if bytes == 0 {
            return;
        }

        let sleep_until = {
            let mut state = self.state.lock().await;
            if state.rate_bps == 0 {
                return;
            }

            let now = Instant::now();
            // If the throttler has been idle, reset timeline to `now` to prevent burst buildup
            if now > state.available_at {
                state.available_at = now;
            }

            let my_slot = state.available_at;
            let duration_for_bytes = Duration::from_secs_f64(bytes as f64 / state.rate_bps as f64);
            state.available_at = my_slot + duration_for_bytes;

            my_slot
        };

        let now = Instant::now();
        if sleep_until > now {
            tokio::time::sleep(sleep_until - now).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_throttler_single_stream() {
        let rate_bps = 100_000; // 100 KB/s
        let throttler = Throttler::new(rate_bps);

        let start = Instant::now();
        // 4 chunks of 25,000 bytes = 100,000 bytes total (should take ~0.75 - 1.0s)
        for _ in 0..4 {
            throttler.acquire(25_000).await;
        }
        let elapsed = start.elapsed();
        // First chunk at 0s, 2nd at 0.25s, 3rd at 0.50s, 4th at 0.75s
        assert!(
            elapsed >= Duration::from_millis(700),
            "Elapsed: {:?}",
            elapsed
        );
        assert!(
            elapsed <= Duration::from_millis(1100),
            "Elapsed: {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_throttler_multi_stream_concurrency() {
        let rate_bps = 100_000; // 100 KB/s
        let throttler = Throttler::new(rate_bps);

        let start = Instant::now();
        let mut handles = Vec::new();

        // 4 concurrent tasks, each transferring 25,000 bytes
        for _ in 0..4 {
            let t = throttler.clone();
            handles.push(tokio::spawn(async move {
                t.acquire(25_000).await;
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        let elapsed = start.elapsed();
        // 4 tasks * 25,000 bytes = 100,000 bytes total.
        // Task 1: 0s, Task 2: 0.25s, Task 3: 0.50s, Task 4: 0.75s.
        // The last task finishes at ~0.75s - 0.85s.
        assert!(
            elapsed >= Duration::from_millis(700),
            "Multi-stream elapsed: {:?}",
            elapsed
        );
        assert!(
            elapsed <= Duration::from_millis(1100),
            "Multi-stream elapsed: {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_throttler_zero_rate_unlimited() {
        let throttler = Throttler::new(0);
        let start = Instant::now();
        throttler.acquire(1_000_000).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }
}
