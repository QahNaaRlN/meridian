//! [`EventSink`] — the port an [`super::ObservedEvent`] is recorded
//! through, and [`NoOpEventSink`], the disabled implementation
//! (`meridian-rust-migration-program-plan.md` §5.4, item 5; §6.4a).

use super::envelope::ObservedEvent;

/// Records an [`ObservedEvent`].
///
/// This trait's signature — `fn record(&self, event: &ObservedEvent)`, no
/// `Result` — means an implementation cannot propagate an error back into
/// the caller's control flow through its return value. That is the entire
/// guarantee the signature itself provides. It does **not** mean every
/// implementation is incapable of affecting execution: a real
/// implementation could still panic, block indefinitely, or mutate shared
/// state a caller later reads. Nothing about this trait prevents that, and
/// no doc comment on this trait should claim otherwise. The specific
/// "changes nothing observable" guarantee this corrective package
/// establishes belongs to [`NoOpEventSink`] below, verified for it alone by
/// `meridian_app::storage::role`'s
/// `a_disabled_event_sink_does_not_change_the_domain_result_or_stored_state`
/// test against a real domain operation — not by this trait's shape, and
/// not by a self-contained test that returns a constant regardless of what
/// it is given.
pub trait EventSink {
    fn record(&self, event: &ObservedEvent);
}

/// The disabled sink: records nothing and has no other effect. This is the
/// one implementation this package actually verifies is inert — see the
/// trait documentation above for where and how.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpEventSink;

impl EventSink for NoOpEventSink {
    fn record(&self, _event: &ObservedEvent) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use meridian_core::types::NonEmptyString;

    use super::super::envelope::EventKind;

    fn sample_event() -> ObservedEvent {
        ObservedEvent::new(
            EventKind::Check,
            NonEmptyString::new("ran cargo test").unwrap(),
        )
    }

    #[test]
    fn no_op_sink_can_be_called_without_panicking() {
        let sink = NoOpEventSink;
        sink.record(&sample_event());
        // Nothing to assert about state — there is none. Reaching this
        // point without a panic is the whole test. The stronger claim
        // ("does not change a real domain operation's result or state") is
        // proved separately, against a real operation — see the trait
        // documentation above.
    }
}
