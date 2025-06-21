use core::marker::Sized;
use embassy_rp::adc::{Adc, Channel};
use embassy_rp::gpio::Input;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

// Shared signals accessible from multiple tasks
pub static JOYSTICK_BUTTON_SIGNAL: Signal<CriticalSectionRawMutex, bool> = Signal::new();
pub static BUTTON_A_SIGNAL: Signal<CriticalSectionRawMutex, bool> = Signal::new();
pub static BUTTON_B_SIGNAL: Signal<CriticalSectionRawMutex, bool> = Signal::new();

// Hardware button wrapper for running in tasks
pub struct ButtonHardware {
    button: Input<'static>,
    signal: &'static Signal<CriticalSectionRawMutex, bool>,
}

impl ButtonHardware {
    pub fn new_joystick_button(button: Input<'static>) -> Self {
        Self {
            button,
            signal: &JOYSTICK_BUTTON_SIGNAL,
        }
    }

    pub fn new_button_a(button: Input<'static>) -> Self {
        Self {
            button,
            signal: &BUTTON_A_SIGNAL,
        }
    }

    pub fn new_button_b(button: Input<'static>) -> Self {
        Self {
            button,
            signal: &BUTTON_B_SIGNAL,
        }
    }

    pub async fn run(mut self) -> ! {
        loop {
            // Wait for falling edge interrupt (button press)
            self.button.wait_for_falling_edge().await;

            // Signal that button was pressed
            self.signal.signal(true);

            // Debounce delay
            embassy_time::Timer::after_millis(200).await;
        }
    }
}

// Tasks for handling hardware buttons
#[embassy_executor::task]
pub async fn joystick_button_task(button_hardware: ButtonHardware) {
    button_hardware.run().await;
}

#[embassy_executor::task]
pub async fn button_a_task(button_hardware: ButtonHardware) {
    button_hardware.run().await;
}

#[embassy_executor::task]
pub async fn button_b_task(button_hardware: ButtonHardware) {
    button_hardware.run().await;
}

// Joystick controller that doesn't own the button
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
            0..=1600 => 1,
            2500..=4096 => -1,
            _ => 0,
        }
    }

    pub async fn read_y(&mut self) -> i8 {
        let adc_val = self.adc.read(&mut self.pin_y).await.unwrap();
        match adc_val {
            0..=1600 => -1,
            2500..=4096 => 1,
            _ => 0,
        }
    }
}

// Main game controller that uses shared signals
pub struct Control<'a> {
    joystick: Joystick<'a>,
}

impl<'a> Control<'a> {
    pub fn new(joystick: Joystick<'a>) -> Self {
        Self { joystick }
    }
}

// Implement the GameController trait
impl<'a> tetris_lib::common::GameController for Control<'a> {
    async fn read_x(&mut self) -> i8 {
        self.joystick.read_x().await
    }

    async fn read_y(&mut self) -> i8 {
        self.joystick.read_y().await
    }

    fn joystick_was_pressed(&self) -> bool {
        JOYSTICK_BUTTON_SIGNAL.try_take().unwrap_or(false)
    }

    fn a_was_pressed(&self) -> bool {
        BUTTON_A_SIGNAL.try_take().unwrap_or(false)
    }

    fn b_was_pressed(&self) -> bool {
        BUTTON_B_SIGNAL.try_take().unwrap_or(false)
    }
}
