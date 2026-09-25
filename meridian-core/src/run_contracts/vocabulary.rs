//! The closed run vocabularies shared by `execution-state-model`,
//! `role-and-human-control` and `bounded-context-manifest`: the ordered
//! lifecycle, the work-status pool, the seven universal roles, the
//! switchable supervision modes and the communication modes. Each is an
//! enum with one `as_str`/`parse` pair — the ONE production owner of these
//! pools (`governance/plans/meridian-rust-migration-program-plan.md`
//! §5.19.4 point 4); no consumer compares against a second string list.

use core::fmt;

macro_rules! closed_vocabulary {
    (
        $(#[$meta:meta])*
        $name:ident { $($variant:ident => $text:literal),+ $(,)? }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            /// Every member, in the contract's own declaration order.
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// The literal the contract uses for this member.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $text),+
                }
            }

            /// The member named by `value`, or `None` outside the closed pool.
            pub fn parse(value: &str) -> Option<$name> {
                match value {
                    $($text => Some($name::$variant),)+
                    _ => None,
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

closed_vocabulary! {
    /// The closed, ORDERED lifecycle of one run (`execution-state-model.md`).
    /// Declaration order is lifecycle order: [`LifecycleStage::ordinal`] and
    /// the derived `Ord` both follow it.
    LifecycleStage {
        Intake => "intake",
        Classification => "classification",
        NormResolution => "norm_resolution",
        Planning => "planning",
        Execution => "execution",
        Verification => "verification",
        Acceptance => "acceptance",
        Integration => "integration",
        Deployment => "deployment",
        Observation => "observation",
        Completion => "completion",
    }
}

impl LifecycleStage {
    /// Zero-based position in the lifecycle.
    pub const fn ordinal(self) -> usize {
        // Fieldless enum with implicit discriminants in declaration order.
        self as usize
    }

    /// The stages strictly between `from` and `to` (exclusive both ends),
    /// in lifecycle order — empty unless `to` is at least two stages
    /// after `from`.
    pub fn between(from: LifecycleStage, to: LifecycleStage) -> &'static [LifecycleStage] {
        let (a, b) = (from.ordinal(), to.ordinal());
        if b > a + 1 {
            &Self::ALL[a + 1..b]
        } else {
            &[]
        }
    }
}

closed_vocabulary! {
    /// The closed work-status pool of one run.
    WorkStatus {
        Planned => "planned",
        Ready => "ready",
        Active => "active",
        WaitingHuman => "waiting_human",
        Blocked => "blocked",
        Failed => "failed",
        Completed => "completed",
        Cancelled => "cancelled",
    }
}

impl WorkStatus {
    /// `completed` and `cancelled` are terminal: a later transition needs an
    /// explicit reopen basis and the run carries no next executable step.
    pub fn is_terminal(self) -> bool {
        matches!(self, WorkStatus::Completed | WorkStatus::Cancelled)
    }
}

closed_vocabulary! {
    /// The seven universal roles (`operating-glossary.md`, `role`): a set of
    /// powers and duties, never a participant, a programme, an AI model or a
    /// vendor. One participant may hold several.
    UniversalRole {
        Owner => "owner",
        Operator => "operator",
        Executor => "executor",
        Reviewer => "reviewer",
        Verifier => "verifier",
        GitIntegrator => "git_integrator",
        Deployer => "deployer",
    }
}

impl UniversalRole {
    /// The role that holds human-in-command.
    pub const HUMAN_AUTHORITY: UniversalRole = UniversalRole::Owner;

    /// Roles that make an actor a suitable INDEPENDENT reviewer.
    pub const INDEPENDENT_REVIEW: [UniversalRole; 2] =
        [UniversalRole::Reviewer, UniversalRole::Verifier];

    /// `"owner, operator, …"` — the closed pool as the diagnostics name it.
    pub fn pool_text() -> String {
        Self::ALL
            .iter()
            .map(|r| r.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

closed_vocabulary! {
    /// The switchable supervision modes. Human-in-command is NOT a member:
    /// it is the permanent basis on its own axis ([`HUMAN_AUTHORITY_POSTURE`]).
    SupervisionMode {
        HumanInTheLoop => "human-in-the-loop",
        HumanOnTheLoop => "human-on-the-loop",
    }
}

closed_vocabulary! {
    /// How the acting participant communicates with the owner.
    CommunicationMode {
        OwnerRelayed => "owner_relayed",
        Direct => "direct",
    }
}

/// The permanent human command posture — a constant, not a choice.
pub const HUMAN_AUTHORITY_POSTURE: &str = "human-in-command";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_contracts_lifecycle_is_ordered_and_closed() {
        assert_eq!(LifecycleStage::ALL.len(), 11);
        assert_eq!(LifecycleStage::Intake.ordinal(), 0);
        assert_eq!(LifecycleStage::Completion.ordinal(), 10);
        assert!(LifecycleStage::Planning < LifecycleStage::Execution);
        assert_eq!(
            LifecycleStage::parse("norm_resolution"),
            Some(LifecycleStage::NormResolution)
        );
        assert_eq!(LifecycleStage::parse("bogus"), None);
        assert_eq!(
            LifecycleStage::between(LifecycleStage::Intake, LifecycleStage::Planning),
            &[
                LifecycleStage::Classification,
                LifecycleStage::NormResolution
            ]
        );
        assert!(
            LifecycleStage::between(LifecycleStage::Planning, LifecycleStage::Intake).is_empty()
        );
    }

    #[test]
    fn run_contracts_work_status_terminal_members() {
        let terminal: Vec<_> = WorkStatus::ALL.iter().filter(|s| s.is_terminal()).collect();
        assert_eq!(terminal, [&WorkStatus::Completed, &WorkStatus::Cancelled]);
    }

    #[test]
    fn run_contracts_role_and_mode_pools_are_closed() {
        assert_eq!(UniversalRole::ALL.len(), 7);
        assert_eq!(
            UniversalRole::pool_text(),
            "owner, operator, executor, reviewer, verifier, git_integrator, deployer"
        );
        assert_eq!(SupervisionMode::parse(HUMAN_AUTHORITY_POSTURE), None);
        assert_eq!(
            CommunicationMode::parse("owner_relayed"),
            Some(CommunicationMode::OwnerRelayed)
        );
    }
}
