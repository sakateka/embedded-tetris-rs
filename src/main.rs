#![cfg_attr(not(test), no_main)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, allow(dead_code))]
#![cfg_attr(test, allow(unused))]

mod figure;

#[cfg(not(test))]
use nrf52833_hal::Rng;
use smart_leds::{colors, RGB8};
use smart_leds_trait::SmartLedsWrite;
use ws2812_nrf52833_pwm::Ws2812;

#[cfg(not(test))]
use cortex_m_rt::entry;
use embedded_hal::{delay::DelayNs, digital::InputPin};
use microbit::{board::Board, hal::Timer};
#[cfg(not(test))]
use rtt_target::{rprintln, rtt_init_print};

#[cfg(not(test))]
use panic_halt as _;

use figure::{Digits, Figure, Tetramino};

const SCREEN_WIDTH: usize = 8;
const SCREEN_HEIGHT: usize = 32;

const BRICK: RGB8 = RGB8::new(12, 2, 0);
const RED: RGB8 = RGB8::new(6, 0, 0);
const GREEN: RGB8 = RGB8::new(0, 6, 0);
const BLUE: RGB8 = RGB8::new(0, 0, 6);
const PINK: RGB8 = RGB8::new(3, 0, 3);
const YELLOW: RGB8 = RGB8::new(6, 6, 0);

type ColorsType = [RGB8; 6];
const COLORS: ColorsType = [BRICK, RED, GREEN, BLUE, PINK, YELLOW];

trait ColorsIndexer<'a> {
    fn at(&self, idx: u8) -> RGB8;
}

impl<'a> ColorsIndexer<'a> for ColorsType {
    fn at(&self, idx: u8) -> RGB8 {
        let mut idx = idx as usize;
        while idx >= self.len() {
            idx -= self.len();
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

fn draw_score(m: &mut [RGB8], score: u8) {
    let speed = score / 10;
    let score = DIGITS.wrapping_at(score);
    let speed = DIGITS.wrapping_at(speed);
    speed.draw(m, 0, 0, GREEN, dot);
    score.draw(m, 4, 0, GREEN, dot);
}

include!(concat!(env!("OUT_DIR"), "/figures.rs"));

// #[cfg(not(test))]
#[entry]
fn main() -> ! {
    rtt_init_print!();
    rprintln!("message from build.rs:\n{}", DIGITS.wrapping_at(8).str());
    let board = Board::take().unwrap();
    let mut timer = Timer::new(board.TIMER0);
    let pin = board.edge.e16.degrade();
    let mut push = board.edge.e08.into_pullup_input();
    let mut ws2812: Ws2812<{ 256 * 24 }, _> = Ws2812::new(board.PWM0, pin);
    let mut leds: [RGB8; 256] = [RGB8::default(); 256];

    let mut r = Rng::new(board.RNG);

    let mut score = 0;
    let mut rotate = false;

    let mut x = 3;
    let mut y = 6;
    let mut pass = 0;

    let mut digit_idx: u8 = 0;
    let mut digit = DIGITS.wrapping_at(digit_idx);
    let mut color = COLORS.at(r.random_u8());

    rprintln!("starting loop");
    loop {
        rotate = rotate || push.is_high().unwrap();

        if pass >= 10 {
            pass = 0;
            y += 1;
            color = COLORS.at(r.random_u8());
        }

        clear(&mut leds);
        draw_score(&mut leds, score);
        HLINE.draw(&mut leds, 0, 5, PINK, dot);

        if rotate {
            digit = digit.rotate();
            rotate = false;
        }

        _ = digit.draw(&mut leds, x, y, color, dot);
        ws2812.write(leds).unwrap();
        if (y + digit.height()) as usize >= SCREEN_HEIGHT {
            digit_idx += 1;
            digit = DIGITS.wrapping_at(digit_idx);
            y = 6;
            score += 1;
        }

        pass += 1;
        timer.delay_ms(50);
    }
}

#[cfg(test)]
fn main() {}
