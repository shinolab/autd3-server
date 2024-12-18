mod signal;

use std::time::Instant;

pub use signal::Signal;

#[derive(Debug)]
pub enum UserEvent {
    RequestRepaint {
        when: Instant,
        cumulative_pass_nr: u64,
    },
    Server(Signal),
}

pub enum EventResult {
    Wait,
    RepaintNow,
    RepaintNext,
    RepaintAt(Instant),
    Exit,
}
