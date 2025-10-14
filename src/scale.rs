//! Scale implementation for Phidget-based load cell scales.
//!
//! This module provides concrete implementations for connecting to and reading
//! from Phidget devices configured as 4-load-cell weighing scales.

use phidget::ReturnCode;
use phidget::{devices::VoltageRatioInput, Phidget};
use std::time::Duration;
use std::{array, time};
use thiserror::Error;

use crate::{median, Grams, MedianGrams};

/// The number of load cell inputs expected on the scale (fixed at 4).
const NUMBER_OF_INPUTS: usize = 4;

/// Default timeout for Phidget operations.
pub const TIMEOUT: Duration = phidget::TIMEOUT_DEFAULT;

/// Calculates the dot product of two slices.
///
/// Used internally to apply calibration coefficients to raw voltage readings.
fn dot_product(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(a, b)| a * b).sum::<f64>()
}

/// Error returned when a Phidget device operation fails.
///
/// Contains the Phidget return code and the specific load cell channel
/// where the error occurred for easier debugging.
#[derive(Debug, Clone)]
pub struct PhidgetError {
    return_code: ReturnCode,
    load_cell: usize,
}
impl PhidgetError {
    /// Creates a new Phidget error.
    ///
    /// # Arguments
    ///
    /// * `return_code` - The error code returned by the Phidget library
    /// * `load_cell` - The channel number (0-3) where the error occurred
    pub fn new(return_code: ReturnCode, load_cell: usize) -> Self {
        Self {
            return_code,
            load_cell,
        }
    }
}
impl std::fmt::Display for PhidgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Phidget error at Load Cell {}: {}",
            self.load_cell, self.return_code
        )
    }
}

/// Errors that can occur during scale operations.
#[derive(Error, Debug, Clone)]
pub enum ScaleError {
    /// The provided calibration coefficients are invalid.
    #[error("Invalid coefficients")]
    InvalidCoefficients,

    /// The specified Phidget device ID could not be found or opened.
    #[error("Invalid Phidget ID")]
    InvalidPhidgetId,

    /// A Phidget library operation failed.
    #[error("{0}")]
    PhidgetError(PhidgetError),

    /// An I/O error occurred.
    #[error("IO Error")]
    IoError,
}
impl ScaleError {
    /// Helper to create a [`ScaleError::PhidgetError`] from a return code.
    ///
    /// # Arguments
    ///
    /// * `return_code` - The error code from the Phidget library
    /// * `load_cell` - The channel number where the error occurred
    pub fn phidget_error(return_code: ReturnCode, load_cell: usize) -> Self {
        ScaleError::PhidgetError(PhidgetError::new(return_code, load_cell))
    }
}

/// A scale configuration before connection to the hardware.
///
/// This type represents a scale that has been configured with a Phidget ID
/// but not yet connected. Call [`connect`](DisconnectedScale::connect) to
/// establish the hardware connection.
pub struct DisconnectedScale {
    phidget_id: i32,
}

impl DisconnectedScale {
    /// Creates a new disconnected scale with a specific Phidget device ID.
    ///
    /// # Arguments
    ///
    /// * `phidget_id` - The serial number of the Phidget device
    pub fn new(phidget_id: i32) -> Self {
        Self { phidget_id }
    }

    /// Connects to the Phidget hardware and creates a [`ConnectedScale`].
    ///
    /// Opens all 4 load cell channels and applies the calibration parameters.
    ///
    /// # Arguments
    ///
    /// * `offset` - The zero offset in grams (tare weight)
    /// * `coefficients` - Calibration coefficients for each of the 4 load cells
    /// * `timeout` - Maximum time to wait for device connection
    ///
    /// # Returns
    ///
    /// A [`ConnectedScale`] ready for measurements, or a [`ScaleError`] if
    /// the connection fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use libra::scale::{DisconnectedScale, TIMEOUT};
    ///
    /// let scale = DisconnectedScale::new(123456)
    ///     .connect(0.0, [1.0, 1.0, 1.0, 1.0], TIMEOUT)?;
    /// # Ok::<(), libra::scale::ScaleError>(())
    /// ```
    pub fn connect(
        self,
        offset: f64,
        coefficients: [f64; NUMBER_OF_INPUTS],
        timeout: Duration,
    ) -> Result<ConnectedScale, ScaleError> {
        let vins_result: Result<Vec<VoltageRatioInput>, ScaleError> = (0..NUMBER_OF_INPUTS)
            .map(|i| {
                let mut vin = VoltageRatioInput::new();
                vin.set_serial_number(self.phidget_id)
                    .map_err(|_| ScaleError::InvalidPhidgetId)?;
                vin.set_channel(i as i32).unwrap();
                vin.open_wait(timeout)
                    .map_err(|return_code| ScaleError::phidget_error(return_code, i))?;
                Ok(vin)
            })
            .collect();

        let vins_vec = vins_result?;
        let vins = match vins_vec.try_into() {
            Ok(arr) => arr,
            Err(_) => unreachable!("We know the size is correct"),
        };

        Ok(ConnectedScale::new(
            self.phidget_id,
            offset,
            coefficients,
            vins,
        ))
    }
}

/// A scale that is connected to Phidget hardware and ready for measurements.
///
/// This type provides methods for reading weights, accessing raw sensor data,
/// and updating calibration parameters. It maintains connections to all 4
/// load cell channels.
pub struct ConnectedScale {
    phidget_id: i32,
    offset: f64,
    coefficients: [f64; NUMBER_OF_INPUTS],
    vins: [VoltageRatioInput; NUMBER_OF_INPUTS],
}

impl ConnectedScale {
    fn new(
        phidget_id: i32,
        offset: f64,
        coefficients: [f64; NUMBER_OF_INPUTS],
        vins: [VoltageRatioInput; NUMBER_OF_INPUTS],
    ) -> Self {
        Self {
            phidget_id,
            offset,
            coefficients,
            vins,
        }
    }

    /// Connects to the first available Phidget scale device.
    ///
    /// This method auto-detects and connects to any available Phidget with
    /// 4 voltage ratio input channels. Useful when only one scale is connected.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum time to wait for device detection
    ///
    /// # Returns
    ///
    /// A [`ConnectedScale`] with default calibration (coefficients = 0, offset = 0),
    /// or a [`ScaleError`] if no device is found.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use libra::scale::{ConnectedScale, TIMEOUT};
    ///
    /// let scale = ConnectedScale::without_id(TIMEOUT)?;
    /// println!("Connected to device: {}", scale.get_phidget_id());
    /// # Ok::<(), libra::scale::ScaleError>(())
    /// ```
    pub fn without_id(timeout: Duration) -> Result<Self, ScaleError> {
        let vins_result: Result<Vec<VoltageRatioInput>, ScaleError> = (0..NUMBER_OF_INPUTS)
            .map(|i| {
                let mut vin = VoltageRatioInput::new();
                vin.set_channel(i as i32).unwrap(); //This is ok because its impossible for i to exceed the number of channels
                vin.open_wait(timeout)
                    .map_err(|return_code| ScaleError::phidget_error(return_code, i))?;
                Ok(vin)
            })
            .collect();

        let vins_vec = vins_result?;
        let mut vins: [VoltageRatioInput; NUMBER_OF_INPUTS] = match vins_vec.try_into() {
            Ok(arr) => arr,
            Err(_) => unreachable!("We know the size is correct"),
        };

        let vin = 0;
        let sn = Phidget::serial_number(&mut vins[vin])
            .map_err(|return_code| ScaleError::phidget_error(return_code, vin))?;

        Ok(Self::new(sn, 0., [0.; NUMBER_OF_INPUTS], vins))
    }

    /// Sets the data acquisition interval for all load cell channels.
    ///
    /// This controls how frequently the Phidget hardware samples the sensors.
    ///
    /// # Arguments
    ///
    /// * `interval` - The time between sensor readings
    ///
    /// # Errors
    ///
    /// Returns [`ScaleError`] if the interval cannot be set on any channel.
    pub fn set_data_intervals(&mut self, interval: Duration) -> Result<(), ScaleError> {
        self.vins.iter_mut().enumerate().try_for_each(|(i, vin)| {
            vin.set_data_interval(interval)
                .map_err(|return_code| ScaleError::phidget_error(return_code, i))
        })
    }

    /// Retrieves the current data acquisition intervals for all channels.
    ///
    /// # Returns
    ///
    /// A vector of intervals, one for each of the 4 load cell channels.
    ///
    /// # Errors
    ///
    /// Returns [`ScaleError`] if any channel cannot be queried.
    pub fn get_data_intervals(&mut self) -> Result<Vec<Duration>, ScaleError> {
        self.vins
            .iter_mut()
            .enumerate()
            .map(|(i, vin)| {
                vin.data_interval()
                    .map_err(|e| ScaleError::phidget_error(e, i))
            })
            .collect()
    }

    /// Updates the calibration coefficients and returns a new scale instance.
    ///
    /// # Arguments
    ///
    /// * `coefficients` - New calibration coefficients for the 4 load cells
    ///
    /// # Returns
    ///
    /// A new [`ConnectedScale`] with updated coefficients (consumes self).
    pub fn update_coefficients(self, coefficients: [f64; 4]) -> Self {
        Self {
            phidget_id: self.phidget_id,
            offset: self.offset,
            coefficients,
            vins: self.vins,
        }
    }

    /// Updates the zero offset (tare) and returns a new scale instance.
    ///
    /// # Arguments
    ///
    /// * `offset` - New offset in grams (tare weight to subtract)
    ///
    /// # Returns
    ///
    /// A new [`ConnectedScale`] with updated offset (consumes self).
    pub fn update_offset(self, offset: f64) -> Self {
        Self {
            phidget_id: self.phidget_id,
            offset,
            coefficients: self.coefficients,
            vins: self.vins,
        }
    }

    /// Reads raw voltage ratios from all load cell channels.
    ///
    /// Returns the uncalibrated voltage ratio readings, useful for
    /// calibration and debugging.
    ///
    /// # Returns
    ///
    /// A vector of 4 voltage ratio readings, one per load cell.
    ///
    /// # Errors
    ///
    /// Returns [`ScaleError`] if any channel cannot be read.
    pub fn get_raw_readings(&self) -> Result<Vec<f64>, ScaleError> {
        self.vins
            .iter()
            .enumerate()
            .map(|(i, vin)| {
                vin.voltage_ratio()
                    .map_err(|e| ScaleError::phidget_error(e, i))
            })
            .collect()
    }
    /// Measures the current weight on the scale.
    ///
    /// Reads all load cells, applies calibration coefficients via dot product,
    /// and subtracts the offset to return the weight in grams.
    ///
    /// # Returns
    ///
    /// The current weight as [`Grams`], or a [`ScaleError`] if reading fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use libra::scale::{ConnectedScale, TIMEOUT};
    ///
    /// let scale = ConnectedScale::without_id(TIMEOUT)?;
    /// let weight = scale.get_weight()?;
    /// println!("Current weight: {} g", weight.get());
    /// # Ok::<(), libra::scale::ScaleError>(())
    /// ```
    pub fn get_weight(&self) -> Result<Grams, ScaleError> {
        let readings = self.get_raw_readings()?;
        Ok(Grams(
            dot_product(readings.as_slice(), self.coefficients.as_slice()) - self.offset,
        ))
    }

    /// Measures the median weight from multiple samples.
    ///
    /// Collects multiple weight measurements at the specified interval and
    /// returns the median value for more stable readings.
    ///
    /// # Arguments
    ///
    /// * `samples` - The number of weight measurements to collect
    /// * `interval` - Minimum time between each sample
    ///
    /// # Returns
    ///
    /// The median weight as [`MedianGrams`], or a [`ScaleError`] if any reading fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use libra::scale::{ConnectedScale, TIMEOUT};
    /// use std::time::Duration;
    ///
    /// let scale = ConnectedScale::without_id(TIMEOUT)?;
    /// let median = scale.get_median_weight(10, Duration::from_millis(100))?;
    /// println!("Stable weight: {} g", median.get());
    /// # Ok::<(), libra::scale::ScaleError>(())
    /// ```
    pub fn get_median_weight(
        &self,
        samples: usize,
        interval: Duration,
    ) -> Result<MedianGrams, ScaleError> {
        let mut weights = Vec::with_capacity(samples);
        let mut init_time = time::Instant::now();
        while weights.len() < samples {
            let current_time = time::Instant::now();
            let time_delta = current_time - init_time;
            if time_delta > interval {
                let weight = self.get_weight()?;
                weights.push(weight);
                init_time = time::Instant::now();
            }
        }
        Ok(median(weights.as_mut_slice()))
    }

    fn get_input_reading(&self, input: usize) -> Result<f64, ScaleError> {
        self.vins[input]
            .voltage_ratio()
            .map_err(|e| ScaleError::phidget_error(e, input))
    }

    /// Collects median voltage readings from each load cell individually.
    ///
    /// This is particularly useful for calibration, as it provides stable
    /// readings from each load cell separately rather than a combined weight.
    ///
    /// # Arguments
    ///
    /// * `samples` - The number of readings to collect per load cell
    /// * `sample_period` - Time to wait between each reading
    ///
    /// # Returns
    ///
    /// An array of 4 median voltage ratios, one for each load cell.
    ///
    /// # Errors
    ///
    /// Returns [`ScaleError`] if any reading fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use libra::scale::{ConnectedScale, TIMEOUT};
    /// use std::time::Duration;
    ///
    /// let scale = ConnectedScale::without_id(TIMEOUT)?;
    /// let medians = scale.get_load_cell_medians(50, Duration::from_millis(20))?;
    /// println!("Load cell medians: {:?}", medians);
    /// # Ok::<(), libra::scale::ScaleError>(())
    /// ```
    pub fn get_load_cell_medians(&self, samples: usize, sample_period: Duration) -> Result<[f64; NUMBER_OF_INPUTS], ScaleError> {
        let mut medians: [Vec<f64>; NUMBER_OF_INPUTS] =
            array::from_fn(|_| Vec::with_capacity(samples));
        for _ in 0..samples {
            for (i, vin_medians) in medians.iter_mut().enumerate().take(NUMBER_OF_INPUTS) {
                vin_medians.push(self.get_input_reading(i)?);
            }
            std::thread::sleep(sample_period);
        }
        Ok(array::from_fn(|vin| {
            medians.sort_by(|a, b| a.partial_cmp(b).unwrap());
            medians[vin][samples / 2]
        }))
    }

    /// Returns the Phidget device serial number.
    ///
    /// # Returns
    ///
    /// The serial number (ID) of the connected Phidget device.
    pub fn get_phidget_id(&self) -> i32 {
        self.phidget_id
    }
}
