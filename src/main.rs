#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::adc::{Adc, Channel, Config, InterruptHandler as AdcInterruptHandler};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::{ADC, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::pio_programs::ws2812::{PioWs2812, PioWs2812Program};
use embassy_time::{Duration, Ticker, Timer};
use smart_leds::{colors, RGB8};
use {defmt_rtt as _, panic_probe as _};

mod control;
mod figure;

use control::{button_was_pressed, ButtonController};
use figure::{Digits, Figure, Tetramino};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
    ADC_IRQ_FIFO => AdcInterruptHandler;
});

const SCREEN_WIDTH: usize = 8;
const SCREEN_HEIGHT: usize = 32;
const SCREEN_SIZE: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

// Colors matching the Python version
const BLACK: RGB8 = RGB8::new(0, 0, 0);
const BRICK: RGB8 = RGB8::new(12, 2, 0);
const RED: RGB8 = RGB8::new(6, 0, 0);
const GREEN: RGB8 = RGB8::new(0, 6, 0);
const BLUE: RGB8 = RGB8::new(0, 0, 6);
const LIGHT_BLUE: RGB8 = RGB8::new(0, 6, 6);
const PINK: RGB8 = RGB8::new(3, 0, 3);
const YELLOW: RGB8 = RGB8::new(6, 6, 0);
const DARK_GREEN: RGB8 = RGB8::new(0, 3, 0);
const LIGHT_GREEN: RGB8 = RGB8::new(0, 9, 0);

// Color indices
const BLACK_IDX: u8 = 0;
const BRICK_IDX: u8 = 1;
const RED_IDX: u8 = 2;
const GREEN_IDX: u8 = 3;
const BLUE_IDX: u8 = 4;
const LIGHT_BLUE_IDX: u8 = 5;
const PINK_IDX: u8 = 6;
const YELLOW_IDX: u8 = 7;
const DARK_GREEN_IDX: u8 = 8;
const LIGHT_GREEN_IDX: u8 = 9;

type ColorsType = [RGB8; 10];
const COLORS: ColorsType = [
    BLACK,
    BRICK,
    RED,
    GREEN,
    BLUE,
    LIGHT_BLUE,
    PINK,
    YELLOW,
    DARK_GREEN,
    LIGHT_GREEN,
];

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

// Simple PRNG implementation
struct Prng {
    state: u32,
}

impl Prng {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u8 {
        self.state = self.state.wrapping_mul(1103515245).wrapping_add(12345);
        (self.state >> 16) as u8
    }

    fn next_range(&mut self, max: u8) -> u8 {
        if max == 0 {
            return 0;
        }
        self.next() % max
    }
}

// Point/Dot structure for coordinates
#[derive(Copy, Clone, Debug, PartialEq)]
struct Dot {
    x: i8,
    y: i8,
}

impl Dot {
    fn new(x: i8, y: i8) -> Self {
        Self { x, y }
    }

    fn move_by(&self, direction: Dot) -> Dot {
        Dot::new(self.x + direction.x, self.y + direction.y)
    }

    fn move_wrap(&self, direction: Dot) -> Dot {
        let mut new_dot = self.move_by(direction);

        if new_dot.x == -1 {
            new_dot.x = SCREEN_WIDTH as i8 - 1;
        } else if new_dot.x == SCREEN_WIDTH as i8 {
            new_dot.x = 0;
        }

        if new_dot.y == -1 {
            new_dot.y = SCREEN_HEIGHT as i8 - 1;
        } else if new_dot.y == SCREEN_HEIGHT as i8 {
            new_dot.y = 0;
        }

        new_dot
    }

    fn is_zero(&self) -> bool {
        self.x == 0 && self.y == 0
    }

    fn is_opposite(&self, other: &Dot) -> bool {
        (self.x + other.x) == 0 && (self.y + other.y) == 0
    }

    fn opposite(&self) -> Dot {
        Dot::new(-self.x, -self.y)
    }

    fn outside(&self) -> bool {
        self.x < 0 || self.x >= SCREEN_WIDTH as i8 || self.y < 0 || self.y >= SCREEN_HEIGHT as i8
    }
}

// Frame buffer for screen management
struct FrameBuffer {
    content: [u8; SCREEN_SIZE],
}

impl FrameBuffer {
    fn new() -> Self {
        Self {
            content: [0; SCREEN_SIZE],
        }
    }

    fn clear(&mut self) {
        self.content.fill(0);
    }

    fn clear_range(&mut self, from: usize, to: usize) {
        for idx in from..to.min(SCREEN_SIZE) {
            self.content[idx] = 0;
        }
    }

    fn set(&mut self, x: usize, y: usize, color: u8) {
        if x < SCREEN_WIDTH && y < SCREEN_HEIGHT {
            let idx = y * SCREEN_WIDTH + x;
            self.content[idx] = color;
        }
    }

    fn get(&self, x: usize, y: usize) -> u8 {
        if x < SCREEN_WIDTH && y < SCREEN_HEIGHT {
            self.content[y * SCREEN_WIDTH + x]
        } else {
            0
        }
    }

    fn available(&self, x: i8, y: i8, color: u8) -> bool {
        if x >= 0 && x < SCREEN_WIDTH as i8 && y >= 0 && y < SCREEN_HEIGHT as i8 {
            self.content[y as usize * SCREEN_WIDTH + x as usize] == color
        } else {
            false
        }
    }

    fn collides(&self, x: i8, y: i8, figure: &Figure) -> bool {
        for row in 0..figure.height() {
            for col in 0..figure.width() {
                if figure.get_bit(col, row)
                    && !self.available(x + col as i8, y + row as i8, BLACK_IDX)
                {
                    return true;
                }
            }
        }
        false
    }

    fn draw_figure(&mut self, x: i8, y: i8, figure: &Figure, color: u8) {
        for row in 0..figure.height() {
            for col in 0..figure.width() {
                if figure.get_bit(col, row) {
                    let px = x + col as i8;
                    let py = y + row as i8;
                    if px >= 0 && px < SCREEN_WIDTH as i8 && py >= 0 && py < SCREEN_HEIGHT as i8 {
                        self.set(px as usize, py as usize, color);
                    }
                }
            }
        }
    }

    fn copy_from(&mut self, other: &FrameBuffer) {
        self.content.copy_from_slice(&other.content);
    }

    fn render(&self, leds: &mut [RGB8]) {
        for x in 0..SCREEN_WIDTH {
            for y in 0..SCREEN_HEIGHT {
                let color_idx = self.get(x, y);
                set_pixel(leds, x, y, color_idx);
            }
        }
    }

    fn row_is_full(&self, row: usize) -> bool {
        if row >= SCREEN_HEIGHT {
            return false;
        }
        for x in 0..SCREEN_WIDTH {
            if self.content[row * SCREEN_WIDTH + x] == 0 {
                return false;
            }
        }
        true
    }

    fn row_is_empty(&self, row: usize) -> bool {
        if row >= SCREEN_HEIGHT {
            return true;
        }
        for x in 0..SCREEN_WIDTH {
            if self.content[row * SCREEN_WIDTH + x] != 0 {
                return false;
            }
        }
        true
    }
}

fn set_pixel(leds: &mut [RGB8], x: usize, y: usize, color_idx: u8) {
    let mut x = x;
    if y % 2 == 0 {
        x = 7 - x;
    }
    let idx = SCREEN_WIDTH * y + x;
    if idx < leds.len() {
        leds[idx] = COLORS.at(color_idx);
    }
}

// Joystick handling
struct Joystick<'a> {
    adc: Adc<'a, embassy_rp::adc::Async>,
    pin_x: Channel<'a>,
    pin_y: Channel<'a>,
}

impl<'a> Joystick<'a> {
    fn new(
        spawner: Spawner,
        adc: embassy_rp::peripherals::ADC,
        pin16: embassy_rp::peripherals::PIN_16,
        pin27: embassy_rp::peripherals::PIN_27,
        pin28: embassy_rp::peripherals::PIN_28,
    ) -> Self {
        // Initialize ADC for joystick input (Pins 27 and 28)
        let adc_reader = Adc::new(adc, Irqs, Config::default());
        let adc_pin_x = Channel::new_pin(pin27, Pull::None);
        let adc_pin_y = Channel::new_pin(pin28, Pull::None);

        // Initialize button (Pin 16)
        let button_pin = Input::new(pin16, Pull::Up);
        let button_controller = ButtonController::new(button_pin);

        // Spawn button task
        spawner.spawn(button_task(button_controller)).unwrap();

        Self {
            adc: adc_reader,
            pin_x: adc_pin_x,
            pin_y: adc_pin_y,
        }
    }

    async fn read_x(&mut self) -> i8 {
        let adc_val = self.adc.read(&mut self.pin_x).await.unwrap();
        match adc_val {
            0..=1800 => 1,
            2300..=4096 => -1,
            _ => 0,
        }
    }

    async fn read_y(&mut self) -> i8 {
        let adc_val = self.adc.read(&mut self.pin_y).await.unwrap();
        match adc_val {
            0..=1800 => -1,
            2300..=4096 => 1,
            _ => 0,
        }
    }

    fn was_pressed(&self) -> bool {
        button_was_pressed()
    }
}

include!(concat!(env!("OUT_DIR"), "/figures.rs"));

// Game title graphics (converted from Python GAMES array)
// const TETRIS_TITLE: [u32; 8] = [
//     0o_00000000000000000000000000000000,
//     0o_00000000000000000000000000000000,
//     0o_03330033300333003330030030033300,
//     0o_00300030000030003030030030030000,
//     0o_00300033300030003330030330030000,
//     0o_00300030000030003000033030030000,
//     0o_00300033300030003000030030033300,
//     0o_00000000000000000000000000000000,
// ];

// const RACES_TITLE: [u32; 8] = [
//     0o_00000000000000000000000000000000,
//     0o_00000000000000000000000000000000,
//     0o_00003330033300303003030030030000,
//     0o_00003000030300303003300030030000,
//     0o_00003000030300333003300030330000,
//     0o_00003000030300303003030033030000,
//     0o_00003000033300303003030030030000,
//     0o_00000000000000000000000000000000,
// ];

// const TANKS_TITLE: [u32; 8] = [
//     0o_00000000000000000000000000000000,
//     0o_00000000000000000000000000000000,
//     0o_00003330003000303003030030030000,
//     0o_00000300030300303003300030030000,
//     0o_00000300033300333003300030330000,
//     0o_00000300030300303003030033030000,
//     0o_00000300030300303003030030030000,
//     0o_00000000000000000000000000000000,
// ];

// const SNAKE_TITLE: [u32; 8] = [
//     0o_00000000000000000000000000000000,
//     0o_00000000000000000030000000000000,
//     0o_03300330330333003000303003003300,
//     0o_00030303030300003003303030030030,
//     0o_03300300030333003030303300033330,
//     0o_00030300030300003300303030030030,
//     0o_03300300030333003000303003030030,
//     0o_00000000000000000000000000000000,
// ];

// // Game titles array
// const GAME_TITLES: [&[u32; 8]; 4] = [
//     &TETRIS_TITLE,
//     &RACES_TITLE,
//     &TANKS_TITLE,
//     &SNAKE_TITLE,
// ];

// Function to create FrameBuffer from title rows (like Python's from_rows)
fn framebuffer_from_rows(rows: &[u32; 8]) -> FrameBuffer {
    let mut buffer = FrameBuffer::new();

    for y in 0..SCREEN_HEIGHT {
        for x in 0..rows.len() {
            if x < SCREEN_WIDTH {
                let color = (rows[x] >> ((SCREEN_HEIGHT - y - 1) * 3)) & 0b111;
                buffer.set(SCREEN_WIDTH - x - 1, y, color as u8);
            }
        }
    }

    buffer
}

// Helper functions that were removed but still needed
fn clear(leds: &mut [RGB8]) {
    leds.fill(BLACK);
}

fn can_draw(_leds: &mut [RGB8], x: u8, y: u8, _color: RGB8) -> bool {
    x < SCREEN_WIDTH as u8 && y < SCREEN_HEIGHT as u8
}

fn dot(leds: &mut [RGB8], x: u8, y: u8, color: RGB8) -> bool {
    let mut x = x;
    if y & 1 != 1 {
        x = 7 - x;
    }
    if (x as usize) < SCREEN_WIDTH && (y as usize) < SCREEN_HEIGHT {
        leds[SCREEN_WIDTH * y as usize + x as usize] = color;
    }
    true
}

fn draw_score(leds: &mut [RGB8], score: u8) {
    let speed = score / 10;
    let score_digit = score % 10;
    let speed_fig = DIGITS.wrapping_at(speed);
    let score_fig = DIGITS.wrapping_at(score_digit);
    speed_fig.draw(leds, 0, 0, GREEN, dot);
    score_fig.draw(leds, 4, 0, GREEN, dot);
}

fn move_x(v: u16, x: &mut u8, width: u8) -> Result<(), ()> {
    match v {
        0..=16000 if *x > 0 => *x -= 1,
        49000..=65535 if SCREEN_WIDTH as u8 > *x + width => *x += 1,
        _ => {}
    }
    Ok(())
}

fn move_y(v: u16, y: &mut u8, height: u8) -> Result<(), ()> {
    match v {
        49000..=65535 if SCREEN_HEIGHT as u8 > *y + height => *y += 1,
        _ => {}
    }
    Ok(())
}

// Game trait for different game implementations
trait Game {
    async fn run(
        &mut self,
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        joystick: &mut Joystick<'_>,
    );
}

// Tetris game implementation
struct TetrisGame {
    screen: FrameBuffer,
    concrete: FrameBuffer,
    score: u8,
    init_x: i8,
    init_y: i8,
    next_visible_y: i8,
    prng: Prng,
}

impl TetrisGame {
    fn new() -> Self {
        Self {
            screen: FrameBuffer::new(),
            concrete: FrameBuffer::new(),
            score: 0,
            init_x: 3,
            init_y: 6,
            next_visible_y: 11,
            prng: Prng::new(12345),
        }
    }

    fn draw_score(&mut self) {
        self.score %= 100;
        let speed = self.score / 10;
        let score_digit = self.score % 10;

        let speed_fig = DIGITS.wrapping_at(speed);
        let score_fig = DIGITS.wrapping_at(score_digit);

        self.screen.draw_figure(0, 0, &speed_fig, GREEN_IDX);
        self.screen.draw_figure(4, 0, &score_fig, GREEN_IDX);

        // Draw horizontal line
        for x in 0..SCREEN_WIDTH {
            self.screen.set(x, 5, PINK_IDX);
        }
    }

    fn reduce_concrete(&mut self) -> u8 {
        let mut score = 0;
        for row in (6..SCREEN_HEIGHT).rev() {
            if self.concrete.row_is_full(row) {
                score += 1;
                self.concrete
                    .clear_range(row * SCREEN_WIDTH, (row + 1) * SCREEN_WIDTH);
            }
        }
        score
    }

    fn shift_concrete(&mut self) {
        let mut to_idx = SCREEN_HEIGHT - 1;
        let mut from_idx = SCREEN_HEIGHT - 1;

        while from_idx > 5 {
            if self.concrete.row_is_empty(from_idx) {
                if from_idx == 0 {
                    break;
                }
                from_idx -= 1;
                continue;
            }

            if from_idx != to_idx {
                // Copy row
                for x in 0..SCREEN_WIDTH {
                    let color = self.concrete.get(x, from_idx);
                    self.concrete.set(x, to_idx, color);
                }
            }

            if from_idx == 0 {
                break;
            }
            from_idx -= 1;
            if to_idx == 0 {
                break;
            }
            to_idx -= 1;
        }

        // Clear remaining rows
        for row in 6..=to_idx {
            for x in 0..SCREEN_WIDTH {
                self.concrete.set(x, row, BLACK_IDX);
            }
        }
    }

    async fn game_over(
        &mut self,
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        joystick: &mut Joystick<'_>,
    ) {
        let mut leds: [RGB8; 256] = [RGB8::default(); 256];
        let figure = TETRAMINO.wrapping_at(0); // Use first tetramino for game over

        loop {
            if joystick.was_pressed() {
                break;
            }

            for color_idx in 1..COLORS.len() as u8 {
                self.screen.clear();
                self.screen
                    .draw_figure(self.init_x, self.init_y, &figure, color_idx);
                self.screen.render(&mut leds);
                ws2812.write(&leds).await;
                Timer::after_millis(500).await;

                self.screen
                    .draw_figure(self.init_x, self.init_y, &figure, BLACK_IDX);
                self.screen.render(&mut leds);
                ws2812.write(&leds).await;
                Timer::after_millis(500).await;
            }
        }
    }
}

impl Game for TetrisGame {
    async fn run(
        &mut self,
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        joystick: &mut Joystick<'_>,
    ) {
        let mut x = self.init_x;
        let mut y = self.init_y;
        let mut ipass = 0;

        let mut curr = TETRAMINO.wrapping_at(self.prng.next_range(7));
        let mut next = TETRAMINO.wrapping_at(self.prng.next_range(7));
        let mut leds: [RGB8; 256] = [RGB8::default(); 256];

        loop {
            if ipass >= 10 {
                ipass = 0;
                y += 1;
            }

            // Read joystick
            let x_diff = joystick.read_x().await;
            let new_x = x + x_diff;

            if new_x >= 0 && new_x < SCREEN_WIDTH as i8 && !self.concrete.collides(new_x, y, &curr)
            {
                x = new_x;
            }

            if joystick.was_pressed() {
                let rotated = curr.rotate();
                let shift = if rotated.height() > rotated.width()
                    && x + rotated.width() as i8 >= SCREEN_WIDTH as i8
                {
                    (rotated.height() - rotated.width()) as i8
                } else {
                    0
                };

                if !self.concrete.collides(x - shift, y, &rotated) {
                    curr = rotated;
                    x -= shift;
                }
            }

            self.screen.copy_from(&self.concrete);
            self.draw_score();

            if y > self.next_visible_y {
                self.screen
                    .draw_figure(self.init_x, self.init_y, &next, YELLOW_IDX);
            }

            if !self.concrete.collides(x, y, &curr) {
                self.screen.draw_figure(x, y, &curr, RED_IDX);
            } else {
                self.screen.draw_figure(x, y - 1, &curr, RED_IDX);
                self.concrete.draw_figure(x, y - 1, &curr, BRICK_IDX);

                x = self.init_x;
                y = self.init_y + 1;

                if self.concrete.collides(x, y, &curr) {
                    self.game_over(ws2812, joystick).await;
                    return;
                }

                curr = next;
                next = TETRAMINO.wrapping_at(self.prng.next_range(7));

                let reduced = self.reduce_concrete();
                self.score += reduced;

                if reduced > 0 {
                    self.shift_concrete();
                }
            }

            self.screen.render(&mut leds);
            ws2812.write(&leds).await;

            if self.score > 99 {
                self.score = 0;
            }

            let speed_bonus = (self.score / 10).max(1);
            let y_input = joystick.read_y().await;
            let down_bonus = if y_input > 0 { 10 } else { 0 };

            ipass += speed_bonus + down_bonus;
            Timer::after_millis(50).await;
        }
    }
}

// Snake game implementation
struct SnakeGame {
    body: [Dot; 32], // Fixed size array for snake body
    body_len: usize,
    direction: Dot,
    apple: Dot,
    screen: FrameBuffer,
    prng: Prng,
}

impl SnakeGame {
    fn new() -> Self {
        let mut body = [Dot::new(0, 0); 32];
        body[0] = Dot::new(4, 15);
        body[1] = Dot::new(4, 16);
        body[2] = Dot::new(4, 17);

        Self {
            body,
            body_len: 3,
            direction: Dot::new(0, 1),
            apple: Dot::new(5, 5),
            screen: FrameBuffer::new(),
            prng: Prng::new(54321),
        }
    }

    fn respawn_apple(&mut self) {
        // Find empty spot for apple
        for _ in 0..100 {
            // Try up to 100 times
            let x = self.prng.next_range(SCREEN_WIDTH as u8) as i8;
            let y = self.prng.next_range(SCREEN_HEIGHT as u8) as i8;
            let pos = Dot::new(x, y);

            // Check if position is not occupied by snake
            let mut occupied = false;
            for i in 0..self.body_len {
                if self.body[i] == pos {
                    occupied = true;
                    break;
                }
            }

            if !occupied {
                self.apple = pos;
                break;
            }
        }
    }

    fn move_forward(&mut self) -> bool {
        let head = self.body[self.body_len - 1];
        let new_head = head.move_wrap(self.direction);

        // Check collision with body
        for i in 0..self.body_len {
            if self.body[i] == new_head {
                return false; // Collision with self
            }
        }

        // Check if eating apple
        let eating_apple = new_head == self.apple;

        if eating_apple {
            // Grow snake
            if self.body_len < self.body.len() {
                self.body[self.body_len] = new_head;
                self.body_len += 1;
            }
            self.respawn_apple();
        } else {
            // Move snake
            for i in 0..self.body_len - 1 {
                self.body[i] = self.body[i + 1];
            }
            self.body[self.body_len - 1] = new_head;
        }

        true
    }

    fn draw_snake(&mut self) {
        for i in 0..self.body_len {
            let color = if i == self.body_len - 1 {
                LIGHT_GREEN_IDX
            } else {
                GREEN_IDX
            };
            self.screen
                .set(self.body[i].x as usize, self.body[i].y as usize, color);
        }
    }

    async fn game_over(
        &mut self,
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        joystick: &mut Joystick<'_>,
    ) {
        let mut leds: [RGB8; 256] = [RGB8::default(); 256];
        let mut color_idx = 0u8;

        loop {
            if joystick.was_pressed() {
                break;
            }

            self.screen.clear();
            self.draw_snake();
            self.screen
                .set(self.apple.x as usize, self.apple.y as usize, color_idx);
            self.screen.render(&mut leds);
            ws2812.write(&leds).await;

            color_idx = (color_idx + 1) % COLORS.len() as u8;
            Timer::after_millis(200).await;
        }
    }
}

impl Game for SnakeGame {
    async fn run(
        &mut self,
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        joystick: &mut Joystick<'_>,
    ) {
        let mut step = 30;
        let mut leds: [RGB8; 256] = [RGB8::default(); 256];

        loop {
            let new_x = joystick.read_x().await;
            let new_y = joystick.read_y().await;

            if new_x != 0 && new_y != 0 {
                // Prioritize one direction
                let direction = Dot::new(new_x, 0);
                if !self.direction.is_opposite(&direction) {
                    self.direction = direction;
                }
            } else if new_x != 0 || new_y != 0 {
                let direction = Dot::new(new_x, new_y);
                if !self.direction.is_opposite(&direction) {
                    self.direction = direction;
                }
            }

            if step >= 30 {
                step = 0;

                self.screen.clear();
                self.draw_snake();
                self.screen
                    .set(self.apple.x as usize, self.apple.y as usize, RED_IDX);

                if !self.move_forward() {
                    self.game_over(ws2812, joystick).await;
                    return;
                }

                self.screen.render(&mut leds);
                ws2812.write(&leds).await;
            }

            step += 1;
            Timer::after_millis(20).await;
        }
    }
}

// Tanks game implementation
struct TanksGame {
    screen: FrameBuffer,
    score: u8,
    prng: Prng,
}

impl TanksGame {
    fn new() -> Self {
        Self {
            screen: FrameBuffer::new(),
            score: 0,
            prng: Prng::new(98765),
        }
    }

    fn draw_score(&mut self) {
        self.score %= 100;
        let speed = self.score / 10;
        let score_digit = self.score % 10;

        let speed_fig = DIGITS.wrapping_at(speed);
        let score_fig = DIGITS.wrapping_at(score_digit);

        self.screen.draw_figure(0, 0, &speed_fig, GREEN_IDX);
        self.screen.draw_figure(4, 0, &score_fig, GREEN_IDX);

        // Draw horizontal line
        for x in 0..SCREEN_WIDTH {
            self.screen.set(x, 5, PINK_IDX);
        }
    }

    async fn game_over(
        &mut self,
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        joystick: &mut Joystick<'_>,
    ) {
        let mut leds: [RGB8; 256] = [RGB8::default(); 256];

        loop {
            if joystick.was_pressed() {
                break;
            }

            // Random flashing effect
            for _ in 0..20 {
                let x = self.prng.next_range(SCREEN_WIDTH as u8) as usize;
                let y = (self.prng.next_range((SCREEN_HEIGHT - 6) as u8) + 6) as usize;
                let color = self.prng.next_range(COLORS.len() as u8);
                self.screen.set(x, y, color);
            }

            self.screen.render(&mut leds);
            ws2812.write(&leds).await;
            Timer::after_millis(50).await;
        }
    }
}

impl Game for TanksGame {
    async fn run(
        &mut self,
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        joystick: &mut Joystick<'_>,
    ) {
        let mut leds: [RGB8; 256] = [RGB8::default(); 256];

        loop {
            self.screen.clear();
            self.draw_score();

            let x_input = joystick.read_x().await;
            let y_input = joystick.read_y().await;

            // Simple tank game - just move a tank around
            let tank_x = 3 + x_input;
            let tank_y = 16 + y_input;

            if tank_x >= 0
                && tank_x < (SCREEN_WIDTH - 3) as i8
                && tank_y >= 6
                && tank_y < (SCREEN_HEIGHT - 3) as i8
            {
                // Draw tank as 3x3 block
                for dy in 0..3 {
                    for dx in 0..3 {
                        self.screen
                            .set((tank_x + dx) as usize, (tank_y + dy) as usize, GREEN_IDX);
                    }
                }
            }

            if joystick.was_pressed() {
                self.score += 1;
            }

            self.screen.render(&mut leds);
            ws2812.write(&leds).await;

            Timer::after_millis(50).await;
        }
    }
}

// Races game implementation
struct RacesGame {
    screen: FrameBuffer,
    score: u8,
    car_pos: Dot,
    obstacles: [Dot; 8], // Fixed size array for obstacles
    obstacle_count: usize,
    bullets: [Dot; 4], // Fixed size array for bullets
    bullet_count: usize,
    lives: u8,
    invulnerable_time: u8,
    prng: Prng,
}

impl RacesGame {
    fn new() -> Self {
        Self {
            screen: FrameBuffer::new(),
            score: 0,
            car_pos: Dot::new(3, 27),
            obstacles: [Dot::new(0, 0); 8],
            obstacle_count: 0,
            bullets: [Dot::new(0, 0); 4],
            bullet_count: 0,
            lives: 3,
            invulnerable_time: 0,
            prng: Prng::new(13579),
        }
    }

    fn draw_score(&mut self) {
        self.score %= 100;
        let speed = self.score / 10;
        let score_digit = self.score % 10;

        let speed_fig = DIGITS.wrapping_at(speed);
        let score_fig = DIGITS.wrapping_at(score_digit);

        self.screen.draw_figure(0, 0, &speed_fig, GREEN_IDX);
        self.screen.draw_figure(4, 0, &score_fig, GREEN_IDX);

        // Draw horizontal line
        for x in 0..SCREEN_WIDTH {
            self.screen.set(x, 5, PINK_IDX);
        }
    }

    fn draw_road(&mut self) {
        // Draw road edges
        for y in 6..SCREEN_HEIGHT {
            self.screen.set(0, y, PINK_IDX);
            self.screen.set(SCREEN_WIDTH - 1, y, PINK_IDX);
        }
    }

    fn draw_car(&mut self) {
        // Draw car as a simple 3x5 shape
        if self.invulnerable_time == 0 || self.invulnerable_time % 4 < 2 {
            for dy in 0..5 {
                for dx in 0..3 {
                    let x = self.car_pos.x + dx;
                    let y = self.car_pos.y + dy;
                    if x >= 0 && x < SCREEN_WIDTH as i8 && y >= 0 && y < SCREEN_HEIGHT as i8 {
                        self.screen.set(x as usize, y as usize, GREEN_IDX);
                    }
                }
            }
        }
    }

    fn draw_obstacles(&mut self) {
        for i in 0..self.obstacle_count {
            let obs = &self.obstacles[i];
            // Draw obstacle as 3x3 block
            for dy in 0..3 {
                for dx in 0..3 {
                    let x = obs.x + dx;
                    let y = obs.y + dy;
                    if x >= 0 && x < SCREEN_WIDTH as i8 && y >= 0 && y < SCREEN_HEIGHT as i8 {
                        self.screen.set(x as usize, y as usize, RED_IDX);
                    }
                }
            }
        }
    }

    fn draw_bullets(&mut self) {
        for i in 0..self.bullet_count {
            let bullet = &self.bullets[i];
            if bullet.x >= 0
                && bullet.x < SCREEN_WIDTH as i8
                && bullet.y >= 0
                && bullet.y < SCREEN_HEIGHT as i8
            {
                self.screen
                    .set(bullet.x as usize, bullet.y as usize, YELLOW_IDX);
            }
        }
    }

    fn spawn_obstacles(&mut self) {
        if self.obstacle_count < self.obstacles.len() && self.prng.next_range(30) == 0 {
            // Choose random lane (avoid road edges)
            let lane = 1 + self.prng.next_range(SCREEN_WIDTH as u8 - 4);

            // Make sure obstacle doesn't overlap with existing ones
            let mut can_spawn = true;
            for i in 0..self.obstacle_count {
                if self.obstacles[i].y < 10 && (self.obstacles[i].x - lane as i8).abs() < 4 {
                    can_spawn = false;
                    break;
                }
            }

            if can_spawn {
                self.obstacles[self.obstacle_count] = Dot::new(lane as i8, 6);
                self.obstacle_count += 1;
            }
        }
    }

    fn update_obstacles(&mut self) {
        // Move obstacles down
        let mut active_count = 0;
        for i in 0..self.obstacle_count {
            self.obstacles[i].y += 1;
            if self.obstacles[i].y < SCREEN_HEIGHT as i8 {
                if active_count != i {
                    self.obstacles[active_count] = self.obstacles[i];
                }
                active_count += 1;
            }
        }
        self.obstacle_count = active_count;
    }

    fn update_bullets(&mut self) {
        // Move bullets up and check collisions
        let mut active_bullets = 0;
        let mut active_obstacles = 0;

        for i in 0..self.bullet_count {
            self.bullets[i].y -= 2;
            if self.bullets[i].y >= 0 {
                // Check collision with obstacles
                let mut hit = false;
                for j in 0..self.obstacle_count {
                    if (self.bullets[i].x - self.obstacles[j].x).abs() <= 1
                        && (self.bullets[i].y - self.obstacles[j].y).abs() <= 1
                    {
                        hit = true;
                        self.score += 10;
                        // Mark obstacle for removal
                        self.obstacles[j].y = SCREEN_HEIGHT as i8; // Move off screen
                        break;
                    }
                }

                if !hit {
                    if active_bullets != i {
                        self.bullets[active_bullets] = self.bullets[i];
                    }
                    active_bullets += 1;
                }
            }
        }
        self.bullet_count = active_bullets;

        // Remove hit obstacles
        for i in 0..self.obstacle_count {
            if self.obstacles[i].y < SCREEN_HEIGHT as i8 {
                if active_obstacles != i {
                    self.obstacles[active_obstacles] = self.obstacles[i];
                }
                active_obstacles += 1;
            }
        }
        self.obstacle_count = active_obstacles;
    }

    fn check_collisions(&mut self) {
        if self.invulnerable_time > 0 {
            return;
        }

        let car_center = Dot::new(self.car_pos.x + 1, self.car_pos.y + 2);

        for i in 0..self.obstacle_count {
            let obs_center = Dot::new(self.obstacles[i].x + 1, self.obstacles[i].y + 1);

            if (car_center.x - obs_center.x).abs() <= 2 && (car_center.y - obs_center.y).abs() <= 3
            {
                self.lives -= 1;
                self.invulnerable_time = 60; // 3 seconds at 50ms per frame
                break;
            }
        }
    }

    async fn game_over(
        &mut self,
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        joystick: &mut Joystick<'_>,
    ) {
        let mut leds: [RGB8; 256] = [RGB8::default(); 256];

        loop {
            if joystick.was_pressed() {
                break;
            }

            // Flash the screen
            for _ in 0..10 {
                let x = self.prng.next_range(SCREEN_WIDTH as u8) as usize;
                let y = (self.prng.next_range((SCREEN_HEIGHT - 6) as u8) + 6) as usize;
                let color = self.prng.next_range(COLORS.len() as u8);
                self.screen.set(x, y, color);
            }

            self.screen.render(&mut leds);
            ws2812.write(&leds).await;
            Timer::after_millis(100).await;
        }
    }
}

impl Game for RacesGame {
    async fn run(
        &mut self,
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        joystick: &mut Joystick<'_>,
    ) {
        let mut step = 0;
        let mut leds: [RGB8; 256] = [RGB8::default(); 256];

        loop {
            self.screen.clear();
            self.draw_score();
            self.draw_road();

            let x_input = joystick.read_x().await;
            let y_input = joystick.read_y().await;

            // Move car
            if x_input != 0 && y_input != 0 {
                // Prioritize horizontal movement
                let new_pos = self.car_pos.move_by(Dot::new(x_input, 0));
                if new_pos.x >= 1 && new_pos.x < (SCREEN_WIDTH - 4) as i8 {
                    self.car_pos = new_pos;
                }
            } else {
                let new_pos = self.car_pos.move_by(Dot::new(x_input, y_input));
                if new_pos.x >= 1
                    && new_pos.x < (SCREEN_WIDTH - 4) as i8
                    && new_pos.y >= 6
                    && new_pos.y < (SCREEN_HEIGHT - 5) as i8
                {
                    self.car_pos = new_pos;
                }
            }

            if joystick.was_pressed() && self.bullet_count < self.bullets.len() {
                self.bullets[self.bullet_count] = Dot::new(self.car_pos.x + 1, self.car_pos.y - 1);
                self.bullet_count += 1;
            }

            // Update game state
            self.update_bullets();
            self.spawn_obstacles();

            if step % 5 == 0 {
                self.update_obstacles();
            }

            self.check_collisions();

            // Draw everything
            self.draw_obstacles();
            self.draw_bullets();
            self.draw_car();

            // Draw lives
            for i in 0..self.lives.min(3) {
                self.screen.set(3 + i as usize * 2, 1, LIGHT_BLUE_IDX);
            }

            self.screen.render(&mut leds);
            ws2812.write(&leds).await;

            // Check game over
            if self.lives == 0 {
                self.game_over(ws2812, joystick).await;
                return;
            }

            // Update counters
            if self.invulnerable_time > 0 {
                self.invulnerable_time -= 1;
            }

            step += 1;
            Timer::after_millis(50).await;
        }
    }
}

#[embassy_executor::task]
async fn button_task(mut button_controller: ButtonController) {
    button_controller.run().await;
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Starting game collection!");
    let p = embassy_rp::init(Default::default());

    // Initialize PIO for WS2812
    let Pio {
        mut common, sm0, ..
    } = Pio::new(p.PIO0, Irqs);

    let program = PioWs2812Program::new(&mut common);
    let mut ws2812 = PioWs2812::new(&mut common, sm0, p.DMA_CH0, p.PIN_13, &program);

    let mut joystick = Joystick::new(spawner, p.ADC, p.PIN_16, p.PIN_27, p.PIN_28);
    let mut leds: [RGB8; 256] = [RGB8::default(); 256];

    // Game menu
    let mut game_idx: u8 = 0;
    let num_games = 4; // Tetris, Snake, Tanks, Races

    info!("Starting main menu loop");
    let mut n = 0;
    loop {
        info!("Iteration {}", n);
        n += 1;
        // Read joystick for menu navigation
        let x_input = joystick.read_x().await;

        if x_input != 0 {
            game_idx = game_idx.wrapping_add(x_input as u8) % num_games;
            Timer::after_millis(200).await; // Debounce
        }

        if joystick.was_pressed() {
            match game_idx {
                0 => {
                    info!("Starting Tetris");
                    let mut tetris = TetrisGame::new();
                    tetris.run(&mut ws2812, &mut joystick).await;
                }
                1 => {
                    info!("Starting Snake");
                    let mut snake = SnakeGame::new();
                    snake.run(&mut ws2812, &mut joystick).await;
                }
                2 => {
                    info!("Starting Tanks");
                    let mut tanks = TanksGame::new();
                    tanks.run(&mut ws2812, &mut joystick).await;
                }
                3 => {
                    info!("Starting Races");
                    let mut races = RacesGame::new();
                    races.run(&mut ws2812, &mut joystick).await;
                }
                _ => {}
            }
        }

        // Display menu - show game index
        leds.fill(BLACK);
        let digit = DIGITS.wrapping_at(game_idx as u8);
        let mut screen = FrameBuffer::new();
        screen.draw_figure(2, 14, &digit, GREEN_IDX);
        screen.render(&mut leds);
        ws2812.write(&leds).await;

        // Phantom press ???
        _ = joystick.was_pressed();

        Timer::after_millis(100).await;
    }
}
