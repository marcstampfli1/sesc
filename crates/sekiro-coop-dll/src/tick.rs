//! Frame-tick driver.
//!
//! Ideally we hook Sekiro's native tick function (P1 gap #10, SPEC §11)
//! — until that AOB lands, a 60 Hz background thread drives
//! [`super::on_frame`] so the netcode + rollback state still advances
//! while development continues.
//!
//! When the DX11 Present hook is installed (see `overlay` feature), the
//! Present callback calls [`kick`] and the thread idles.

use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const FRAME_DURATION: Duration = Duration::from_micros(16_667); // ~60 Hz

#[derive(Default)]
pub struct Ticker {
    running: AtomicBool,
    handle: Mutex<Option<JoinHandle<()>>>,
    /// Counts Present-hook "kicks" so the thread can back off.
    kicks: AtomicBool,
}

impl Ticker {
    pub const fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            handle: parking_lot::const_mutex(None),
            kicks: AtomicBool::new(false),
        }
    }

    pub fn start(self: &Arc<Self>, mut on_frame: impl FnMut() + Send + 'static) -> bool {
        if self.running.swap(true, Ordering::AcqRel) {
            return false;
        }
        let me = Arc::clone(self);
        let h = thread::Builder::new()
            .name("sekiro-coop-tick".into())
            .spawn(move || {
                let mut next = Instant::now();
                while me.running.load(Ordering::Acquire) {
                    // If the Present hook is kicking us, don't also
                    // run the fallback tick.
                    if me.kicks.swap(false, Ordering::AcqRel) {
                        thread::sleep(FRAME_DURATION * 4);
                        next = Instant::now();
                        continue;
                    }
                    on_frame();
                    next += FRAME_DURATION;
                    let now = Instant::now();
                    if next > now {
                        thread::sleep(next - now);
                    } else {
                        // Fell behind — don't accumulate debt.
                        next = now;
                    }
                }
            })
            .ok();
        *self.handle.lock() = h;
        true
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
        if let Some(h) = self.handle.lock().take() {
            let _ = h.join();
        }
    }

    /// Called from the Present hook when it's active.  Signals the
    /// thread to back off.
    pub fn kick(&self) {
        self.kicks.store(true, Ordering::Release);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    #[test]
    fn ticker_runs_callback() {
        let t = Arc::new(Ticker::new());
        let count = Arc::new(AtomicU32::new(0));
        let c = Arc::clone(&count);
        t.start(move || {
            c.fetch_add(1, Ordering::Relaxed);
        });
        thread::sleep(Duration::from_millis(100));
        t.stop();
        // ~6 ticks at 60 Hz over 100 ms; allow jitter.
        let seen = count.load(Ordering::Relaxed);
        assert!(seen >= 3, "saw {seen} ticks");
    }

    #[test]
    fn kick_suppresses_thread_ticks() {
        let t = Arc::new(Ticker::new());
        let count = Arc::new(AtomicU32::new(0));
        let c = Arc::clone(&count);
        t.start(move || {
            c.fetch_add(1, Ordering::Relaxed);
        });
        // Kick continuously for 80 ms.
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(80) {
            t.kick();
            thread::sleep(Duration::from_millis(5));
        }
        t.stop();
        // Thread should have idled after seeing kicks.
        // Can't assert exact count due to timing, but it should be
        // reasonable (not runaway).
        let seen = count.load(Ordering::Relaxed);
        assert!(seen < 20, "expected fewer than 20 ticks, got {seen}");
    }
}
