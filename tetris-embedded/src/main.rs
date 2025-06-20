#![no_std]
#![no_main]

use crate::control::{button_task, ButtonController, Joystick};
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

    let adc_reader = Adc::new(p.ADC, Irqs, Config::default());
    let adc_pin_x = Channel::new_pin(p.PIN_27, Pull::None);
    let adc_pin_y = Channel::new_pin(p.PIN_28, Pull::None);
    let button_pin = Input::new(p.PIN_16, Pull::Up);
    let button_controller = ButtonController::new(button_pin);

    // Spawn button task
    spawner.spawn(button_task(button_controller)).unwrap();

    let mut joystick = Joystick::new(adc_reader, adc_pin_x, adc_pin_y);
    let timer = EmbeddedTimer;

    info!("Starting main menu loop");
    run_game_menu(&mut display, &mut joystick, &timer, || {
        Instant::now().as_ticks() as u32
    }).await;
}
