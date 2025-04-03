use phidget::ReturnCode;
use phidget::{devices::VoltageRatioInput, Phidget};
use std::time::Duration;
use thiserror::Error;

use crate::median;
const NUMBER_OF_INPUTS: usize = 4;
pub const TIMEOUT: Duration = phidget::TIMEOUT_DEFAULT;

fn dot_product(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(a, b)| a * b).sum::<f64>()
}

#[derive(Error, Debug)]
pub enum ScaleError {
    #[error("Invalid coefficients")]
    InvalidCoefficients,

    #[error("Invalid Phidget ID")]
    InvalidPhidgetId,

    #[error("Phidget error: {0}")]
    PhidgetError(ReturnCode),

    #[error("IO Error")]
    IoError,
}

pub trait Scale {
    fn get_weight(&self) -> Result<f64, Box<dyn std::error::Error>>;
    fn get_median_weight(&self, samples: usize) -> Result<f64, Box<dyn std::error::Error>>;
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
        let vins_result: Result<Vec<VoltageRatioInput>, ScaleError> =
            (0..NUMBER_OF_INPUTS)
                .map(|i| {
                    let mut vin = VoltageRatioInput::new();
                    vin.set_serial_number(self.phidget_id)
                        .map_err(|_| ScaleError::InvalidPhidgetId)?;
                    vin.set_channel(i as i32).unwrap(); //This is ok because its impossible for i to exceed
                                                        //the number of channels
                    vin.open_wait(timeout)
                        .map_err(ScaleError::PhidgetError)?;
                    let min_interval = vin
                        .min_data_interval()
                        .map_err(ScaleError::PhidgetError)?;
                    vin.set_data_interval(min_interval)
                        .map_err(ScaleError::PhidgetError)?;
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
        let vins_result: Result<Vec<VoltageRatioInput>, ScaleError> =
            (0..NUMBER_OF_INPUTS)
                .map(|i| {
                    let mut vin = VoltageRatioInput::new();
                    vin.set_channel(i as i32).unwrap(); //This is ok because its impossible for i to exceed
                                                        //the number of channels
                    vin.open_wait(timeout)
                        .map_err(ScaleError::PhidgetError)?;
                    let min_interval = vin
                        .min_data_interval()
                        .map_err(ScaleError::PhidgetError)?;
                    vin.set_data_interval(min_interval)
                        .map_err(ScaleError::PhidgetError)?;
                    Ok(vin)
                })
                .collect();

        let vins_vec = vins_result?;
        let mut vins: [VoltageRatioInput; NUMBER_OF_INPUTS] = match vins_vec.try_into() {
            Ok(arr) => arr,
            Err(_) => unreachable!("We know the size is correct"),
        };

        let sn = Phidget::serial_number(&mut vins[0]).map_err(ScaleError::PhidgetError)?;

        Ok(Self::new(sn, 0., [0.; NUMBER_OF_INPUTS], vins))
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

    pub fn get_raw_readings(&self) -> Result<Vec<f64>, ReturnCode> {
        self.vins.iter().map(|vin| vin.voltage_ratio()).collect()
    }
}

impl Scale for ConnectedScale {
    fn get_weight(&self) -> Result<f64, Box<dyn std::error::Error>> {
        let readings = self.get_raw_readings();
        match readings {
            Ok(readings) => {
                Ok(dot_product(readings.as_slice(), self.coefficients.as_slice()) - self.offset)
            }
            Err(e) => Err(Box::new(ScaleError::PhidgetError(e))),
        }
    }
    fn get_median_weight(&self, samples: usize) -> Result<f64, Box<dyn std::error::Error>> {
        let mut weights = Vec::with_capacity(samples);
        while weights.len() < samples {
            let weight = self.get_weight()?;
            weights.push(weight);
        }
        Ok(median(weights.as_mut_slice()))
    }
}
