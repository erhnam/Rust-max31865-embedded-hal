use std::{thread, time::Duration};
use std::collections::HashMap;

use linux_embedded_hal::spidev::{SpidevOptions, SpiModeFlags};
use linux_embedded_hal::sysfs_gpio::Direction;
use linux_embedded_hal::{Pin, Spidev};
use max31865::{FilterMode, Max31865, SensorType};

const MAX31865_RDY_PIN: u64 = 7;
const MAX31865_CS_PIN: u64 = 12;
const MAX31865_HZ: u32 = 5_000_000;
const MAX31865_CALIBRATION_DEFAULT: u32 = 43234;

fn gpio_get_pin(pin_num: u64) -> u64 {
    let pin_map: HashMap<u64, u64> = [
        (1, 508),
        (2, 509),
        (4, 378),
        (5, 377),
        (6, 371),
        (7, 372),
        (9, 375),
        (10, 374),
        (11, 373),
        (12, 370),
        (14, 425),
        (15, 426),
        (16, 496),
        (17, 497),
        (19, 494),
        (20, 495),
        (21, 503),
        (22, 504),
        (24, 502),
        (25, 505),
        (26, 507),
        (27, 506),
        (29, 356),
        (41, 440),
    ]
    .iter()
    .cloned()
    .collect();

    *pin_map.get(&pin_num).unwrap_or(&0)
}

#[tracing::instrument]
fn main() -> anyhow::Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::new();

    if tracing::subscriber::set_global_default(subscriber).is_err() {
        tracing::error!("Can't set global tracing::subscriber default");
    }

    let mut spi1 = Spidev::open("/dev/spidev0.0").expect("error initializing SPI");
    let options = SpidevOptions::new()
        .bits_per_word(8)
        .max_speed_hz(MAX31865_HZ)
        .mode(SpiModeFlags::SPI_MODE_3)
        .build();

    spi1.configure(&options).expect("error configuring SPI");

    thread::sleep(Duration::from_millis(100));

    let spi1_cs = Pin::new(gpio_get_pin(MAX31865_CS_PIN));
    spi1_cs.export().unwrap();
    while !spi1_cs.is_exported() {}
    spi1_cs.set_direction(Direction::Out).unwrap();
    spi1_cs.set_value(1).unwrap();

    let max31865_rdy = Pin::new(gpio_get_pin(MAX31865_RDY_PIN));
    max31865_rdy.export().unwrap();
    while !max31865_rdy.is_exported() {}
    max31865_rdy.set_direction(Direction::In).unwrap();

    let mut max31865 = Max31865::new(spi1, spi1_cs, max31865_rdy).unwrap();

    // Setup the sensor so it repeatedly performs conversion and informs us over
    // the ready pin.
    max31865
        .configure(
            true,
            true,
            false,
            SensorType::ThreeWire,
            FilterMode::Filter50Hz,
        )
        .unwrap();

        max31865.set_calibration(MAX31865_CALIBRATION_DEFAULT);

    loop {
        // If the sensor is ready, read the value and print it otherwise do
        // nothing one may not want to loop like this.
        if max31865.is_ready().unwrap() {
            let temp = max31865.read_default_conversion().unwrap();

            tracing::info!("MAX31865 - Temperatura: {:?}", temp as f32 / 100.0);
        }
        thread::sleep(Duration::from_millis(2000));
    }
}
