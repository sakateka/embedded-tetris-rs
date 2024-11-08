#![cfg_attr(not(test), no_main)] 
#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, allow(dead_code))]
#![cfg_attr(test, allow(unused))]

mod figure;

use core::ops::Index;

#[cfg(not(test))]
use nrf52833_hal::Rng;
use smart_leds::{colors, RGB8};
use smart_leds_trait::SmartLedsWrite;
use ws2812_nrf52833_pwm::Ws2812;

#[cfg(not(test))]
use cortex_m_rt::entry;
use embedded_hal::delay::DelayNs;
use microbit::{board::Board, hal::Timer};
#[cfg(not(test))]
use rtt_target::{rprintln, rtt_init_print};

#[cfg(not(test))]
use panic_halt as _;

use figure::{Figure, Digits, Tetramino};

const SCREEN_WIDTH: usize = 8;
const SCREEN_HEIGHT: usize = 32;

#[derive(Copy, Clone, Debug)]
enum ColorIdx {
    Brick,
    Red,
    Green,
    Blue,
    Pink,
    Yellow,
    ColorArraySize,
}
const C_IDX_BRICK: usize = ColorIdx::Brick as usize;
const C_IDX_RED: usize = ColorIdx::Red as usize;
const C_IDX_GREEN: usize = ColorIdx::Green as usize;
const C_IDX_BLUE: usize = ColorIdx::Blue as usize;
const C_IDX_PINK: usize = ColorIdx::Pink as usize;
const C_IDX_YELLOW: usize = ColorIdx::Yellow as usize;

impl From<usize> for ColorIdx {
    fn from(value: usize) -> Self {
        match value {
            C_IDX_BRICK => ColorIdx::Brick,
            C_IDX_RED => ColorIdx::Red,
            C_IDX_GREEN => ColorIdx::Green,
            C_IDX_BLUE => ColorIdx::Blue,
            C_IDX_PINK => ColorIdx::Pink,
            C_IDX_YELLOW => ColorIdx::Yellow,
            _ => panic!("out of range {}", value),
        }
    }
}

type ColorsType = [RGB8; ColorIdx::ColorArraySize as usize];

const COLORS: ColorsType = [
    RGB8::new(12, 2, 0),
    RGB8::new(6, 0, 0),
    RGB8::new(0, 6, 0),
    RGB8::new(0, 0, 6),
    RGB8::new(3, 0, 3),
    RGB8::new(6, 6, 0),
];

impl Index<ColorIdx> for ColorsType {
    type Output = RGB8;
    fn index(&self, index: ColorIdx) -> &Self::Output {
        &self[index as usize]
    }
}

trait ColorsIndexer<'a> {
    fn at(&self, idx: usize) -> RGB8;

}

impl<'a> ColorsIndexer<'a> for ColorsType {
    fn at(&self, idx: usize) -> RGB8 {
        if idx >= self.len() {
            return colors::RED; // bright RED indicates an error
        }
        self[idx]
    }
}

fn dot(m: &mut [RGB8], x: u8, y: u8, color: RGB8) -> bool {
    let mut x = x;
    if y & 1 != 1 {
        x = 7 - x;
    }
    m[SCREEN_WIDTH * y as usize + x as usize] = color;

    true
}

fn clear(m: &mut [RGB8]) {
    (0..m.len()).for_each(|idx| {
        m[idx] = colors::BLACK;
    });
}

include!(concat!(env!("OUT_DIR"), "/figures.rs"));

#[cfg(not(test))]
#[entry]
fn main() -> ! {
    rtt_init_print!();
    rprintln!("message from build.rs:\n{}", DIGITS.wrapping_at(8).str());
    let board = Board::take().unwrap();
    let mut timer = Timer::new(board.TIMER0);
    let pin = board.edge.e16.degrade();
    let mut ws2812: Ws2812<{ 256 * 24 }, _> = Ws2812::new(board.PWM0, pin);
    let mut leds: [RGB8; 256] = [RGB8::default(); 256];

    let mut r = Rng::new(board.RNG);
    rprintln!("created figure(1):\n{}", DIGITS.wrapping_at(1).str());

    let x = 3;
    let mut y = 4;

    rprintln!("starting loop");
    let mut digit_idx: u8 = 0;
    loop {
        let color = COLORS.at(r.random_u8() as usize % COLORS.len());
        clear(&mut leds);
        rprintln!("draw at x={} y={} color={:?}", x, y, color);
        digit_idx += 1;
        let mut digit = DIGITS.wrapping_at(digit_idx);
        if digit_idx & 1 == 1 {
           digit = digit.rotate();
        }

        _ = digit.draw(&mut leds, x, y, color, dot);
        ws2812.write(leds).unwrap();
        rprintln!("sleep");
        timer.delay_ms(1000);
        y += 1;
        if (y + digit.height()) as usize  > SCREEN_HEIGHT {
            y = 0;
        }
    }
}

#[cfg(test)]
fn main() {}
