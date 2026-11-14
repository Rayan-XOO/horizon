//! A line in the log every few seconds while a world is open (Android): frames per second, the
//! average and worst frame, the chunks waiting to be meshed and drawn, the memory the game holds,
//! and how hot the phone runs (the system's thermal status, the battery's temperature, the
//! clocks the cores run at). A report of a game that slows down after a while then shows why:
//! the phone throttling, chunks piling up, or memory growing.
use std::time::{Duration, Instant};

/// How often a line is written.
const EVERY: Duration = Duration::from_secs(5);

#[derive(Default)]
pub(super) struct PerfLog {
    since: Option<Instant>,
    frames: u32,
    total: Duration,
    worst: Duration,
}

impl PerfLog {
    /// One frame of `elapsed`; `Some` with the window's numbers when a line is due.
    fn frame(&mut self, elapsed: Duration) -> Option<(f32, f32, f32)> {
        let now = Instant::now();
        let since = *self.since.get_or_insert(now);
        self.frames += 1;
        self.total += elapsed;
        self.worst = self.worst.max(elapsed);
        let span = now.duration_since(since);
        if span < EVERY {
            return None;
        }
        let fps = self.frames as f32 / span.as_secs_f32();
        let avg = self.total.as_secs_f32() * 1000.0 / self.frames.max(1) as f32;
        let worst = self.worst.as_secs_f32() * 1000.0;
        *self = Self { since: Some(now), ..Self::default() };
        Some((fps, avg, worst))
    }
}

/// The game's resident memory in MiB (`/proc/self/statm`).
fn resident_mib() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: u64 = text.split_whitespace().nth(1)?.parse().ok()?;
    Some(pages * 4096 / (1024 * 1024))
}

/// Each core's current clock in MHz, the slowest and the fastest (`scaling_cur_freq`).
fn core_clocks() -> Option<(u64, u64)> {
    let clocks: Vec<u64> = (0..16)
        .filter_map(|cpu| {
            let path = format!("/sys/devices/system/cpu/cpu{cpu}/cpufreq/scaling_cur_freq");
            std::fs::read_to_string(path).ok()?.trim().parse::<u64>().ok()
        })
        .map(|khz| khz / 1000)
        .collect();
    Some((*clocks.iter().min()?, *clocks.iter().max()?))
}

impl super::Game<'_> {
    /// Once a frame, with the time since the last one.
    pub(super) fn perf_log_frame(&mut self, elapsed: Duration) {
        if !cfg!(target_os = "android") || self.player.is_none() {
            return;
        }
        let Some((fps, avg, worst)) = self.perf_log.frame(elapsed) else {
            return;
        };
        let drawn = self.renderer.as_ref().map_or(0, |r| r.drawn_sections);
        let memory = resident_mib().map_or("?".to_owned(), |m| format!("{m} MiB"));
        let clocks = core_clocks().map_or("?".to_owned(), |(lo, hi)| format!("{lo}-{hi} MHz"));
        #[cfg(target_os = "android")]
        let (thermal, battery) = super::android_ime::thermal();
        #[cfg(not(target_os = "android"))]
        let (thermal, battery) = (-1, -1);
        let thermal = match thermal {
            0 => "none",
            1 => "light",
            2 => "moderate",
            3 => "severe",
            4 => "critical",
            5 => "emergency",
            6 => "shutdown",
            _ => "?",
        };
        let battery = if battery >= 0 { format!("{:.1} C", battery as f32 / 10.0) } else { "?".to_owned() };
        log::info!(
            "Horizon perf: {fps:.1} fps, frame {avg:.1} ms avg {worst:.0} ms worst, chunks to mesh {}, sections drawn {drawn}, render distance {}, memory {memory}, thermal {thermal}, battery {battery}, cores {clocks}",
            self.streaming.pending(),
            self.settings.render_distance,
        );
    }
}
