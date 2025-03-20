use serde::{Deserialize, Serialize};
pub mod scale;

pub fn median(weights: &mut [f64]) -> f64 {
    weights.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let middle = weights.len() / 2;
    weights[middle]
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ScaleCmd {
    GetWeight,
    GetMedianWeight{samples: usize},
    Shutdown
}
