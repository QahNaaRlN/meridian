//! The single production [`Clock`]: the system clock, read once per
//! operation.

use std::time::{SystemTime, UNIX_EPOCH};

use meridian_app::workspace_state::Clock;
use meridian_core::workspace_state::timestamp::Timestamp;

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        // A clock set before 1970 reads as the epoch rather than panicking.
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or(0);
        Timestamp::from_unix_millis(millis)
    }
}
