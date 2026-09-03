extern crate alloc;
use alloc::vec::Vec;

use super::ArielOSHost;

use ariel_os_sensors::{
    Category, Label, MeasurementUnit, Reading as _,
    sensor::{ReadingChannel, ReadingError, Sample, SampleMetadata},
};
use ariel_os_sensors_registry::REGISTRY;

use wasmtime::component::bindgen;

#[cfg(feature = "sensors-async")]
bindgen!({
    world: "ariel:wasm-bindings/sensors@0.0.1",
    path: "../../wit/",
    imports: {
        "ariel:wasm-bindings/sensors-api.wait-for-reading": async,
    }
});

#[cfg(not(feature = "sensors-async"))]
bindgen!({
    world: "ariel:wasm-bindings/sensors@0.0.1",
    path: "../../wit/",
});

#[cfg(not(feature = "sensors-async"))]
use embassy_futures::block_on;

pub use ariel::wasm_bindings::sensors_api as comp_sensor;
pub use ariel::wasm_bindings::sensors_api::{Host, HostWithStore, add_to_linker};

impl Host for ArielOSHost {
    fn trigger_measurements(&mut self, category: Option<comp_sensor::Category>) -> Result<(), ()> {
        match category {
            Some(cat) => {
                for sensor in REGISTRY
                    .sensors()
                    .filter(|s| s.categories().contains(&cat.into()))
                {
                    if sensor.trigger_measurement().is_err() {
                        return Err(());
                    }
                }
            }
            None => {
                for sensor in REGISTRY.sensors() {
                    if sensor.trigger_measurement().is_err() {
                        return Err(());
                    }
                }
            }
        }

        Ok(())
    }

    #[cfg(feature = "sensors-async")]
    async fn wait_for_reading(
        &mut self,
        label: Option<comp_sensor::Label>,
    ) -> Result<Vec<(comp_sensor::Sample, comp_sensor::Channel)>, ()> {
        let mut results = Vec::new();
        for sensor in REGISTRY.sensors() {
            match sensor.wait_for_reading().await {
                // Sensor could have been filtered out before
                Err(ReadingError::NotMeasuring) => {
                    ariel_os_log::debug!(
                        "Sensor {:?} of categories {:?} wasn't measuring, possibly because it was filtered out before",
                        sensor.display_name(),
                        sensor.categories()
                    );
                    continue;
                }
                Ok(samples) => match label {
                    Some(label) => {
                        for (reading_channel, sample) in
                            samples.samples().filter(|(r, _)| r.label() == label.into())
                        {
                            results.push((
                                comp_sensor::Sample::from(sample),
                                comp_sensor::Channel::from(reading_channel),
                            ))
                        }
                    }
                    None => {
                        for (reading_channel, sample) in samples.samples() {
                            results.push((
                                comp_sensor::Sample::from(sample),
                                comp_sensor::Channel::from(reading_channel),
                            ))
                        }
                    }
                },
                Err(_error) => return Err(()),
            }
        }
        Ok(results)
    }

    #[cfg(not(feature = "sensors-async"))]
    fn wait_for_reading(
        &mut self,
        label: Option<comp_sensor::Label>,
    ) -> Result<Vec<(comp_sensor::Sample, comp_sensor::Channel)>, ()> {
        let mut results = Vec::new();
        for sensor in REGISTRY.sensors() {
            match block_on(sensor.wait_for_reading()) {
                // Sensor could have been filtered out before
                Err(ReadingError::NotMeasuring) => {
                    ariel_os_log::debug!(
                        "Sensor {:?} of categories {:?} wasn't measuring, possibly because it was filtered out before",
                        sensor.display_name(),
                        sensor.categories()
                    );
                    continue;
                }
                Ok(samples) => match label {
                    Some(label) => {
                        for (reading_channel, sample) in
                            samples.samples().filter(|(r, _)| r.label() == label.into())
                        {
                            results.push((
                                comp_sensor::Sample::from(sample),
                                comp_sensor::Channel::from(reading_channel),
                            ))
                        }
                    }
                    None => {
                        for (reading_channel, sample) in samples.samples() {
                            results.push((
                                comp_sensor::Sample::from(sample),
                                comp_sensor::Channel::from(reading_channel),
                            ))
                        }
                    }
                },
                Err(_error) => return Err(()),
            }
        }
        Ok(results)
    }
}

impl From<Category> for comp_sensor::Category {
    fn from(value: Category) -> Self {
        match value {
            Category::Accelerometer => comp_sensor::Category::Accelerometer,
            Category::AccelerometerTemperature => comp_sensor::Category::AccelerometerTemperature,
            Category::AccelerometerGyroscope => comp_sensor::Category::AccelerometerGyroscope,
            Category::AccelerometerGyroscopeTemperature => {
                comp_sensor::Category::AccelerometerGyroscopeTemperature
            }
            Category::AccelerometerMagnetometerTemperature => {
                comp_sensor::Category::AccelerometerMagnetometerTemperature
            }
            Category::Ammeter => comp_sensor::Category::Ammeter,
            Category::Co2Gas => comp_sensor::Category::Co2Gas,
            Category::Color => comp_sensor::Category::Color,
            Category::Gnss => comp_sensor::Category::Gnss,
            Category::Gyroscope => comp_sensor::Category::Gyroscope,
            Category::RelativeHumidity => comp_sensor::Category::RelativeHumidity,
            Category::RelativeHumidityTemperature => {
                comp_sensor::Category::RelativeHumidityTemperature
            }
            Category::Light => comp_sensor::Category::Light,
            Category::Magnetometer => comp_sensor::Category::Magnetometer,
            Category::Ph => comp_sensor::Category::Ph,
            Category::Pressure => comp_sensor::Category::Pressure,
            Category::PushButton => comp_sensor::Category::PushButton,
            Category::Temperature => comp_sensor::Category::Temperature,
            Category::Tvoc => comp_sensor::Category::Tvoc,
            Category::Voltage => comp_sensor::Category::Voltage,
            category => {
                unimplemented!(
                    "Sensors of the following category ({:?}) isn't supported yet",
                    category
                )
            }
        }
    }
}

impl From<comp_sensor::Category> for Category {
    fn from(value: comp_sensor::Category) -> Self {
        match value {
            comp_sensor::Category::Accelerometer => Category::Accelerometer,
            comp_sensor::Category::AccelerometerTemperature => Category::AccelerometerTemperature,
            comp_sensor::Category::AccelerometerGyroscope => Category::AccelerometerGyroscope,
            comp_sensor::Category::AccelerometerGyroscopeTemperature => {
                Category::AccelerometerGyroscopeTemperature
            }
            comp_sensor::Category::AccelerometerMagnetometerTemperature => {
                Category::AccelerometerMagnetometerTemperature
            }
            comp_sensor::Category::Ammeter => Category::Ammeter,
            comp_sensor::Category::Co2Gas => Category::Co2Gas,
            comp_sensor::Category::Color => Category::Color,
            comp_sensor::Category::Gnss => Category::Gnss,
            comp_sensor::Category::Gyroscope => Category::Gyroscope,
            comp_sensor::Category::RelativeHumidity => Category::Gyroscope,
            comp_sensor::Category::RelativeHumidityTemperature => {
                Category::RelativeHumidityTemperature
            }
            comp_sensor::Category::Light => Category::Light,
            comp_sensor::Category::Magnetometer => Category::Magnetometer,
            comp_sensor::Category::Ph => Category::Ph,
            comp_sensor::Category::Pressure => Category::Pressure,
            comp_sensor::Category::PushButton => Category::PushButton,
            comp_sensor::Category::Temperature => Category::Temperature,
            comp_sensor::Category::Tvoc => Category::Tvoc,
            comp_sensor::Category::Voltage => Category::Voltage,
        }
    }
}

impl From<Label> for comp_sensor::Label {
    fn from(value: Label) -> Self {
        match value {
            Label::AccelerationX => comp_sensor::Label::AccelerationX,
            Label::AccelerationY => comp_sensor::Label::AccelerationY,
            Label::AccelerationZ => comp_sensor::Label::AccelerationZ,
            Label::Altitude => comp_sensor::Label::Altitude,
            Label::AngularVelocityX => comp_sensor::Label::AngularVelocityX,
            Label::AngularVelocityY => comp_sensor::Label::AngularVelocityY,
            Label::AngularVelocityZ => comp_sensor::Label::AngularVelocityZ,
            Label::GroundSpeed => comp_sensor::Label::GroundSpeed,
            Label::Latitude => comp_sensor::Label::Latitude,
            Label::Longitude => comp_sensor::Label::Longitude,
            Label::Opaque => comp_sensor::Label::Opaque,
            Label::RelativeHumidity => comp_sensor::Label::RelativeHumidity,
            Label::Heading => comp_sensor::Label::Heading,
            Label::Temperature => comp_sensor::Label::Temperature,
            Label::VerticalSpeed => comp_sensor::Label::VerticalSpeed,
            Label::X => comp_sensor::Label::X,
            Label::Y => comp_sensor::Label::Y,
            Label::Z => comp_sensor::Label::Z,
            label => {
                unimplemented!("This label ({}) is not supported yet", label)
            }
        }
    }
}

impl From<comp_sensor::Label> for Label {
    fn from(value: comp_sensor::Label) -> Self {
        match value {
            comp_sensor::Label::AccelerationX => Label::AccelerationX,
            comp_sensor::Label::AccelerationY => Label::AccelerationY,
            comp_sensor::Label::AccelerationZ => Label::AccelerationZ,
            comp_sensor::Label::Altitude => Label::Altitude,
            comp_sensor::Label::AngularVelocityX => Label::AngularVelocityX,
            comp_sensor::Label::AngularVelocityY => Label::AngularVelocityY,
            comp_sensor::Label::AngularVelocityZ => Label::AngularVelocityZ,
            comp_sensor::Label::GroundSpeed => Label::GroundSpeed,
            comp_sensor::Label::Latitude => Label::Latitude,
            comp_sensor::Label::Longitude => Label::Longitude,
            comp_sensor::Label::Opaque => Label::Opaque,
            comp_sensor::Label::RelativeHumidity => Label::RelativeHumidity,
            comp_sensor::Label::Heading => Label::Heading,
            comp_sensor::Label::Temperature => Label::Temperature,
            comp_sensor::Label::VerticalSpeed => Label::VerticalSpeed,
            comp_sensor::Label::X => Label::X,
            comp_sensor::Label::Y => Label::Y,
            comp_sensor::Label::Z => Label::Z,
        }
    }
}

impl From<MeasurementUnit> for comp_sensor::MeasurementUnit {
    fn from(value: MeasurementUnit) -> Self {
        match value {
            MeasurementUnit::AccelG => comp_sensor::MeasurementUnit::AccelG,
            MeasurementUnit::Ampere => comp_sensor::MeasurementUnit::Ampere,
            MeasurementUnit::Becquerel => comp_sensor::MeasurementUnit::Becquerel,
            MeasurementUnit::Bool => comp_sensor::MeasurementUnit::Boolean,
            MeasurementUnit::Candela => comp_sensor::MeasurementUnit::Candela,
            MeasurementUnit::Celsius => comp_sensor::MeasurementUnit::Celsius,
            MeasurementUnit::Coulomb => comp_sensor::MeasurementUnit::Coulomb,
            MeasurementUnit::Decibel => comp_sensor::MeasurementUnit::Decibel,
            MeasurementUnit::DecimalDegree => comp_sensor::MeasurementUnit::Decimaldegree,
            MeasurementUnit::Degree => comp_sensor::MeasurementUnit::Degree,
            MeasurementUnit::DegreePerSecond => comp_sensor::MeasurementUnit::DegreePerSecond,
            MeasurementUnit::Farad => comp_sensor::MeasurementUnit::Farad,
            MeasurementUnit::Gram => comp_sensor::MeasurementUnit::Gram,
            MeasurementUnit::Gray => comp_sensor::MeasurementUnit::Gray,
            MeasurementUnit::Henry => comp_sensor::MeasurementUnit::Henry,
            MeasurementUnit::Hertz => comp_sensor::MeasurementUnit::Hertz,
            MeasurementUnit::Joule => comp_sensor::MeasurementUnit::Joule,
            MeasurementUnit::Katal => comp_sensor::MeasurementUnit::Katal,
            MeasurementUnit::Kelvin => comp_sensor::MeasurementUnit::Kelvin,
            MeasurementUnit::Lumen => comp_sensor::MeasurementUnit::Lumen,
            MeasurementUnit::Lux => comp_sensor::MeasurementUnit::Lux,
            MeasurementUnit::Meter => comp_sensor::MeasurementUnit::Meter,
            MeasurementUnit::MeterPerSecond => comp_sensor::MeasurementUnit::MeterPerSecond,
            MeasurementUnit::Mole => comp_sensor::MeasurementUnit::Mole,
            MeasurementUnit::Newton => comp_sensor::MeasurementUnit::Newton,
            MeasurementUnit::Ohm => comp_sensor::MeasurementUnit::Ohm,
            MeasurementUnit::Pascal => comp_sensor::MeasurementUnit::Pascal,
            MeasurementUnit::Percent => comp_sensor::MeasurementUnit::Percent,
            MeasurementUnit::PercentageRelativeHumidity => {
                comp_sensor::MeasurementUnit::PercentageRelativeHumidity
            }
            MeasurementUnit::Radian => comp_sensor::MeasurementUnit::Radian,
            MeasurementUnit::Second => comp_sensor::MeasurementUnit::Second,
            MeasurementUnit::Siemens => comp_sensor::MeasurementUnit::Siemens,
            MeasurementUnit::Sievert => comp_sensor::MeasurementUnit::Sievert,
            MeasurementUnit::Steradian => comp_sensor::MeasurementUnit::Steradian,
            MeasurementUnit::Tesla => comp_sensor::MeasurementUnit::Tesla,
            MeasurementUnit::Volt => comp_sensor::MeasurementUnit::Volt,
            MeasurementUnit::Watt => comp_sensor::MeasurementUnit::Watt,
            MeasurementUnit::Weber => comp_sensor::MeasurementUnit::Weber,
            unit => {
                unimplemented!("This unit ({}) is not supported yet", unit)
            }
        }
    }
}

impl From<Sample> for comp_sensor::Sample {
    fn from(value: Sample) -> Self {
        let measure = value.value().unwrap_or_default();
        let metadata = value.metadata();
        comp_sensor::Sample {
            value: measure,
            metadata: metadata.into(),
        }
    }
}

impl From<SampleMetadata> for comp_sensor::SampleMetadata {
    fn from(value: SampleMetadata) -> Self {
        match value {
            SampleMetadata::ChannelDisabled => comp_sensor::SampleMetadata::ChannelDisabled,
            SampleMetadata::ChannelTemporarilyUnavailable => {
                comp_sensor::SampleMetadata::ChannelTemporarilyUnavailable
            }
            SampleMetadata::NoMeasurementError => comp_sensor::SampleMetadata::NoMeasurementError,
            SampleMetadata::SymmetricalError {
                deviation,
                bias,
                scaling,
            } => comp_sensor::SampleMetadata::SymmetricalError((deviation, bias, scaling)),
            SampleMetadata::UnknownAccuracy => comp_sensor::SampleMetadata::UnknownAccuracy,
        }
    }
}

impl From<ReadingChannel> for comp_sensor::Channel {
    fn from(value: ReadingChannel) -> Self {
        let scaling = value.scaling();
        let label = value.label();
        let unit = value.unit();
        comp_sensor::Channel {
            label: label.into(),
            scaling,
            unit: unit.into(),
        }
    }
}
