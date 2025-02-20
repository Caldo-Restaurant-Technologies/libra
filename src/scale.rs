use phidget::ReturnCode;
use phidget::{devices::VoltageRatioInput, Phidget};
use std::{array, time::Duration};
const NUMBER_OF_INPUTS: usize = 4;
const TIMEOUT: Duration = phidget::TIMEOUT_DEFAULT;

fn dot_product(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(a, b)| a * b).sum::<f64>()
}

#[derive(Debug)]
pub enum Error {
    InvalidCoefficients,
    InvalidPhidgetId,
    PhidgetError,
}
pub struct Scale {
    phidget_id: i32,
    coefficients: [f64; 4],
    vins: [VoltageRatioInput; NUMBER_OF_INPUTS],
}

impl Scale {
    pub fn new(phidget_id: i32, coefficients: [f64; 4]) -> Self {
        Self {
            phidget_id,
            coefficients,
            vins: array::from_fn(|_| VoltageRatioInput::new()),
        }
    }

    pub fn connect(mut self) -> Self {
        self.vins.iter_mut().enumerate().for_each(|(i, vin)| {
            vin.set_serial_number(self.phidget_id)
                .expect("Invalid phidget_id");

            vin.set_channel(i as i32).expect("Invalid Channel");

            vin.open_wait(TIMEOUT)
                .expect("Failed to open Phidget connection");

            let min_interval = vin
                .min_data_interval()
                .expect("Unable to get min data interval");

            vin.set_data_interval(min_interval)
                .expect("Failed to set data interval");
        });
        self
    }

    pub fn get_weight(&self, offset: f64) -> Result<f64, Error> {
        let readings = self.get_raw_readings();
        match readings {
            Ok(readings) => {
                Ok(dot_product(readings.as_slice(), self.coefficients.as_slice()) - offset)
            }
            Err(_) => Err(Error::PhidgetError),
        }
    }

    fn get_raw_readings(&self) -> Result<Vec<f64>, ReturnCode> {
        self.vins.iter().map(|vin| vin.voltage_ratio()).collect()
    }
}
