use std::io::{Read,Write};

use std::{thread, time::Duration};
use linux_embedded_hal::spidev::{Spidev, SpidevOptions, SpidevTransfer, SpiModeFlags};
use linux_embedded_hal::sysfs_gpio::{Direction, Pin};
use max31865::{FilterMode, SensorType};
use max31865::temp_conversion::LOOKUP_VEC_PT100;

const PIN_SPI_CS: u64 = 12;
const MAX31865_REG_READ_CONF: u8 = 0x00;
const MAX31865_REG_WRITE_CONF: u8 = 0x80;
const MAX31865_REG_MSB: u8 = 0x01;
const MAX31865_REG_LSB: u8 = 0x02;
const MAX31865_CALIBRATION_DEFAULT: u32 = 43234;

fn gpio_get_pin(pin_num: u64) -> u64 {
    let pinnum = [
        1, 2, 4, 5, 6, 7, 9, 10, 11, 12, 14, 15, 16, 17, 19, 20, 21, 22, 24, 25, 26, 27, 29, 41,
    ];
    let pinmap = [
        508, 509, 378, 377, 371, 372, 375, 374, 373, 370, 425, 426, 496, 497, 494, 495, 503, 504,
        502, 505, 507, 506, 356, 440,
    ];

    match pinnum.iter().position(|&x| x == pin_num) {
        Some(index) => pinmap[index],
        None => 0, // Valor predeterminado si el pin no se encuentra en la lista
    }
}

/// perform half duplex operations using Read and Write traits
fn read_write(spi: &mut Spidev, address: u8, value: u8) -> u8 {
    let mut rx_buf = [0_u8; 2];
    if value != 0 {
        spi.write(&[address, value]).unwrap();
    } else {
        spi.write(&[address, 0x00]).unwrap();
    }

    spi.read(&mut rx_buf).unwrap();

    rx_buf[1]
}

/// Perform full duplex operations using Ioctl
fn transfer(spi: &mut Spidev, address: u8) -> u8 {
    // "write" transfers are also reads at the same time with
    // the read having the same length as the write
    let tx_buf = [address, 0x00];
    let mut rx_buf = [0; 2];
    
    let mut transfer = SpidevTransfer::read_write(&tx_buf, &mut rx_buf);
    spi.transfer(&mut transfer).unwrap();

    rx_buf[1]
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
        .max_speed_hz(5_000_000)
        .mode(SpiModeFlags::SPI_MODE_3)
        .build();

    spi1.configure(&options).expect("error configuring SPI");

    spi1.flush().unwrap();

    thread::sleep(Duration::from_millis(100));

    let spi1_cs = Pin::new(gpio_get_pin(PIN_SPI_CS));
    spi1_cs.export().expect("error exporting cs pin");
    spi1_cs.set_direction(Direction::Out)
        .expect("error setting cs pin direction");
    spi1_cs.set_value(0).unwrap();

    /* 
    let conf: u8 = ((vbias as u8) << 7)
            | ((conversion_mode as u8) << 6)
            | ((one_shot as u8) << 5)
            | ((sensor_type as u8) << 4)
            | (filter_mode as u8);
*/
    let conf: u8 = ((true as u8) << 7)
            | ((true as u8) << 6)
            | ((false as u8) << 5)
            | ((SensorType::TwoOrFourWire as u8) << 4)
            | (FilterMode::Filter50Hz as u8);  
   
    /* Enviar Configuracion */
    spi1_cs.set_value(0).unwrap();
    read_write(&mut spi1, MAX31865_REG_WRITE_CONF, conf);
    spi1_cs.set_value(1).unwrap();

    spi1_cs.set_value(0).unwrap();
    let config: u8 = read_write(&mut spi1, MAX31865_REG_WRITE_CONF, conf);
    spi1_cs.set_value(1).unwrap();

    tracing::info!("MAX31865 - configurado: 0x{:02X}", config);

    tracing::info!("MAX31865 - leyendo temperatura");

    // leer el registro MSB
    spi1_cs.set_value(0).unwrap();
    let msb: u8 = transfer(&mut spi1, MAX31865_REG_MSB);
    spi1_cs.set_value(1).unwrap();

    // leer el registro LSB
    spi1_cs.set_value(0).unwrap();
    let lsb: u8 = transfer(&mut spi1, MAX31865_REG_LSB);
    spi1_cs.set_value(1).unwrap();

    // Combinar MSB y LSB
    let raw_value = (msb as u16) << 8 | (lsb as u16);
    let ohms = ((raw_value >> 1) as u32 * MAX31865_CALIBRATION_DEFAULT) >> 15;
    let temp = LOOKUP_VEC_PT100.lookup_temperature(ohms as i32);
    
    tracing::info!("MAX31865 - Temperatura: {:?}", temp as f32 / 100.0);

    return Ok(());
}
