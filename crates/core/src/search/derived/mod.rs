//! Derived data jobs (SPEC-0100): the record of each rebuild of data computed
//! from a file (today only its search entries), and the paging marker of
//! `fs.list_derived_jobs`.
//!
//! A job row lives in the admin store (`derived_job`, DEC-004) so it is global
//! to the deployment and goes away with its project through the cascade.

pub mod scheduler;
pub mod store;

pub use scheduler::Scheduler;
pub use store::JobStore;

use crate::errors::{Result, ToolError};
use base64::Engine as _;
use serde_json::{Value, json};
use std::str::FromStr;

/// What a job rebuilds. One value today; later kinds add a variant (DEC-011).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobKind {
    SearchIndex,
}

impl JobKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SearchIndex => "search_index",
        }
    }
}

impl FromStr for JobKind {
    type Err = ToolError;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "search_index" => Ok(Self::SearchIndex),
            _ => Err(ToolError::invalid_argument("kind must be 'search_index'")),
        }
    }
}

/// Where a job is in its life. `Done`, `Failed` and `Superseded` are finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Waiting,
    Running,
    Done,
    Failed,
    Superseded,
}

impl JobState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Superseded => "superseded",
        }
    }

    pub fn is_finished(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Superseded)
    }
}

impl FromStr for JobState {
    type Err = ToolError;
    fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "waiting" => Self::Waiting,
            "running" => Self::Running,
            "done" => Self::Done,
            "failed" => Self::Failed,
            "superseded" => Self::Superseded,
            other => return Err(ToolError::internal(format!("unknown job state '{other}'"))),
        })
    }
}

/// Why a job exists: a change to the file, or a member asking for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobCause {
    Change,
    Manual,
}

impl JobCause {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Change => "change",
            Self::Manual => "manual",
        }
    }
}

impl FromStr for JobCause {
    type Err = ToolError;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "change" => Ok(Self::Change),
            "manual" => Ok(Self::Manual),
            other => Err(ToolError::internal(format!("unknown job cause '{other}'"))),
        }
    }
}

/// One row of `derived_job`. `due` is never part of it: a restart gives every
/// waiting job the full delay again (DEC-005).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub job_id: String,
    pub project_id: String,
    pub path: String,
    pub kind: JobKind,
    pub state: JobState,
    pub cause: JobCause,
    /// A delay 0 waiting entry, not shown to members (DEC-007).
    pub hidden: bool,
    pub created_at: String,
    pub ended_at: Option<String>,
    pub reason: Option<String>,
}

impl Job {
    /// A new waiting job created now, with a fresh uuid v4 id.
    pub fn new_waiting(project_id: &str, path: &str, kind: JobKind, cause: JobCause) -> Self {
        Self {
            job_id: uuid::Uuid::new_v4().to_string(),
            project_id: project_id.to_string(),
            path: path.to_string(),
            kind,
            state: JobState::Waiting,
            cause,
            hidden: false,
            created_at: now_secs(),
            ended_at: None,
            reason: None,
        }
    }

    /// The member facing shape of FR-NEW-010: `seconds_left` only when waiting,
    /// `ended_at` only when finished, `reason` only when failed.
    pub fn to_json(&self, seconds_left: Option<u64>) -> Value {
        let mut v = json!({
            "job_id": self.job_id,
            "path": self.path,
            "kind": self.kind.as_str(),
            "state": self.state.as_str(),
            "cause": self.cause.as_str(),
            "created_at": self.created_at,
        });
        let obj = v.as_object_mut().expect("a json object");
        if self.state == JobState::Waiting {
            obj.insert("seconds_left".into(), json!(seconds_left.unwrap_or(0)));
        }
        if self.state.is_finished()
            && let Some(ended) = &self.ended_at
        {
            obj.insert("ended_at".into(), json!(ended));
        }
        if self.state == JobState::Failed
            && let Some(reason) = &self.reason
        {
            obj.insert("reason".into(), json!(reason));
        }
        v
    }
}

/// RFC 3339 UTC with whole seconds (`2026-11-20T10:01:10Z`), the one time format
/// of a job (DEC-005); its text order is its time order, which the keyset page
/// relies on.
pub fn now_secs() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Where the previous page stopped, in the list order of FR-NEW-010: group 0
/// waiting (by seconds left), 1 running (by creation), 2 finished (newest end
/// first); ties by path then kind (design section 5.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Marker {
    Waiting { seconds_left: u64, path: String, kind: String },
    Running { created_at: String, path: String, kind: String },
    Finished { ended_at: String, path: String, kind: String },
}

impl Marker {
    pub fn group(&self) -> u8 {
        match self {
            Self::Waiting { .. } => 0,
            Self::Running { .. } => 1,
            Self::Finished { .. } => 2,
        }
    }

    /// Base64 url safe without padding of the JSON array `[group, a, b, c]`.
    pub fn encode(&self) -> String {
        let arr = match self {
            Self::Waiting { seconds_left, path, kind } => json!([0, seconds_left, path, kind]),
            Self::Running { created_at, path, kind } => json!([1, created_at, path, kind]),
            Self::Finished { ended_at, path, kind } => json!([2, ended_at, path, kind]),
        };
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(arr.to_string())
    }

    /// Any decoding or shape failure is the one refusal of the contract, so a
    /// member never learns which part of an opaque value was wrong.
    pub fn decode(raw: &str) -> Result<Self> {
        Self::try_decode(raw).ok_or_else(|| ToolError::invalid_argument("marker is not valid"))
    }

    fn try_decode(raw: &str) -> Option<Self> {
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(raw).ok()?;
        let v: Value = serde_json::from_slice(&bytes).ok()?;
        let arr = v.as_array().filter(|a| a.len() == 4)?;
        let path = arr[2].as_str()?.to_string();
        let kind = arr[3].as_str()?.to_string();
        Some(match arr[0].as_u64()? {
            0 => Self::Waiting { seconds_left: arr[1].as_u64()?, path, kind },
            1 => Self::Running { created_at: arr[1].as_str()?.to_string(), path, kind },
            2 => Self::Finished { ended_at: arr[1].as_str()?.to_string(), path, kind },
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marker_round_trips_in_every_group() {
        for m in [
            Marker::Waiting { seconds_left: 3, path: "/a".into(), kind: "search_index".into() },
            Marker::Running {
                created_at: "2026-11-20T10:01:10Z".into(),
                path: "/b".into(),
                kind: "search_index".into(),
            },
            Marker::Finished {
                ended_at: "2026-11-20T10:01:10Z".into(),
                path: "/c".into(),
                kind: "search_index".into(),
            },
        ] {
            assert_eq!(Marker::decode(&m.encode()).unwrap(), m);
        }
    }

    #[test]
    fn a_damaged_marker_is_refused_with_the_contract_text() {
        let bad_group =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"[7,"x","/a","k"]"#);
        let short = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("[2,\"x\"]");
        for raw in ["", "not base64 !", bad_group.as_str(), short.as_str()] {
            let e = Marker::decode(raw).unwrap_err();
            assert_eq!(e.to_string(), "ERR_INVALID_ARGUMENT: marker is not valid", "{raw}");
        }
    }

    #[test]
    fn an_unknown_kind_is_refused_with_the_contract_text() {
        let e = JobKind::from_str("thumbnail").unwrap_err();
        assert_eq!(e.to_string(), "ERR_INVALID_ARGUMENT: kind must be 'search_index'");
    }
}
