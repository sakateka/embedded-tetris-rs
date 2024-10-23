#![deny(unsafe_code)]
#![no_main]
#![no_std]

use cortex_m_rt::entry;
use microbit::{board::Board, display::blocking::Display};
use microbit::hal::Timer;
// use embedded_hal::delay::DelayNs;
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};

fn clear(leds: &mut [[u8; 5]; 5]) {
    for r in leds {
        for i in r {
            *i = 0;
        }
    }
}
const DIRECTIONS: [(i8, i8); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];


#[entry]
fn main() -> ! {
    rtt_init_print!();
    let board = Board::take().unwrap();
    let mut timer = Timer::new(board.TIMER0);
    let mut display = Display::new(board.display_pins);
    rprintln!("hello world");

    let mut leds = [
        [1, 0, 0, 0, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0],
    ];


    let mut pos: (i8, i8) = (0, 0);
    loop {
        for dir in DIRECTIONS {
            loop {
                rprintln!("Roll! {:?} -> {:?}", dir, pos);
                let new_y = pos.0 + dir.0;
                if new_y < 0 || new_y >= leds[0].len() as i8 {
                    break
                }
                let new_x = pos.1 + dir.1;
                if new_x < 0 || new_x >= leds.len() as i8 {
                    break
                }
                display.show(&mut timer, leds, 500);
                clear(&mut leds);
                pos.0 = new_y;
                pos.1 = new_x;
                leds[pos.0 as usize][pos.1 as usize] = 1;
            }
        }
    }
}
