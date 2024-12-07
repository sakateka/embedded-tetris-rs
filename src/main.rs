#![cfg_attr(not(test), no_main)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, allow(dead_code))]
#![cfg_attr(test, allow(unused))]

mod control;
mod figure;

use control::{button_was_pressed, init_button};
#[cfg(not(test))]
use nrf52833_hal::Rng;
use nrf52833_hal::{saadc::SaadcConfig, Saadc};
use smart_leds::{colors, RGB8};
use smart_leds_trait::SmartLedsWrite;
use ws2812_nrf52833_pwm::Ws2812;

#[cfg(not(test))]
use cortex_m_rt::entry;
use embedded_hal::delay::DelayNs;
use microbit::{adc::Adc, board::Board, hal::Timer};
#[cfg(not(test))]
use rtt_target::{rprintln, rtt_init_print};

#[cfg(not(test))]
use panic_rtt_target as _;
// use panic_halt as _;

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

trait ColorsIndexer {
    fn at(&self, idx: u8) -> RGB8;
}

impl ColorsIndexer for ColorsType {
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


fn move_x(val: Result<i16, ()>, x: &mut u8, width: u8) {
    if let Ok(v) = val {
        match v {
            0..300 if SCREEN_WIDTH as u8 > *x + width => *x += 1,
            16000..16384 if *x > 0 =>  *x -= 1,
            _ => {},
        }
        rprintln!("val: {}", v);
    }
}

fn move_y(val: Result<i16, ()>, y: &mut u8, height: u8) {
    if let Ok(v) = val {
        match v {
            16000..16384 if SCREEN_HEIGHT as u8 > *y + height =>  *y += 1,
            _ => {},
        }
    }
}

// #[cfg(not(test))]
#[entry]
fn main() -> ! {
    rtt_init_print!();
    rprintln!("message from build.rs:\n{}", DIGITS.wrapping_at(8).str());
    let board = Board::take().unwrap();
    let pin = board.edge.e16.degrade();
    init_button(board.GPIOTE, board.edge.e08.into_floating_input().into());

    let mut adc: Saadc =  Saadc::new(board.ADC, SaadcConfig::default());
    let mut left_right = board.edge.e01.into_floating_input();
    let mut up_down = board.edge.e02.into_floating_input();

    let mut timer = Timer::new(board.TIMER0);

    let mut ws2812: Ws2812<{ 256 * 24 }, _> = Ws2812::new(board.PWM0, pin);
    let mut leds: [RGB8; 256] = [RGB8::default(); 256];

    let mut r = Rng::new(board.RNG);

    let mut score = 0;
    let mut x = 3;
    let mut y = 6;
    let mut pass = 0;

    let mut digit_idx: u8 = 0;
    let mut digit = DIGITS.wrapping_at(digit_idx);
    let mut color = COLORS.at(r.random_u8());

    rprintln!("starting loop");
    loop {
        if pass >= 10 {
            pass = 0;
            y += 1;
            color = COLORS.at(r.random_u8());
        }

        move_x(adc.read_channel(&mut left_right), &mut x, digit.width());
        move_y(adc.read_channel(&mut up_down), &mut y, digit.height());

        clear(&mut leds);
        draw_score(&mut leds, score);
        HLINE.draw(&mut leds, 0, 5, PINK, dot);

        if button_was_pressed(true) {
            digit = digit.rotate();
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
