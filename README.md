# Libra

A Rust library for interfacing with Phidget-based weighing scales with load cell support.

## Overview

Libra provides a safe, ergonomic API for working with Phidget VoltageRatioInput devices configured as weighing scales. The library handles connection management, calibration, and both single and median weight measurements.

## Features

- **Simple scale initialization** - Connect to scales by Phidget ID or auto-detect
- **Calibration support** - Configure offset and per-load-cell coefficients
- **Median filtering** - Built-in median calculation for stable readings
- **Type-safe measurements** - Distinct types for `Grams` and `MedianGrams`
- **Raw sensor access** - Direct access to voltage ratio readings for calibration
- **Comprehensive error handling** - Clear error types for debugging

## Dependencies

- `phidget` - Phidget device bindings
- `serde` - Serialization support for scale commands
- `bincode` - Binary encoding/decoding
- `thiserror` - Error type derivation

## Quick Start

### Connecting to a Scale

```rust
use libra::scale::{DisconnectedScale, TIMEOUT};
use std::time::Duration;

// Connect to a specific Phidget device
let scale = DisconnectedScale::new(123456)
    .connect(
        0.0,                          // offset in grams
        [1.0, 1.0, 1.0, 1.0],        // calibration coefficients
        TIMEOUT
    )?;

// Or auto-detect the first available scale
let scale = ConnectedScale::without_id(TIMEOUT)?;
```

### Getting Weights

```rust
// Single weight reading
let weight = scale.get_weight()?;
println!("Weight: {} g", weight.get());

// Median of multiple samples (more stable)
let median_weight = scale.get_median_weight(
    10,                              // number of samples
    Duration::from_millis(100)       // interval between samples
)?;
println!("Median weight: {} g", median_weight.get());
```

### Calibration

```rust
// Get raw voltage ratios from all load cells
let raw_readings = scale.get_raw_readings()?;

// Get median readings for calibration
let medians = scale.get_load_cell_medians(
    50,                              // number of samples
    Duration::from_millis(20)        // sample period
)?;

// Update calibration coefficients
let scale = scale.update_coefficients([c1, c2, c3, c4]);
let scale = scale.update_offset(offset_grams);
```

### Configuration

```rust
// Set data interval for all sensors
scale.set_data_intervals(Duration::from_millis(100))?;

// Read current data intervals
let intervals = scale.get_data_intervals()?;

// Get device serial number
let phidget_id = scale.get_phidget_id();
```

## Architecture

### Types

- **`Grams`** - Single weight measurement
- **`MedianGrams`** - Median-filtered weight measurement
- **`DisconnectedScale`** - Scale configuration before connection
- **`ConnectedScale`** - Active scale connection with 4 load cells

### Error Handling

```rust
use libra::scale::ScaleError;

match scale.get_weight() {
    Ok(weight) => println!("Weight: {}", weight.get()),
    Err(ScaleError::PhidgetError(e)) => eprintln!("Phidget error: {}", e),
    Err(ScaleError::InvalidCoefficients) => eprintln!("Invalid calibration"),
    Err(e) => eprintln!("Error: {}", e),
}
```

### Scale Commands (for IPC/serialization)

```rust
use libra::ScaleCmd;

let cmd = ScaleCmd::GetMedianWeight { samples: 10 };
let encoded = bincode::encode_to_vec(&cmd, bincode::config::standard())?;
```

## Hardware Requirements

- Phidget device with 4 VoltageRatioInput channels
- 4 load cells configured in a bridge configuration
- Proper physical mounting for accurate measurements

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Build documentation
cargo doc --open
```

## License

[License information not specified]

## Contributing

[Contributing guidelines not specified]
