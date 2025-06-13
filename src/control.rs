use embassy_executor::Spawner;
use embassy_rp::adc::{Adc, Channel, Config};
use embassy_rp::gpio::{Input, Pull};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
// use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::signal::Signal;

use crate::Irqs;

static BUTTON_SIGNAL: Signal<CriticalSectionRawMutex, bool> = Signal::new();
// static BUTTON_STATE: Mutex<CriticalSectionRawMutex, bool> = Mutex::new(false);

pub struct ButtonController {
    button: Input<'static>,
}

impl ButtonController {
    pub fn new(button: Input<'static>) -> Self {
        Self { button }
    }

    pub async fn run(&mut self) -> ! {
        let mut last_state = false;
        loop {
            let current_state = self.button.is_low();
            if current_state && !last_state {
                // Button pressed (falling edge)
                BUTTON_SIGNAL.signal(true);
            }
            last_state = current_state;
            embassy_time::Timer::after_millis(10).await;
        }
    }
}

pub async fn _wait_for_button_press() -> bool {
    BUTTON_SIGNAL.wait().await
}

fn button_was_pressed() -> bool {
    BUTTON_SIGNAL.try_take().unwrap_or(false)
}

#[embassy_executor::task]
async fn button_task(mut button_controller: ButtonController) {
    button_controller.run().await;
}

// Joystick handling
pub struct Joystick<'a> {
    adc: Adc<'a, embassy_rp::adc::Async>,
    pin_x: Channel<'a>,
    pin_y: Channel<'a>,
}

impl<'a> Joystick<'a> {
    pub fn new(
        spawner: Spawner,
        adc: embassy_rp::peripherals::ADC,
        pin16: embassy_rp::peripherals::PIN_16,
        pin27: embassy_rp::peripherals::PIN_27,
        pin28: embassy_rp::peripherals::PIN_28,
    ) -> Self {
        // Initialize ADC for joystick input (Pins 27 and 28)
        let adc_reader = Adc::new(adc, Irqs, Config::default());
        let adc_pin_x = Channel::new_pin(pin27, Pull::None);
        let adc_pin_y = Channel::new_pin(pin28, Pull::None);

        // Initialize button (Pin 16)
        let button_pin = Input::new(pin16, Pull::Up);
        let button_controller = ButtonController::new(button_pin);

        // Spawn button task
        spawner.spawn(button_task(button_controller)).unwrap();

        Self {
            adc: adc_reader,
            pin_x: adc_pin_x,
            pin_y: adc_pin_y,
        }
    }

    pub async fn read_x(&mut self) -> i8 {
        let adc_val = self.adc.read(&mut self.pin_x).await.unwrap();
        match adc_val {
            0..=1800 => 1,
            2300..=4096 => -1,
            _ => 0,
        }
    }

    pub async fn read_y(&mut self) -> i8 {
        let adc_val = self.adc.read(&mut self.pin_y).await.unwrap();
        match adc_val {
            0..=1800 => -1,
            2300..=4096 => 1,
            _ => 0,
        }
    }

    pub fn was_pressed(&self) -> bool {
        button_was_pressed()
    }
}
