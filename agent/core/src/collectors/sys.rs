//! `sys` collector: CPU, memory, fixed disks, network, uptime. Values are rounded so that noise
//! does not cause a broadcast on every tick (`Store::set` skips equal values).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use sysinfo::{Disks, Networks, System};

use crate::model::{SysDisk, SysMem, SysNet, SysStats};
use crate::state::Store;

const BYTES_PER_MB: f64 = 1_048_576.0;
const BYTES_PER_GB: f64 = 1_073_741_824.0;
const SLICE: Duration = Duration::from_millis(200);

/// Unrounded measurements.
#[derive(Debug, Clone, PartialEq)]
pub struct RawStats {
    pub cpu_percent: f32,
    pub mem_used_bytes: u64,
    pub mem_total_bytes: u64,
    /// (mount point as reported by the OS, used bytes, total bytes)
    pub disks: Vec<(String, u64, u64)>,
    pub rx_kbps: f64,
    pub tx_kbps: f64,
    pub uptime_sec: u64,
}

pub fn round_stats(raw: &RawStats) -> SysStats {
    let mut disks: Vec<SysDisk> = raw
        .disks
        .iter()
        .map(|(mount, used, total)| SysDisk {
            mount: mount.trim_end_matches(['\\', '/']).to_string(),
            used_gb: (*used as f64 / BYTES_PER_GB).round() as u64,
            total_gb: (*total as f64 / BYTES_PER_GB).round() as u64,
        })
        .collect();
    disks.sort_by(|a, b| a.mount.cmp(&b.mount));
    disks.dedup_by(|a, b| a.mount == b.mount);
    SysStats {
        cpu: raw.cpu_percent.clamp(0.0, 100.0).round() as u32,
        mem: SysMem {
            used_mb: (raw.mem_used_bytes as f64 / BYTES_PER_MB).round() as u64,
            total_mb: (raw.mem_total_bytes as f64 / BYTES_PER_MB).round() as u64,
        },
        disks,
        net: SysNet { rx_kbps: raw.rx_kbps.max(0.0).round() as u64, tx_kbps: raw.tx_kbps.max(0.0).round() as u64 },
        uptime_sec: raw.uptime_sec / 60 * 60,
    }
}

/// A running collector thread; dropped or `stop`ped to end it.
pub struct SysCollector {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl SysCollector {
    pub fn start(store: Store, interval_sec: u64) -> SysCollector {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("decka-sys-collector".to_string())
            .spawn(move || run(store, Duration::from_secs(interval_sec.max(1)), flag))
            .expect("spawn sys collector thread");
        SysCollector { stop, thread: Some(thread) }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for SysCollector {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Sleeps `total` in 200 ms slices; false if the stop flag was raised meanwhile.
fn sleep_unless_stopped(total: Duration, stop: &AtomicBool) -> bool {
    let end = Instant::now() + total;
    while Instant::now() < end {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(SLICE.min(end.saturating_duration_since(Instant::now())));
    }
    !stop.load(Ordering::Relaxed)
}

fn run(store: Store, interval: Duration, stop: Arc<AtomicBool>) {
    let (Ok(mut system), Ok(mut networks), Ok(mut disks)) =
        (System::new(), Networks::new_with_refreshed_list(), Disks::new_with_refreshed_list())
    else {
        tracing::warn!("sys collector: system information is unavailable");
        return;
    };
    system.refresh_cpu_usage(); // the first CPU reading needs a baseline
    let mut last_net = Instant::now();

    while sleep_unless_stopped(interval, &stop) {
        if store.receiver_count() == 0 {
            continue; // nobody is connected: no load
        }
        system.refresh_cpu_usage();
        system.refresh_memory();
        disks.refresh(true);
        networks.refresh(true);

        let elapsed = last_net.elapsed().as_secs_f64().max(0.001);
        last_net = Instant::now();
        let (rx, tx) = networks.values().fold((0u64, 0u64), |(r, t), n| (r + n.received(), t + n.transmitted()));

        let raw = RawStats {
            cpu_percent: system.global_cpu_usage(),
            mem_used_bytes: system.used_memory(),
            mem_total_bytes: system.total_memory(),
            disks: disks
                .iter()
                .filter(|d| !d.is_removable() && d.total_space() > 0)
                .map(|d| (d.mount_point().to_string_lossy().into_owned(), d.total_space() - d.available_space(), d.total_space()))
                .collect(),
            rx_kbps: rx as f64 * 8.0 / 1000.0 / elapsed,
            tx_kbps: tx as f64 * 8.0 / 1000.0 / elapsed,
            uptime_sec: System::uptime().unwrap_or(0),
        };
        store.set("sys", serde_json::to_value(round_stats(&raw)).expect("sys stats serialize"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw() -> RawStats {
        RawStats {
            cpu_percent: 12.6,
            mem_used_bytes: 9_564_856_320,
            mem_total_bytes: 33_554_432_000,
            disks: vec![("D:\\".into(), 1_073_741_824, 2_147_483_648), ("C:\\".into(), 225_485_783_040, 511_101_108_224)],
            rx_kbps: 80.4,
            tx_kbps: 11.6,
            uptime_sec: 125,
        }
    }

    #[test]
    fn cpu_is_rounded_and_clamped() {
        assert_eq!(round_stats(&raw()).cpu, 13);
        assert_eq!(round_stats(&RawStats { cpu_percent: 100.4, ..raw() }).cpu, 100);
        assert_eq!(round_stats(&RawStats { cpu_percent: 130.0, ..raw() }).cpu, 100);
        assert_eq!(round_stats(&RawStats { cpu_percent: -1.0, ..raw() }).cpu, 0);
    }

    #[test]
    fn uptime_is_floored_to_a_minute() {
        assert_eq!(round_stats(&raw()).uptime_sec, 120);
        assert_eq!(round_stats(&RawStats { uptime_sec: 59, ..raw() }).uptime_sec, 0);
    }

    #[test]
    fn disks_lose_the_trailing_backslash_and_are_sorted() {
        let stats = round_stats(&raw());
        let mounts: Vec<&str> = stats.disks.iter().map(|d| d.mount.as_str()).collect();
        assert_eq!(mounts, ["C:", "D:"]);
        assert_eq!(stats.disks[0].used_gb, 210);
        assert_eq!(stats.disks[0].total_gb, 476);
    }

    #[test]
    fn memory_and_network_are_integers() {
        let stats = round_stats(&raw());
        assert_eq!(stats.mem.used_mb, 9122);
        assert_eq!(stats.mem.total_mb, 32000);
        assert_eq!((stats.net.rx_kbps, stats.net.tx_kbps), (80, 12));
    }
}
