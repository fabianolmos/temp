use embedded_hal::digital::v2::{OutputPin, InputPin};
use embedded_hal::blocking::delay::{DelayUs, DelayMs};
use esp_idf_hal::gpio::PinDriver;
use esp_idf_hal::delay::Ets;
use esp_idf_hal::prelude::Peripherals;
use esp_idf_hal::io::Write;
use esp_idf_hal::sys::link_patches;
use std::fmt::Debug;
use one_wire_bus::{OneWire, OneWireError, OneWireResult};
use ds18b20::Resolution;
use ds18b20::Ds18b20;

fn get_temperature<P, E>(
    delay: &mut (impl DelayUs<u16> + DelayMs<u16>),
    tx: &mut impl Write,
    one_wire_bus: &mut OneWire<P>,
) -> OneWireResult<(), E>
    where
        P: OutputPin<Error=E> + InputPin<Error=E>,
        E: Debug
{
    // initiate a temperature measurement for all connected devices
    ds18b20::start_simultaneous_temp_measurement(one_wire_bus, delay)?;

    // wait until the measurement is done. This depends on the resolution you specified
    // If you don't know the resolution, you can obtain it from reading the sensor data,
    // or just wait the longest time, which is the 12-bit resolution (750ms)
    Resolution::Bits12.delay_for_measurement_time(delay);

    // iterate over all the devices, and report their temperature
    let mut search_state = None;
    loop {
        if let Some((device_address, state)) = one_wire_bus.device_search(search_state.as_ref(), false, delay)? {
            search_state = Some(state);
            if device_address.family_code() != ds18b20::FAMILY_CODE {
                // skip other devices
                continue;
            }
            // You will generally create the sensor once, and save it for later
            let sensor = Ds18b20::new(device_address)?;

            // contains the read temperature, as well as config info such as the resolution used
            let sensor_data = sensor.read_data(one_wire_bus, delay)?;
            writeln!(tx, "Device at {:?} is {}°C", device_address, sensor_data.temperature);
        } else {
            break;
        }
    }
    Ok(())
}

fn test_config<P, E>(
    delay: &mut (impl DelayUs<u16> + DelayMs<u16>),
    tx: &mut impl Write,
    one_wire_bus: &mut OneWire<P>,
) -> OneWireResult<(), E>
    where
        P: OutputPin<Error=E> + InputPin<Error=E>,
        E: Debug
{

    // Find the first device on the bus (assuming they are all Ds18b20's)
    if let Some(device_address) = one_wire_bus.devices(false, delay).next() {
        let device_address = device_address?;
        let device = Ds18b20::new(device_address)?;

        // read the initial config values (read from EEPROM by the device when it was first powered)
        let initial_data = device.read_data(one_wire_bus, delay)?;
        writeln!(tx, "Initial data: {:?}", initial_data);

        let resolution = initial_data.resolution;

        // set new alarm values, but keep the resolution the same
        device.set_config(18, 24, resolution, one_wire_bus, delay)?;

        // confirm the new config is now in the scratchpad memory
        let new_data = device.read_data(one_wire_bus, delay)?;
        writeln!(tx, "New data: {:?}", new_data);

        // save the config to EEPROM to save it permanently
        device.save_to_eeprom(one_wire_bus, delay)?;

        // read the values from EEPROM back to the scratchpad to verify it was saved correctly
        device.recall_from_eeprom(one_wire_bus, delay)?;
        let eeprom_data = device.read_data(one_wire_bus, delay)?;
        writeln!(tx, "EEPROM data: {:?}", eeprom_data);
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    link_patches();

    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    let mut delay = Ets;
    let mut tx = std::io::stdout();

    let mut pin = PinDriver::input_output(pins.gpio4)?;
    let mut one_wire_bus = OneWire::new(pin)?;

    writeln!(tx, "Testing DS18B20 sensor").unwrap();

    // Test the sensor configuration
    test_config(&mut delay, &mut tx, &mut one_wire_bus)?;

    // Get the temperature from the sensor
    get_temperature(&mut delay, &mut tx, &mut one_wire_bus)?;

    Ok(())
}
