use live_production_media::protocol::CompositionPlacement;

use crate::model::AudioRoute;

pub struct TakeRequest<'a> {
    pub command_id: &'a str,
    pub expected_engine_run_id: &'a str,
    pub revision: &'a str,
    pub expected_program_revision: Option<&'a str>,
    pub placements: Vec<CompositionPlacement>,
}

pub struct PrepareRequest<'a> {
    pub command_id: &'a str,
    pub expected_engine_run_id: &'a str,
    pub revision: &'a str,
    pub placements: Vec<CompositionPlacement>,
}

pub struct TrackTimingRequest<'a> {
    pub command_id: &'a str,
    pub expected_engine_run_id: &'a str,
    pub holdback_ms: i64,
    pub corrections: Vec<(String, i64)>,
}

pub struct AudioRequest<'a> {
    pub command_id: &'a str,
    pub expected_engine_run_id: &'a str,
    pub routes: &'a [AudioRoute],
}
