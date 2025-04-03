use serde::{Deserialize, Serialize};
use std::future::Future;
pub mod scale;

pub fn median(weights: &mut [f64]) -> f64 {
    weights.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let middle = weights.len() / 2;
    weights[middle]
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ScaleCmd {
    GetWeight,
    GetMedianWeight { samples: usize },
    Shutdown,
}

pub trait AsyncScale {
    fn get_weight(&self) -> impl Future<Output = Result<f64, Box<dyn std::error::Error>>>;
    fn get_median_weight(
        &self,
        samples: usize,
    ) -> impl Future<Output = Result<f64, Box<dyn std::error::Error>>>;
}

pub trait Scale {
    fn get_weight(&self) -> Result<f64, Box<dyn std::error::Error>>;
    fn get_median_weight(&self) -> Result<f64, Box<dyn std::error::Error>>;
}
