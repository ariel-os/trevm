use ariel_os::sensors::{
    Category, Label, MeasurementUnit, Sensor,
    sensor::{
        Mode as SensorMode, ReadingChannel, ReadingChannels, ReadingError, ReadingWaiter, Sample,
        SampleMetadata, Samples, SetModeError, State, TriggerMeasurementError,
    },
    signal::Signal as ReadingSignal,
};

use rand_core::RngCore as _;

/// Fake temperature sensor that reports a random reading from -10 to +45 °C
#[derive(Default)]
pub struct FakeTemperatureSensor {
    reading_buffer: ReadingSignal<Result<Samples, ReadingError>>,
}

impl FakeTemperatureSensor {
    pub const fn new() -> Self {
        Self {
            reading_buffer: ReadingSignal::new(),
        }
    }
}

impl Sensor for FakeTemperatureSensor {
    fn trigger_measurement(&self) -> Result<(), TriggerMeasurementError> {
        Ok(())
    }

    fn categories(&self) -> &'static [Category] {
        &[Category::Temperature]
    }

    fn display_name(&self) -> Option<&'static str> {
        Some("Fake Temperature Sensor")
    }

    fn label(&self) -> Option<&'static str> {
        Some("Fake sensor")
    }

    fn part_number(&self) -> Option<&'static str> {
        Some("Fake sensor")
    }

    fn reading_channels(&self) -> ReadingChannels {
        ReadingChannels::from([ReadingChannel::new(
            Label::Temperature,
            -2,
            MeasurementUnit::Celsius,
        )])
    }

    fn set_mode(&self, _mode: SensorMode) -> Result<State, SetModeError> {
        Ok(State::Measuring)
    }

    fn state(&self) -> State {
        State::Measuring
    }

    fn version(&self) -> u8 {
        0
    }

    fn wait_for_reading(&'static self) -> ReadingWaiter {
        match self.state() {
            State::Measuring => {
                // restrict to [0 5500] through mod then substract 1000 to get [-1000; 4500] which is [-10; +45] degree when accounting for scaling
                let reading = (ariel_os::random::fast_rng().next_u32() % 5500) as i32 - 1000;
                let metadata = SampleMetadata::SymmetricalError {
                    deviation: 25,
                    bias: -20,
                    scaling: -2,
                };
                self.reading_buffer
                    .signal(Ok(Samples::from_1(self, [Sample::new(reading, metadata)])));
                ReadingWaiter::new(self.reading_buffer.wait())
            }
            _ => unreachable!(),
        }
    }
}

pub static FAKE_SENSOR: FakeTemperatureSensor = const { FakeTemperatureSensor::new() };

#[ariel_os::reexports::linkme::distributed_slice(ariel_os::sensors::SENSOR_REFS)]
#[linkme(crate = ariel_os::reexports::linkme)]
static FAKE_SENSOR_REF: &'static dyn ariel_os::sensors::Sensor = &FAKE_SENSOR;
