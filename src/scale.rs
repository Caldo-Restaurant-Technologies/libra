use phidget::ReturnCode;
use phidget::{devices::VoltageRatioInput, Phidget};
use std::fmt;
use std::{array, time::Duration};

use crate::median;
const NUMBER_OF_INPUTS: usize = 4;
pub const TIMEOUT: Duration = phidget::TIMEOUT_DEFAULT;

fn dot_product(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(a, b)| a * b).sum::<f64>()
}

#[derive(Debug)]
pub enum Error {
    InvalidCoefficients,
    InvalidPhidgetId,
    PhidgetError(ReturnCode),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidCoefficients => write!(f, "Invalid coefficients"),
            Error::InvalidPhidgetId => write!(f, "Invalid Phidget ID"),
            Error::PhidgetError(code) => write!(f, "Phidget error:  {}", code),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Error::PhidgetError(ReturnCode::Io)
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(_: std::num::ParseIntError) -> Self {
        Error::InvalidCoefficients
    }
}

impl From<std::string::ParseError> for Error {
    fn from(_: std::string::ParseError) -> Self {
        Error::InvalidPhidgetId
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
    ) -> Result<ConnectedScale, Error> {
        let vins = array::from_fn(|i| {
            let mut vin = VoltageRatioInput::new();
            vin.set_serial_number(self.phidget_id)
                .expect("Invalid Phidget Serial Number");
            vin.set_channel(i as i32).unwrap(); //This is ok because its impossible for i to exceed
                                                //the number of channels
            vin.open_wait(timeout)
                .expect("Unable to connect to phidget");
            let min_interval = vin.min_data_interval().expect("Unable to get min interval");
            vin.set_data_interval(min_interval)
                .expect("Unable to set data interval");
            vin
        });
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

    pub fn get_weight(&self) -> Result<f64, Error> {
        let readings = self.get_raw_readings();
        match readings {
            Ok(readings) => {
                Ok(dot_product(readings.as_slice(), self.coefficients.as_slice()) - self.offset)
            }
            Err(e) => Err(Error::PhidgetError(e)),
        }
    }

    pub fn get_median_weight(&self, samples: usize) -> Result<f64, Error> {
        let mut weights = Vec::with_capacity(samples);
        while weights.len() < samples {
            let weight = self.get_weight()?;
            weights.push(weight);
        }
        Ok(median(weights.as_mut_slice()))
    }

    fn get_raw_readings(&self) -> Result<Vec<f64>, ReturnCode> {
        self.vins.iter().map(|vin| vin.voltage_ratio()).collect()
    }
}
