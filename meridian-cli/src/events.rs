//! Composition-root wiring for [`EventSink`] (`meridian-rust-migration-program-plan.md`
//! §6.4b).
//!
//! The production binary (`main` (the binary crate)) always composes commands with
//! [`NoOpEventSink`] — this package introduces no consumer of observed
//! events and no CLI surface to name a different one (that would be
//! functionality this package does not own). What this module adds is the
//! generic wiring itself: every command function in [`crate::commands`]
//! takes `&dyn EventSink`, never a concrete type, and a second
//! implementation exists here purely so a test can prove swapping it for
//! [`NoOpEventSink`] changes nothing observable — stdout, stderr, exit code,
//! domain result or stored state (the package-7 gate, stronger than the
//! `meridian-app`-level proof `knowledge-agent-foundation` already
//! established: this one is checked at the process-output boundary, not
//! only the in-memory domain boundary).

pub use meridian_app::events::{EventSink, NoOpEventSink};

use meridian_app::events::ObservedEvent;
use std::sync::Mutex;

/// A real, non-empty [`EventSink`]: records every event in memory. It
/// exists only so tests can compare a command's observable output (stdout,
/// stderr, exit code) and its resulting on-disk state between a run that
/// calls this sink and a run that calls [`NoOpEventSink`] — never wired into
/// the production binary, and never printed to stdout or stderr itself.
#[derive(Default)]
pub struct RecordingEventSink {
    events: Mutex<Vec<ObservedEvent>>,
}

impl RecordingEventSink {
    pub fn new() -> Self {
        Self::default()
    }

    /// The events recorded so far, oldest first.
    pub fn recorded(&self) -> Vec<ObservedEvent> {
        self.events
            .lock()
            .expect("recording sink mutex poisoned")
            .clone()
    }
}

impl EventSink for RecordingEventSink {
    fn record(&self, event: &ObservedEvent) {
        self.events
            .lock()
            .expect("recording sink mutex poisoned")
            .push(event.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meridian_app::events::EventKind;
    use meridian_core::types::NonEmptyString;

    #[test]
    fn recording_sink_actually_records() {
        let sink = RecordingEventSink::new();
        sink.record(&ObservedEvent::new(
            EventKind::Check,
            NonEmptyString::new("ran a check").unwrap(),
        ));
        assert_eq!(sink.recorded().len(), 1);
    }
}
