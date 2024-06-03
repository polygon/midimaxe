use std::time::{Duration, Instant};
use once_cell::sync::Lazy;

/* Implements a way to measure time with Durations instead of Instant
 * so that we can get a serializable clock with a defined start.
 * The "now" function returns a Duration since the clock was generated.
 * Usually meant to be created once as a global static variable that is
 * then only read */
struct ProgramClock(Instant);

#[derive(Clone, Copy, Debug)]
pub struct ProgramTime(pub Duration);

impl ProgramClock {
    fn new() -> ProgramClock {
        ProgramClock(Instant::now())
    }

    pub fn now(&self) -> ProgramTime {
        ProgramTime(self.0.elapsed())
    }
}

static PROGRAM_CLOCK: Lazy<ProgramClock> = Lazy::new(|| ProgramClock::new());

pub fn now() -> ProgramTime {
    PROGRAM_CLOCK.now()
}