#![no_main]
#![no_std]


use nrf52833_hal::Rng;
use smart_leds::RGB8;
use smart_leds_trait::SmartLedsWrite;
use ws2812_nrf52833_pwm::Ws2812;

use cortex_m_rt::entry;
use embedded_hal::delay::DelayNs;
use microbit::{board::Board, hal::Timer};
use rtt_target::{rprintln, rtt_init_print};

use panic_halt as _;

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let board = Board::take().unwrap();
    let mut timer = Timer::new(board.TIMER0);
    let pin = board.edge.e16.degrade();
    let mut ws2812: Ws2812<{ 256 * 24 }, _> = Ws2812::new(board.PWM0, pin);

    let leds = [
        RGB8::new(12, 0, 0),
        RGB8::new(0, 12, 0),
        RGB8::new(0, 0, 12),
        RGB8::new(12, 12, 0),
        RGB8::new(0, 12, 12),
        RGB8::new(12, 0, 12),
        RGB8::new(10, 10, 10),
        RGB8::new(12, 6, 0),
    ];

    rprintln!("starting");

    ws2812.write(leds[..4].iter().cloned()).unwrap();

    rprintln!("displaying indices");

    rprintln!("starting loop");

    let nleds = leds.len();
    let mut cur_leds: [RGB8; 256] = [RGB8::default();256];
    let mut r = Rng::new(board.RNG);
    loop {
        let idx = r.random_u8();
        let color = r.random_u8();
        cur_leds[idx as usize] = leds[color as usize % nleds];
        ws2812.write(cur_leds).unwrap();
        timer.delay_ms(1);
    }
}
