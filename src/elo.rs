use crate::structs::*;

pub fn get_predicted_score(elo: Option<f32>, chart: Chart) -> Option<u32>{
    Some((
        (chart.score_slope? as f32 * (elo? - chart.score_miyabi? as f32)) as i32
            + 10i32.pow(6)
    ).max(0) as u32)
}

pub fn get_z_value(actual_score: u32, elo: Option<f32>, chart: Chart)-> Option<f32> {
    return Some(
        (actual_score as i32 - get_predicted_score(elo, chart)? as i32) as f32 / chart.certainty?
    )
}