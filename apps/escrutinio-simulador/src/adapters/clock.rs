//! El reloj real del puerto `Pacer`: duerme hasta el momento de cada mesa.

use std::thread;
use std::time::{Duration, Instant};

use crate::application::Pacer;

pub struct SleepPacer {
    start: Instant,
}

impl SleepPacer {
    /// Cuenta los momentos desde ahora.
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

impl Pacer for SleepPacer {
    fn wait_until(&mut self, at: Duration) {
        if let Some(rest) = at.checked_sub(self.start.elapsed()) {
            thread::sleep(rest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_until_the_moment_and_not_when_it_has_passed() {
        let mut pacer = SleepPacer::start();
        pacer.wait_until(Duration::from_millis(20));
        assert!(pacer.elapsed() >= Duration::from_millis(20));
        let before = pacer.elapsed();
        pacer.wait_until(Duration::ZERO);
        assert!(pacer.elapsed() - before < Duration::from_millis(20));
    }
}
