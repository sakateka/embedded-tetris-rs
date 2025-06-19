use core::marker::Sized;
use embassy_rp::adc::{Adc, Channel};
use embassy_rp::gpio::Input;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

static BUTTON_SIGNAL: Signal<CriticalSectionRawMutex, bool> = Signal::new();

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
pub async fn button_task(mut button_controller: ButtonController) {
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
        adc_reader: embassy_rp::adc::Adc<'a, embassy_rp::adc::Async>,
        adc_pin_x: embassy_rp::adc::Channel<'a>,
        adc_pin_y: embassy_rp::adc::Channel<'a>,
    ) -> Self {
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

// Implement the GameController trait for Joystick
impl<'a> tetris_lib::common::GameController for Joystick<'a> {
    async fn read_x(&mut self) -> i8 {
        self.read_x().await
    }

    async fn read_y(&mut self) -> i8 {
        self.read_y().await
    }

    fn was_pressed(&self) -> bool {
        self.was_pressed()
    }
}
