use phidget::ReturnCode;
use phidget::{devices::VoltageRatioInput, Phidget};
use std::time::Duration;
use std::{array, time};
use thiserror::Error;

use crate::{median, Grams, MedianGrams};
const NUMBER_OF_INPUTS: usize = 4;
pub const TIMEOUT: Duration = phidget::TIMEOUT_DEFAULT;

fn dot_product(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(a, b)| a * b).sum::<f64>()
}

#[derive(Debug, Clone)]
pub struct PhidgetError {
    return_code: ReturnCode,
    load_cell: usize,
}
impl PhidgetError {
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

#[derive(Error, Debug, Clone)]
pub enum ScaleError {
    #[error("Invalid coefficients")]
    InvalidCoefficients,

    #[error("Invalid Phidget ID")]
    InvalidPhidgetId,

    #[error("{0}")]
    PhidgetError(PhidgetError),

    #[error("IO Error")]
    IoError,
}
impl ScaleError {
    pub fn phidget_error(return_code: ReturnCode, load_cell: usize) -> Self {
        ScaleError::PhidgetError(PhidgetError::new(return_code, load_cell))
    }
}

pub struct DisconnectedScale {
    phidget_id: i32,
}

impl DisconnectedScale {
    pub fn new(phidget_id: i32) -> Self {
        Self { phidget_id }
    }

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

    pub fn set_data_intervals(&mut self, interval: Duration) -> Result<(), ScaleError> {
        self.vins.iter_mut().enumerate().try_for_each(|(i, vin)| {
            vin.set_data_interval(interval)
                .map_err(|return_code| ScaleError::phidget_error(return_code, i))
        })
    }

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

    pub fn update_coefficients(self, coefficients: [f64; 4]) -> Self {
        Self {
            phidget_id: self.phidget_id,
            offset: self.offset,
            coefficients,
            vins: self.vins,
        }
    }

    pub fn update_offset(self, offset: f64) -> Self {
        Self {
            phidget_id: self.phidget_id,
            offset,
            coefficients: self.coefficients,
            vins: self.vins,
        }
    }

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
    pub fn get_weight(&self) -> Result<Grams, ScaleError> {
        let readings = self.get_raw_readings()?;
        Ok(Grams(
            dot_product(readings.as_slice(), self.coefficients.as_slice()) - self.offset,
        ))
    }

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

    pub fn get_raw_medians(&self, samples: usize) -> Result<[f64; NUMBER_OF_INPUTS], ScaleError> {
        let mut medians: [Vec<f64>; NUMBER_OF_INPUTS] =
            array::from_fn(|_| Vec::with_capacity(samples));
        for _ in 0..samples {
            for (i, vin_medians) in medians.iter_mut().enumerate().take(NUMBER_OF_INPUTS) {
                vin_medians.push(self.get_input_reading(i)?);
            }
        }
        Ok(array::from_fn(|vin| {
            medians.sort_by(|a, b| a.partial_cmp(b).unwrap());
            medians[vin][samples / 2]
        }))
    }

    pub fn get_phidget_id(&self) -> i32 {
        self.phidget_id
    }
}
