#![deny(unsafe_code)]
#![no_main]
#![no_std]

use cortex_m_rt::entry;
use microbit::{board::Board, display::blocking::Display};
use microbit::hal::Timer;
use embedded_hal::delay::DelayNs;
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let board = Board::take().unwrap();
    let mut timer = Timer::new(board.TIMER0);
    let mut display = Display::new(board.display_pins);
    rprintln!("hello world");

    let heart = [
        [0, 1, 0, 1, 0],
        [1, 1, 1, 1, 1],
        [1, 1, 1, 1, 1],
        [0, 1, 1, 1, 0],
        [0, 0, 1, 0, 0],
    ];

    let small_heart = [
        [0, 0, 0, 0, 0],
        [0, 1, 0, 1, 0],
        [0, 1, 1, 1, 0],
        [0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0],
    ];

    loop {
        rprintln!("Light!");
        display.show(&mut timer, small_heart, 100);
        // Show light_it_all for 1000ms
        rprintln!("Light!");
        display.show(&mut timer, heart, 1000);
        // clear the display again
        rprintln!("Light!");
        display.show(&mut timer, small_heart, 100);
        rprintln!("Dark!");
        display.clear();
        timer.delay_ms(1000_u32);
    }
}
