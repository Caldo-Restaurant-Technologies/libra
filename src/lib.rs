use serde::{Deserialize, Serialize};
use std::future::Future;
pub mod scale;

#[derive(PartialEq, PartialOrd, Debug)]
pub struct MedianGrams(pub f64);
impl MedianGrams {
    pub fn get(&self) -> f64 {
        self.0
    }
}

#[derive(PartialEq, PartialOrd, Debug)]
pub struct Grams(pub f64);
impl Grams {
    pub fn get(&self) -> f64 {
        self.0
    }
}

pub fn median(weights: &mut [Grams]) -> MedianGrams {
    weights.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let middle = weights.len() / 2;
    MedianGrams(weights[middle].0)
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
    fn get_weight(&self) -> Result<Grams, Box<dyn std::error::Error>>;
    fn get_median_weight(&self) -> Result<MedianGrams, Box<dyn std::error::Error>>;
}
