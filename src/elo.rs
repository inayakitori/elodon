use probability::distribution;
use probability::distribution::Inverse;
use crate::structs::*;

pub fn get_predicted_score(elo: Option<f32>, chart: &Chart, elo_sd_z: f32) -> Option<u32>{
    Some((
        (chart.score_slope? as f32 * (elo? + elo_sd_z*chart.sd_sd?*0.001 - chart.score_miyabi? as f32)) as i32
            + 10i32.pow(6)
    ).max(0) as u32)
}

pub fn get_z_value(actual_score: u32, elo: Option<f32>, chart: &Chart, sd_z: f32) -> Option<f32> {
    return Some(
        (actual_score as i32 - get_predicted_score(elo, chart, sd_z)? as i32) as f32 / (chart.sd_mean? + sd_z * chart.sd_sd?)
    )
}

//gets the standard deviation assuming a two-sided inverse normal
pub fn get_sd(x: f64, p: f64) -> f64 {
    if p >= 1. {return 0.}
    if p <= 0. {return f64::INFINITY}
    let z = distribution::Gaussian::new(0., 1.).inverse(0.5 + 0.5*p);
    return x / z;
}