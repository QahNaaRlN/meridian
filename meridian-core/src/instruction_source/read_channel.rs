//! [`ReadChannel`] — how a registered instruction source is actually read
//! (`instruction-source-registry.md`) — and the coherence rule between its
//! `kind`/`meridian_visibility`/`agent_auto_read` fields, extracted as a
//! canonical, typed, reusable piece
//! (`rust-architecture-conformance-1` corrective round item 2).
//!
//! This is the ONE copy of the coherence rule: an agent-native channel is
//! defined by the agent's own tooling ingesting the source itself
//! (`agent_auto_read` true), Meridian cannot fully observe a source read
//! that way, and full `meridian_visibility` is possible only on the
//! `meridian-observed` channel. `meridian_app::operating_model::controlled_rule_intake`
//! builds a [`ReadChannel`] through [`ReadChannel::try_new`] directly from
//! a resolved external response's already-typed fields — never on a
//! `serde_json::Value` round-tripped back from a DTO.
//! `meridian_app::operating_model::instruction_source_registry`'s own
//! `check_read_channel` DELEGATES to [`ReadChannel::try_new`] (third
//! corrective round, item 5; fourth corrective round, item 3.4) rather than
//! carrying a second, divergent copy of the rule text — it stays
//! Value-based only at its own transport boundary (parsing
//! `kind`/`meridian_visibility`/`agent_auto_read` out of a `Value`), out of
//! this pilot's scope to convert further.
//!
//! The coherence rule text itself is a private implementation detail of
//! `try_new` (fourth corrective round, item 3.4): every `ReadChannel` any
//! caller — in this crate or any other — can ever hold has already passed
//! it, so a public standalone checker taking an already-built `ReadChannel`
//! would always return empty and would be a misleading public surface, not
//! a real one. There is no other public constructor to bypass the rule
//! with.

use core::fmt;

use crate::types::Diagnostic;

use super::fail;

/// The closed `read_channel.kind` pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReadChannelKind {
    MeridianObserved,
    AgentNative,
    Manual,
}

impl ReadChannelKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ReadChannelKind::MeridianObserved => "meridian-observed",
            ReadChannelKind::AgentNative => "agent-native",
            ReadChannelKind::Manual => "manual",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "meridian-observed" => Some(ReadChannelKind::MeridianObserved),
            "agent-native" => Some(ReadChannelKind::AgentNative),
            "manual" => Some(ReadChannelKind::Manual),
            _ => None,
        }
    }
}

impl fmt::Display for ReadChannelKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The closed `read_channel.meridian_visibility` pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MeridianVisibility {
    Full,
    Partial,
    None,
}

impl MeridianVisibility {
    pub fn as_str(self) -> &'static str {
        match self {
            MeridianVisibility::Full => "full",
            MeridianVisibility::Partial => "partial",
            MeridianVisibility::None => "none",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "full" => Some(MeridianVisibility::Full),
            "partial" => Some(MeridianVisibility::Partial),
            "none" => Some(MeridianVisibility::None),
            _ => None,
        }
    }
}

impl fmt::Display for MeridianVisibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a registered instruction source is actually read: a closed `kind`
/// and `meridian_visibility`, plus whether the agent's own tooling reads it
/// automatically.
///
/// The ONLY way to construct a value of this type is [`ReadChannel::try_new`]
/// (`rust-architecture-conformance-1`, third corrective round, item 4): it
/// runs the coherence rule between the three fields before allowing
/// construction, so an incoherent channel (for example `kind: agent-native`
/// with `agent_auto_read: false`) is unrepresentable, not merely flagged
/// after the fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReadChannel {
    kind: ReadChannelKind,
    meridian_visibility: MeridianVisibility,
    agent_auto_read: bool,
}

impl ReadChannel {
    /// Validates the coherence rule between `kind`/`meridian_visibility`/
    /// `agent_auto_read` — see this module's own doc comment for the rule
    /// itself — and only then builds `Self`. `id` names the instruction
    /// source this channel belongs to, for the diagnostic text only.
    pub fn try_new(
        id: &str,
        kind: ReadChannelKind,
        meridian_visibility: MeridianVisibility,
        agent_auto_read: bool,
    ) -> Result<Self, Vec<Diagnostic>> {
        let candidate = Self {
            kind,
            meridian_visibility,
            agent_auto_read,
        };
        let problems = coherence_problems(id, &candidate);
        if problems.is_empty() {
            Ok(candidate)
        } else {
            Err(problems)
        }
    }

    pub fn kind(&self) -> ReadChannelKind {
        self.kind
    }

    pub fn meridian_visibility(&self) -> MeridianVisibility {
        self.meridian_visibility
    }

    pub fn agent_auto_read(&self) -> bool {
        self.agent_auto_read
    }
}

/// The coherence rule between `kind`/`meridian_visibility`/`agent_auto_read`
/// — see this module's own doc comment for the rule itself. Private
/// (fourth corrective round, item 3.4): the ONLY caller is
/// [`ReadChannel::try_new`], which enforces it before a `ReadChannel` can
/// exist at all. `instruction_source_registry.rs`'s own Value-based
/// `check_read_channel` delegates through `try_new` itself, not this
/// function directly — it never needs the rule text without also building
/// the typed value. `id` names the instruction source this channel belongs
/// to, for the diagnostic text only.
fn coherence_problems(id: &str, rc: &ReadChannel) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let at = format!("instruction source \"{id}\"");
    let auto = rc.agent_auto_read;

    if auto && rc.kind != ReadChannelKind::AgentNative {
        problems.push(fail(format!(
            "{at}: read_channel agent_auto_read is true but kind is \"{}\"; a source the agent's own tooling ingests by itself is the agent-native channel",
            rc.kind
        )));
    }
    if auto && rc.meridian_visibility == MeridianVisibility::Full {
        problems.push(fail(format!(
            "{at}: read_channel agent_auto_read is true with meridian_visibility \"full\"; Meridian cannot fully observe a source the agent's own tool ingests"
        )));
    }
    if rc.kind == ReadChannelKind::AgentNative && !auto {
        problems.push(fail(format!(
            "{at}: read_channel kind is \"agent-native\" but agent_auto_read is not true; the agent-native channel is defined by the agent ingesting the source itself"
        )));
    }
    if rc.kind == ReadChannelKind::MeridianObserved && auto {
        problems.push(fail(format!(
            "{at}: read_channel kind is \"meridian-observed\" but agent_auto_read is true; the meridian-observed channel is Meridian's own read of the source, not the agent's"
        )));
    }
    if rc.meridian_visibility == MeridianVisibility::Full
        && rc.kind != ReadChannelKind::MeridianObserved
    {
        problems.push(fail(format!(
            "{at}: read_channel meridian_visibility is \"full\" but kind is \"{}\"; full visibility of the source is possible only on the meridian-observed channel",
            rc.kind
        )));
    }

    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_coherent_channel_constructs_cleanly_via_try_new() {
        let rc = ReadChannel::try_new(
            "src-1",
            ReadChannelKind::MeridianObserved,
            MeridianVisibility::Full,
            false,
        );
        assert!(rc.is_ok(), "{rc:?}");
    }

    /// Corrective round item 4: an incoherent combination cannot be
    /// constructed at all — `try_new` rejects it, there is no infallible
    /// constructor to bypass the rule.
    #[test]
    fn try_new_rejects_an_incoherent_combination() {
        let err = ReadChannel::try_new(
            "src-1",
            ReadChannelKind::AgentNative,
            MeridianVisibility::Partial,
            false,
        )
        .unwrap_err();
        assert!(
            err.iter()
                .any(|d| d.message().contains("agent_auto_read is not true")),
            "{err:?}"
        );
    }

    /// Fourth corrective round, item 3.4.3: rejection is verified through
    /// the public `try_new`, never by constructing an invalid `ReadChannel`
    /// directly through its private fields.
    #[test]
    fn agent_auto_read_cannot_claim_full_visibility() {
        let err = ReadChannel::try_new(
            "src-1",
            ReadChannelKind::AgentNative,
            MeridianVisibility::Full,
            true,
        )
        .unwrap_err();
        assert!(
            err.iter()
                .any(|d| d.message().contains("Meridian cannot fully observe")),
            "{err:?}"
        );
    }

    #[test]
    fn parse_rejects_values_outside_the_closed_pool() {
        assert_eq!(ReadChannelKind::parse("invented"), None);
        assert_eq!(MeridianVisibility::parse("invented"), None);
    }
}
