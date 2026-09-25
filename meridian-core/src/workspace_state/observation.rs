//! M-18 — one observation of a gate run (`gate-run-observation`): what the
//! validator concluded, when, and against which Kernel. The same type
//! describes the observations the migration imported from the reference's
//! run log and the ones `meridian validate --log-metrics` appends.

use core::fmt;

use super::timestamp::Timestamp;

/// The most messages of each level one observation keeps — the reference's
/// own bound.
pub const MAX_MESSAGES: usize = 20;

/// Why an observation is not well formed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationError {
    TooManyMessages {
        level: &'static str,
        count: usize,
    },
    MoreMessagesThanCounted {
        level: &'static str,
        messages: usize,
        counted: u64,
    },
    EmptyMessage {
        level: &'static str,
    },
}

impl fmt::Display for ObservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObservationError::TooManyMessages { level, count } => write!(
                f,
                "{level}_messages keeps {count} messages; an observation keeps at most {MAX_MESSAGES}"
            ),
            ObservationError::MoreMessagesThanCounted {
                level,
                messages,
                counted,
            } => write!(
                f,
                "{level}_messages keeps {messages} messages but the run counted {counted}"
            ),
            ObservationError::EmptyMessage { level } => {
                write!(f, "{level}_messages carries an empty message")
            }
        }
    }
}

impl std::error::Error for ObservationError {}

/// Counters of one run; `None` where the run's summary was not available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunCounts {
    pub failing: Option<u64>,
    pub warnings: Option<u64>,
    pub info_ok: Option<u64>,
}

/// One well-formed gate-run observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateRunObservation {
    ts: Timestamp,
    kernel_version: Option<String>,
    kernel_revision: Option<String>,
    instance_revision: Option<String>,
    exit_code: i32,
    counts: RunCounts,
    fail_messages: Vec<String>,
    warn_messages: Vec<String>,
}

fn check_messages(
    level: &'static str,
    messages: &[String],
    counted: Option<u64>,
) -> Result<(), ObservationError> {
    if messages.len() > MAX_MESSAGES {
        return Err(ObservationError::TooManyMessages {
            level,
            count: messages.len(),
        });
    }
    if messages.iter().any(|m| m.trim().is_empty()) {
        return Err(ObservationError::EmptyMessage { level });
    }
    if let Some(counted) = counted {
        if messages.len() as u64 > counted {
            return Err(ObservationError::MoreMessagesThanCounted {
                level,
                messages: messages.len(),
                counted,
            });
        }
    }
    Ok(())
}

impl GateRunObservation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ts: Timestamp,
        kernel_version: Option<String>,
        kernel_revision: Option<String>,
        instance_revision: Option<String>,
        exit_code: i32,
        counts: RunCounts,
        fail_messages: Vec<String>,
        warn_messages: Vec<String>,
    ) -> Result<Self, ObservationError> {
        check_messages("fail", &fail_messages, counts.failing)?;
        check_messages("warn", &warn_messages, counts.warnings)?;
        Ok(Self {
            ts,
            kernel_version,
            kernel_revision,
            instance_revision,
            exit_code,
            counts,
            fail_messages,
            warn_messages,
        })
    }

    /// The observation of a run that just finished: every message list is
    /// sorted and cut to [`MAX_MESSAGES`], the counters are the full ones.
    pub fn of_run(
        ts: Timestamp,
        kernel_version: String,
        kernel_revision: Option<String>,
        exit_code: i32,
        failures: &[String],
        warnings: &[String],
        infos: usize,
    ) -> Result<Self, ObservationError> {
        let bounded = |messages: &[String]| -> Vec<String> {
            let mut sorted = messages.to_vec();
            sorted.sort();
            sorted.truncate(MAX_MESSAGES);
            sorted
        };
        Self::new(
            ts,
            Some(kernel_version),
            kernel_revision,
            None,
            exit_code,
            RunCounts {
                failing: Some(failures.len() as u64),
                warnings: Some(warnings.len() as u64),
                info_ok: Some(infos as u64),
            },
            bounded(failures),
            bounded(warnings),
        )
    }

    pub fn ts(&self) -> Timestamp {
        self.ts
    }
    pub fn kernel_version(&self) -> Option<&str> {
        self.kernel_version.as_deref()
    }
    pub fn kernel_revision(&self) -> Option<&str> {
        self.kernel_revision.as_deref()
    }
    pub fn instance_revision(&self) -> Option<&str> {
        self.instance_revision.as_deref()
    }
    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }
    pub fn counts(&self) -> RunCounts {
        self.counts
    }
    pub fn fail_messages(&self) -> &[String] {
        &self.fail_messages
    }
    pub fn warn_messages(&self) -> &[String] {
        &self.warn_messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_state_observation_of_a_run_bounds_and_sorts_its_messages() {
        let failures: Vec<String> = (0..25).rev().map(|i| format!("f{i:02}")).collect();
        let observation = GateRunObservation::of_run(
            Timestamp::from_unix_millis(0),
            "0.5.0".to_string(),
            None,
            1,
            &failures,
            &[],
            3,
        )
        .unwrap();
        assert_eq!(observation.fail_messages().len(), MAX_MESSAGES);
        assert_eq!(observation.fail_messages()[0], "f00");
        assert_eq!(observation.counts().failing, Some(25));
        assert_eq!(observation.counts().info_ok, Some(3));
    }

    #[test]
    fn workspace_state_observation_refuses_an_inconsistent_record() {
        let counts = RunCounts {
            failing: Some(0),
            warnings: None,
            info_ok: None,
        };
        let error = GateRunObservation::new(
            Timestamp::from_unix_millis(0),
            None,
            None,
            None,
            0,
            counts,
            vec!["x".to_string()],
            vec![],
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "fail_messages keeps 1 messages but the run counted 0"
        );
        let too_many = GateRunObservation::new(
            Timestamp::from_unix_millis(0),
            None,
            None,
            None,
            0,
            RunCounts {
                failing: None,
                warnings: None,
                info_ok: None,
            },
            vec![],
            vec!["w".to_string(); 21],
        );
        assert!(matches!(
            too_many,
            Err(ObservationError::TooManyMessages {
                level: "warn",
                count: 21
            })
        ));
    }
}
