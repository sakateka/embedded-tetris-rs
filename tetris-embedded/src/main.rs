#![no_std]
#![no_main]

use crate::control::{
    button_a_task, button_b_task, joystick_button_task, ButtonHardware, Control, Joystick,
};
use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::adc::InterruptHandler as AdcInterruptHandler;
use embassy_rp::adc::{Adc, Channel, Config};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::pio_programs::ws2812::{PioWs2812, PioWs2812Program};
use embassy_time::{Instant, Timer as EmbassyTimer};
use smart_leds::RGB8;
use tetris_lib::common::{LedDisplay, Timer};
use tetris_lib::games::run_game_menu;
use {defmt_rtt as _, panic_probe as _};

mod control;

// Embedded timer implementation
pub struct EmbeddedTimer;

impl Timer for EmbeddedTimer {
    async fn sleep_millis(&self, millis: u64) {
        EmbassyTimer::after_millis(millis).await;
    }
}

// Wrapper type to implement LedDisplay for PioWs2812
pub struct Ws2812Display<'a>(PioWs2812<'a, PIO0, 0, 256>);

impl<'a> Ws2812Display<'a> {
    pub fn new(ws2812: PioWs2812<'a, PIO0, 0, 256>) -> Self {
        Self(ws2812)
    }
}

// Implement LedDisplay for our wrapper
impl LedDisplay for Ws2812Display<'_> {
    async fn write(&mut self, leds: &[RGB8; 256]) {
        self.0.write(leds).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Starting game collection!");

    bind_interrupts!(struct Irqs {
        PIO0_IRQ_0 => InterruptHandler<PIO0>;
        ADC_IRQ_FIFO => AdcInterruptHandler;
    });

    let p = embassy_rp::init(Default::default());
    // Initialize PIO for WS2812
    let Pio {
        mut common, sm0, ..
    } = Pio::new(p.PIO0, Irqs);

    let program = PioWs2812Program::new(&mut common);
    let ws2812 = PioWs2812::new(&mut common, sm0, p.DMA_CH0, p.PIN_13, &program);
    let mut display = Ws2812Display::new(ws2812);

    // Hardware setup
    let adc_reader = Adc::new(p.ADC, Irqs, Config::default());
    let adc_pin_x = Channel::new_pin(p.PIN_27, Pull::None);
    let adc_pin_y = Channel::new_pin(p.PIN_28, Pull::None);
    let joystick_push_pin = Input::new(p.PIN_16, Pull::Up);
    let button_a_pin = Input::new(p.PIN_0, Pull::Up);
    let button_b_pin = Input::new(p.PIN_1, Pull::Up);

    // Create hardware button controllers
    let joystick_button_hw = ButtonHardware::new_joystick_button(joystick_push_pin);
    let button_a_hw = ButtonHardware::new_button_a(button_a_pin);
    let button_b_hw = ButtonHardware::new_button_b(button_b_pin);

    // Spawn button tasks - these will run independently and signal through static signals
    spawner
        .spawn(joystick_button_task(joystick_button_hw))
        .unwrap();
    spawner.spawn(button_a_task(button_a_hw)).unwrap();
    spawner.spawn(button_b_task(button_b_hw)).unwrap();

    // Create game controller (no longer needs to own button hardware)
    let joystick = Joystick::new(adc_reader, adc_pin_x, adc_pin_y);
    let mut control = Control::new(joystick);
    let timer = EmbeddedTimer;

    info!("Starting main menu loop");
    run_game_menu(&mut display, &mut control, &timer, || {
        Instant::now().as_ticks() as u32
    })
    .await;
}
