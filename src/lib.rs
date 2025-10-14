//! Libra - A library for interfacing with Phidget-based weighing scales.
//!
//! This library provides types and functions for working with load cell scales
//! connected via Phidget VoltageRatioInput devices. It supports calibration,
//! single measurements, and median-filtered measurements for stable readings.
//!
//! # Examples
//!
//! ```no_run
//! use libra::scale::{ConnectedScale, TIMEOUT};
//!
//! let scale = ConnectedScale::without_id(TIMEOUT)?;
//! let weight = scale.get_weight()?;
//! println!("Weight: {} grams", weight.get());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use serde::{Deserialize, Serialize};
use std::future::Future;
pub mod scale;

/// A weight measurement that has been median-filtered from multiple samples.
///
#[derive(PartialEq, PartialOrd, Debug)]
pub struct MedianGrams(pub f64);
impl MedianGrams {
    /// Returns the weight value in grams.
    pub fn get(&self) -> f64 {
        self.0
    }
}

/// A single weight measurement in grams.
///
/// This type represents a raw weight reading from the scale.
/// For more stable readings, consider using [`MedianGrams`] instead.
#[derive(PartialEq, PartialOrd, Debug)]
pub struct Grams(pub f64);
impl Grams {
    /// Returns the weight value in grams.
    pub fn get(&self) -> f64 {
        self.0
    }
}

/// Calculates the median weight from a collection of measurements.
///
/// # Arguments
///
/// * `weights` - A mutable slice of weight measurements that will be sorted in place
///
/// # Returns
///
/// The median value as [`MedianGrams`]
///
/// # Examples
///
/// ```
/// use libra::{Grams, median};
///
/// let mut weights = vec![Grams(100.0), Grams(105.0), Grams(102.0)];
/// let result = median(&mut weights);
/// assert_eq!(result.get(), 102.0);
/// ```
pub fn median(weights: &mut [Grams]) -> MedianGrams {
    weights.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let middle = weights.len() / 2;
    MedianGrams(weights[middle].0)
}

/// Commands that can be sent to a scale for remote operation.
///
/// This enum is designed to be serialized (e.g., with bincode) and sent
/// over IPC or network connections for remote scale control.
#[derive(Serialize, Deserialize, Debug)]
pub enum ScaleCmd {
    /// Request a single weight measurement.
    GetWeight,
    /// Request a median weight from multiple samples.
    GetMedianWeight {
        /// The number of samples to collect for the median calculation.
        samples: usize
    },
    /// Request the scale to shut down or disconnect.
    Shutdown,
}

/// Trait for asynchronous scale operations.
///
/// This trait defines an async interface for scale measurements,
/// useful for non-blocking operations in async contexts.
pub trait AsyncScale {
    /// Asynchronously retrieves a single weight measurement.
    ///
    /// # Returns
    ///
    /// A future that resolves to the weight in grams or an error.
    fn get_weight(&self) -> impl Future<Output = Result<f64, Box<dyn std::error::Error>>>;

    /// Asynchronously retrieves a median weight from multiple samples.
    ///
    /// # Arguments
    ///
    /// * `samples` - The number of samples to collect for median calculation
    ///
    /// # Returns
    ///
    /// A future that resolves to the median weight in grams or an error.
    fn get_median_weight(
        &self,
        samples: usize,
    ) -> impl Future<Output = Result<f64, Box<dyn std::error::Error>>>;
}

/// Trait for synchronous scale operations.
///
/// This trait defines a blocking interface for scale measurements.
pub trait Scale {
    /// Retrieves a single weight measurement.
    ///
    /// # Returns
    ///
    /// The measured weight as [`Grams`] or an error.
    fn get_weight(&self) -> Result<Grams, Box<dyn std::error::Error>>;

    /// Retrieves a median-filtered weight measurement.
    ///
    /// # Returns
    ///
    /// The median weight as [`MedianGrams`] or an error.
    fn get_median_weight(&self) -> Result<MedianGrams, Box<dyn std::error::Error>>;
}
