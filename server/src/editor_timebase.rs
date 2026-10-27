use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditorTimebase {
    media_clock_domain: &'static str,
    wall_clock_domain: &'static str,
    mapping: Option<ClockMapping>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClockMapping {
    media_pts_ns: String,
    wall_time_ns: String,
    bound_ns: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum EditorIssue {
    MissingProgramReference,
    MissingClockMapping,
    UnknownRecordingTimebase,
}

pub(crate) const fn unresolved_timebase() -> EditorTimebase {
    EditorTimebase {
        media_clock_domain: "engineRunPtsNs",
        wall_clock_domain: "unixEpochNs",
        mapping: None,
    }
}
