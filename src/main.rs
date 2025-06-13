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
mod digits;
mod figure;

use control::{button_was_pressed, ButtonController};
use digits::DIGITS;
use figure::{Figure, Tetramino, TETRAMINO};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
    ADC_IRQ_FIFO => AdcInterruptHandler;
});

//  Coordinates
//        x
//     0 --->  7
//    0+-------+
//     |       |
//     |   S   |
//   | |   C   |
// y | |   R   |---+
//   | |   E   | +----+
//   v |   E   | |::::| <- microbit
//     |   N   | +----+
//     |       | @ |<---- joystick
//   31+-------+---+

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

// Function to create FrameBuffer from title rows (like Python's from_rows)
fn framebuffer_from_rows(rows: &[u32; 8], color: u8) -> FrameBuffer {
    let mut buffer = FrameBuffer::new();

    for y in 0..SCREEN_HEIGHT {
        for x in 0..rows.len() {
            if x < SCREEN_WIDTH {
                let bit = rows[x] >> (SCREEN_HEIGHT - y - 1) & 1;
                if bit == 1 {
                    buffer.set(SCREEN_WIDTH - x - 1, y, color);
                }
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

    fn get_tetramino_color(&self, tetramino_idx: u8) -> u8 {
        match tetramino_idx {
            0 => LIGHT_BLUE_IDX, // I piece
            1 => YELLOW_IDX,     // O piece
            2 => PINK_IDX,       // T piece
            3 => GREEN_IDX,      // S piece
            4 => RED_IDX,        // Z piece
            5 => BLUE_IDX,       // J piece
            6 => BRICK_IDX,      // L piece
            _ => RED_IDX,        // Default
        }
    }

    fn draw_score(&mut self) {
        self.score %= 100;
        let speed = self.score / 10;
        let score_digit = self.score % 10;

        let speed_fig = DIGITS.wrapping_at(speed);
        let score_fig = DIGITS.wrapping_at(score_digit);

        self.screen.draw_figure(0, 0, &speed_fig, GREEN_IDX);
        self.screen.draw_figure(5, 0, &score_fig, GREEN_IDX);

        // Draw horizontal line
        for x in 0..SCREEN_WIDTH {
            self.screen.set(x, 5, PINK_IDX);
        }
    }

    fn reduce_concrete(&mut self) -> ([usize; 4], usize) {
        let mut cleared_rows = [0; 4];
        let mut count = 0;

        for row in (6..SCREEN_HEIGHT).rev() {
            if self.concrete.row_is_full(row) {
                cleared_rows[count] = row;
                count += 1;
                self.concrete
                    .clear_range(row * SCREEN_WIDTH, (row + 1) * SCREEN_WIDTH);
            }
        }
        (cleared_rows, count)
    }

    async fn animate_concrete_shift(
        &mut self,
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        cleared_rows: &[usize],
        count: usize,
    ) {
        let mut leds: [RGB8; 256] = [RGB8::default(); 256];

        // For each cleared row, animate the blocks above it falling down
        for i in (0..count).rev() {
            let cleared_row = cleared_rows[i];
            let mut to_idx = cleared_row;
            let mut from_idx = cleared_row - 1;

            while from_idx > 5 {
                if self.concrete.row_is_empty(from_idx) {
                    if from_idx == 0 {
                        break;
                    }
                    from_idx -= 1;
                    continue;
                }

                // Copy row
                for x in 0..SCREEN_WIDTH {
                    let color = self.concrete.get(x, from_idx);
                    self.concrete.set(x, to_idx, color);
                }

                // Render and wait for animation
                self.screen.copy_from(&self.concrete);
                self.draw_score();
                self.screen.render(&mut leds);
                ws2812.write(&leds).await;
                Timer::after_millis(50).await;

                if from_idx == 0 {
                    break;
                }
                from_idx -= 1;
                if to_idx == 0 {
                    break;
                }
                to_idx -= 1;
            }

            // Clear the row that was just shifted
            for x in 0..SCREEN_WIDTH {
                self.concrete.set(x, to_idx, BLACK_IDX);
            }
        }

        // Final render
        self.screen.copy_from(&self.concrete);
        self.draw_score();
        self.screen.render(&mut leds);
        ws2812.write(&leds).await;
    }

    async fn game_over(
        &mut self,
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        joystick: &mut Joystick<'_>,
        last_x: i8,
        last_y: i8,
        last_figure: &Figure,
        last_color: u8,
    ) {
        let mut leds: [RGB8; 256] = [RGB8::default(); 256];

        loop {
            if joystick.was_pressed() {
                break;
            }

            // Preserve the concrete blocks and score
            self.screen.copy_from(&self.concrete);
            self.draw_score();

            // Blink the last tetramino
            self.screen
                .draw_figure(last_x, last_y - 1, last_figure, last_color);
            self.screen.render(&mut leds);
            ws2812.write(&leds).await;
            Timer::after_millis(500).await;

            // Clear only the last tetramino
            self.screen
                .draw_figure(last_x, last_y - 1, last_figure, BLACK_IDX);
            self.screen.render(&mut leds);
            ws2812.write(&leds).await;
            Timer::after_millis(500).await;
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

        let mut curr_idx = self.prng.next_range(7);
        let mut next_idx = self.prng.next_range(7);
        let mut curr = TETRAMINO.wrapping_at(curr_idx);
        let mut next = TETRAMINO.wrapping_at(next_idx);
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

            let curr_color = self.get_tetramino_color(curr_idx);
            let next_color = self.get_tetramino_color(next_idx);

            if y > self.next_visible_y {
                self.screen
                    .draw_figure(self.init_x, self.init_y, &next, next_color);
            }

            if !self.concrete.collides(x, y, &curr) {
                self.screen.draw_figure(x, y, &curr, curr_color);
            } else {
                self.screen.draw_figure(x, y - 1, &curr, curr_color);
                self.concrete.draw_figure(x, y - 1, &curr, curr_color);

                x = self.init_x;
                y = self.init_y + 1;

                if self.concrete.collides(x, y, &curr) {
                    self.game_over(ws2812, joystick, x, y, &curr, curr_color)
                        .await;
                    return;
                }

                curr_idx = next_idx;
                next_idx = self.prng.next_range(7);
                curr = TETRAMINO.wrapping_at(curr_idx);
                next = TETRAMINO.wrapping_at(next_idx);

                let (cleared_rows, count) = self.reduce_concrete();
                self.score += count as u8;

                if count > 0 {
                    // Animate each cleared row separately
                    self.animate_concrete_shift(ws2812, &cleared_rows, count)
                        .await;
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
    score: u8,
    base_speed: Duration,
    current_speed: Duration,
}

impl SnakeGame {
    fn new() -> Self {
        let mut prng = Prng::new(0x12345678);
        let mut game = Self {
            body: [Dot::new(0, 0); 32],
            body_len: 3,
            direction: Dot::new(1, 0),
            apple: Dot::new(0, 0),
            screen: FrameBuffer::new(),
            prng,
            score: 0,
            base_speed: Duration::from_millis(200),
            current_speed: Duration::from_millis(200),
        };

        // Initialize snake body
        game.body[0] = Dot::new(3, 15);
        game.body[1] = Dot::new(2, 15);
        game.body[2] = Dot::new(1, 15);

        game.respawn_apple();
        game
    }

    fn respawn_apple(&mut self) {
        loop {
            let x = self.prng.next_range(SCREEN_WIDTH as u8) as i8;
            let y = self.prng.next_range(SCREEN_HEIGHT as u8) as i8;
            let new_apple = Dot::new(x, y);

            // Check if apple spawns on snake body
            let mut valid = true;
            for i in 0..self.body_len {
                if self.body[i] == new_apple {
                    valid = false;
                    break;
                }
            }

            if valid {
                self.apple = new_apple;
                break;
            }
        }
    }

    fn move_forward(&mut self) -> bool {
        let head = self.body[0];
        let new_head = head.move_wrap(self.direction);

        // Check collision with self
        for i in 0..self.body_len {
            if self.body[i] == new_head {
                return false;
            }
        }

        // Move body
        for i in (1..self.body_len).rev() {
            self.body[i] = self.body[i - 1];
        }
        self.body[0] = new_head;

        // Check if apple is eaten
        if new_head == self.apple {
            if self.body_len < 32 {
                self.body_len += 1;
                self.body[self.body_len - 1] = self.body[self.body_len - 2];
            }
            self.score += 1;
            self.respawn_apple();
        }

        true
    }

    fn draw_snake(&mut self) {
        // Clear screen
        self.screen.clear();

        // Draw snake body
        for i in 0..self.body_len {
            let dot = self.body[i];
            self.screen.set(dot.x as usize, dot.y as usize, GREEN_IDX);
        }

        // Draw apple
        self.screen
            .set(self.apple.x as usize, self.apple.y as usize, RED_IDX);

        // Draw score
        self.draw_score();
    }

    fn draw_score(&mut self) {
        let mut score = self.score;
        let mut x = 0;

        // Handle single digit
        if score < 10 {
            let figure = DIGITS.wrapping_at(score);
            self.screen.draw_figure(x, 0, &figure, YELLOW_IDX);
            return;
        }

        // Handle two digits
        let tens = score / 10;
        let ones = score % 10;

        let tens_figure = DIGITS.wrapping_at(tens);
        self.screen.draw_figure(x, 0, &tens_figure, YELLOW_IDX);
        x += tens_figure.width() as i8;

        let ones_figure = DIGITS.wrapping_at(ones);
        self.screen.draw_figure(x, 0, &ones_figure, YELLOW_IDX);
    }

    async fn game_over(
        &mut self,
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        joystick: &mut Joystick<'_>,
    ) {
        let mut leds = [RGB8::new(0, 0, 0); 256];

        // Flash screen
        for _ in 0..3 {
            self.screen.clear();
            self.screen.render(&mut leds);
            ws2812.write(&leds).await;
            Timer::after(Duration::from_millis(200)).await;

            self.draw_snake();
            self.screen.render(&mut leds);
            ws2812.write(&leds).await;
            Timer::after(Duration::from_millis(200)).await;
        }

        // Wait for button press
        while !joystick.was_pressed() {
            Timer::after(Duration::from_millis(50)).await;
        }
    }
}

impl Game for SnakeGame {
    async fn run(
        &mut self,
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        joystick: &mut Joystick<'_>,
    ) {
        let mut leds = [RGB8::new(0, 0, 0); 256];
        let mut ticker = Ticker::every(self.current_speed);

        loop {
            // Handle joystick input
            let x = joystick.read_x().await;
            let y = joystick.read_y().await;

            if x != 0 || y != 0 {
                // Prioritize horizontal movement over vertical
                let new_dir = if x != 0 {
                    Dot::new(x, 0)
                } else {
                    Dot::new(0, y)
                };

                if !new_dir.is_opposite(&self.direction) {
                    // If pressing same direction as current movement, speed up
                    if new_dir.x == self.direction.x && new_dir.y == self.direction.y {
                        self.current_speed = Duration::from_millis(100); // Double speed
                        ticker = Ticker::every(self.current_speed);
                    }
                    self.direction = new_dir;
                }
            } else {
                // Reset to normal speed when no direction is pressed
                if self.current_speed != self.base_speed {
                    self.current_speed = self.base_speed;
                    ticker = Ticker::every(self.current_speed);
                }
            }

            // Move snake
            if !self.move_forward() {
                self.game_over(ws2812, joystick).await;
                break;
            }

            // Draw and update display
            self.draw_snake();
            self.screen.render(&mut leds);
            ws2812.write(&leds).await;

            ticker.next().await;
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
        self.screen.draw_figure(5, 0, &score_fig, GREEN_IDX);

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
    cars_destroyed: u8,
    car_pos: Dot,
    obstacles: [Dot; 2],
    obstacle_count: usize,
    bullets: [Dot; 4],
    bullet_count: usize,
    max_bullets: u8,
    lives: u8,
    invulnerable_time: u8,
    racing_cars: [Dot; 1],
    racing_speeds: [i8; 1],
    racing_car_health: u8,
    road_animation: u8,
    bullet_powerup: Option<Dot>,
    prng: Prng,
}

impl RacesGame {
    fn new() -> Self {
        let prng = Prng::new(0x12345678);
        let mut game = Self {
            screen: FrameBuffer::new(),
            score: 0,
            cars_destroyed: 0,
            car_pos: Dot::new(3, 28),
            obstacles: [Dot::new(0, 0); 2],
            obstacle_count: 0,
            bullets: [Dot::new(0, 0); 4],
            bullet_count: 0,
            max_bullets: 5,
            lives: 3,
            invulnerable_time: 0,
            racing_cars: [Dot::new(0, 0); 1],
            racing_speeds: [1],
            racing_car_health: 3,
            road_animation: 0,
            bullet_powerup: None,
            prng,
        };

        // Initialize racing car at the top
        game.racing_cars[0] = Dot::new(3, 10);

        game
    }

    fn spawn_obstacles(&mut self) {
        if self.obstacle_count < self.obstacles.len() && self.prng.next_range(30) == 0 {
            // Reduced spawn rate
            let x = self.prng.next_range(6) as i8;
            self.obstacles[self.obstacle_count] = Dot::new(x, 0);
            self.obstacle_count += 1;
        }
    }

    fn spawn_bullet_powerup(&mut self) {
        if self.bullet_powerup.is_none() && self.prng.next_range(50) == 0 {
            let x = self.prng.next_range(5) as i8 + 1;
            self.bullet_powerup = Some(Dot::new(x, 0));
        }
    }

    fn update_bullet_powerup(&mut self) {
        if let Some(mut powerup) = self.bullet_powerup.take() {
            powerup.y += 1;

            // Check collision with player car
            if (powerup.x == self.car_pos.x
                || powerup.x == self.car_pos.x - 1
                || powerup.x == self.car_pos.x + 1)
                && (powerup.y >= self.car_pos.y - 3 && powerup.y <= self.car_pos.y)
            {
                if self.max_bullets < 5 {
                    self.max_bullets += 1;
                }
            } else if powerup.y < SCREEN_HEIGHT as i8 {
                // Only keep powerup if not collected and still on screen
                self.bullet_powerup.replace(powerup);
            }
        }
    }

    fn draw_bullet_powerup(&mut self) {
        if let Some(powerup) = self.bullet_powerup {
            if powerup.y >= 0 && powerup.y < SCREEN_HEIGHT as i8 {
                // Draw two vertical dots in pink
                self.screen
                    .set(powerup.x as usize, powerup.y as usize, PINK_IDX);
                self.screen
                    .set(powerup.x as usize, (powerup.y + 1) as usize, PINK_IDX);
            }
        }
    }

    fn update_racing_cars(&mut self) {
        for i in 0..self.racing_cars.len() {
            // Only move every 4 frames to make it very slow
            if self.prng.next_range(4) == 0 {
                let new_y = self.racing_cars[i].y + self.racing_speeds[i];

                // Check if new position would overlap with any obstacle
                let mut can_move = true;
                for j in 0..self.obstacle_count {
                    let obs = self.obstacles[j];
                    if (obs.x == self.racing_cars[i].x || obs.x == self.racing_cars[i].x + 1)
                        && (obs.y == new_y || obs.y == new_y + 1)
                    {
                        can_move = false;
                        break;
                    }
                }

                // Only move if no obstacle collision
                if can_move {
                    self.racing_cars[i].y = new_y;
                }
            }

            // If car goes off screen at bottom, reset it to top with random x position
            if self.racing_cars[i].y >= SCREEN_HEIGHT as i8 {
                self.racing_cars[i].y = 0;
                // Try to find a position that doesn't overlap with obstacles
                loop {
                    let new_x = self.prng.next_range(5) as i8 + 1;
                    let mut valid_position = true;

                    for j in 0..self.obstacle_count {
                        let obs = self.obstacles[j];
                        if (obs.x == new_x || obs.x == new_x + 1) && (obs.y == 0 || obs.y == 1) {
                            valid_position = false;
                            break;
                        }
                    }

                    if valid_position {
                        self.racing_cars[i].x = new_x;
                        break;
                    }
                }
            }
        }
    }

    fn draw_racing_cars(&mut self) {
        for i in 0..self.racing_cars.len() {
            let car = self.racing_cars[i];
            if car.y >= 0 && car.y < SCREEN_HEIGHT as i8 && self.racing_car_health > 0 {
                // Draw racing car (same shape as player car but in blue)
                let x = car.x as usize;
                let y = car.y as usize;

                // Draw car body
                self.screen.set(x - 1, y, BLUE_IDX);
                self.screen.set(x, y, BLUE_IDX);
                self.screen.set(x + 1, y, BLUE_IDX);
                self.screen.set(x, y - 1, BLUE_IDX);
                self.screen.set(x, y - 2, BLUE_IDX);
                self.screen.set(x - 1, y - 2, BLUE_IDX);
                self.screen.set(x + 1, y - 2, BLUE_IDX);
                self.screen.set(x, y - 3, BLUE_IDX);
            }
        }
    }

    fn update_obstacles(&mut self) {
        let mut i = 0;
        while i < self.obstacle_count {
            self.obstacles[i].y += 1;

            // Remove obstacles that are off screen
            if self.obstacles[i].y >= SCREEN_HEIGHT as i8 {
                // Remove by swapping with last obstacle
                self.obstacle_count -= 1;
                if i < self.obstacle_count {
                    self.obstacles[i] = self.obstacles[self.obstacle_count];
                }
            } else {
                i += 1;
            }
        }
    }

    fn update_bullets(&mut self) {
        let mut i = 0;
        while i < self.bullet_count {
            self.bullets[i].y -= 1;

            // Remove bullets that are off screen
            if self.bullets[i].y < 0 {
                // Remove by swapping with last bullet
                self.bullet_count -= 1;
                if i < self.bullet_count {
                    self.bullets[i] = self.bullets[self.bullet_count];
                }
            } else {
                i += 1;
            }
        }
    }

    fn check_car_obstacle_collision(&self, obs: &Dot) -> bool {
        // Check collision with the entire car shape
        // Bottom row (3 pixels wide)
        if (obs.x == self.car_pos.x - 1 || obs.x == self.car_pos.x || obs.x == self.car_pos.x + 1)
            && obs.y == self.car_pos.y
        {
            return true;
        }
        // Middle section (1 pixel wide, 2 pixels tall)
        if obs.x == self.car_pos.x && (obs.y == self.car_pos.y - 1 || obs.y == self.car_pos.y - 2) {
            return true;
        }
        // Top row (3 pixels wide)
        if (obs.x == self.car_pos.x - 1 || obs.x == self.car_pos.x + 1)
            && obs.y == self.car_pos.y - 2
        {
            return true;
        }
        // Top pixel
        if obs.x == self.car_pos.x && obs.y == self.car_pos.y - 3 {
            return true;
        }
        false
    }

    fn check_bullet_obstacle_collision(&self, bullet: &Dot, obs: &Dot) -> bool {
        (bullet.x == obs.x || bullet.x == obs.x + 1) && (bullet.y == obs.y || bullet.y == obs.y + 1)
    }

    fn check_bullet_racing_car_collision(&self, bullet: &Dot, racing_car: &Dot) -> bool {
        ((bullet.x == racing_car.x - 1 || bullet.x == racing_car.x || bullet.x == racing_car.x + 1)
            && (bullet.y == racing_car.y))
            || ((bullet.x == racing_car.x)
                && (bullet.y == racing_car.y - 1 || bullet.y == racing_car.y - 2))
            || ((bullet.x == racing_car.x - 1 || bullet.x == racing_car.x + 1)
                && (bullet.y == racing_car.y - 2))
            || ((bullet.x == racing_car.x) && (bullet.y == racing_car.y - 3))
    }

    fn check_collisions(&mut self) {
        if self.invulnerable_time > 0 {
            self.invulnerable_time -= 1;
            return;
        }

        // Check car-obstacle collisions
        for i in 0..self.obstacle_count {
            let obs = self.obstacles[i];
            if self.check_car_obstacle_collision(&obs) {
                self.lives -= 1;
                self.invulnerable_time = 20; // 1 second of invulnerability
                return;
            }
        }

        // Check bullet collisions
        let mut i = 0;
        while i < self.bullet_count {
            let bullet = self.bullets[i];
            let mut hit = false;

            // Check bullet-obstacle collisions
            let mut j = 0;
            while j < self.obstacle_count {
                let obs = self.obstacles[j];
                if self.check_bullet_obstacle_collision(&bullet, &obs) {
                    // Remove obstacle
                    self.obstacle_count -= 1;
                    if j < self.obstacle_count {
                        self.obstacles[j] = self.obstacles[self.obstacle_count];
                    }
                    hit = true;
                    break;
                }
                j += 1;
            }

            // Check bullet-racing car collisions
            if !hit && self.racing_car_health > 0 {
                let racing_car = self.racing_cars[0];
                if self.check_bullet_racing_car_collision(&bullet, &racing_car) {
                    self.racing_car_health -= 1;
                    hit = true;

                    // If racing car is destroyed, increment counter and respawn it
                    if self.racing_car_health == 0 {
                        self.cars_destroyed += 1;
                        self.racing_cars[0].y = 0;
                        self.racing_cars[0].x = self.prng.next_range(5) as i8 + 1;
                        self.racing_car_health = 3;
                    }
                }
            }

            if hit {
                // Remove bullet
                self.bullet_count -= 1;
                if i < self.bullet_count {
                    self.bullets[i] = self.bullets[self.bullet_count];
                }
            } else {
                i += 1;
            }
        }
    }

    fn draw_road(&mut self) {
        // Draw intermittent road edges with animation
        for y in 0..SCREEN_HEIGHT {
            // Left edge
            if (y + self.road_animation as usize) % 4 < 2 {
                self.screen.set(0, y, BRICK_IDX);
            }
            // Right edge
            if (y + self.road_animation as usize) % 4 < 2 {
                self.screen.set(7, y, BRICK_IDX);
            }
        }
    }

    fn draw_car(&mut self) {
        let x = self.car_pos.x as usize;
        let y = self.car_pos.y as usize;

        if self.invulnerable_time > 0 && (self.invulnerable_time / 4) % 2 == 0 {
            // Blink car when invulnerable
            return;
        }

        self.screen.set(x - 1, y, GREEN_IDX);
        self.screen.set(x, y, GREEN_IDX);
        self.screen.set(x + 1, y, GREEN_IDX);
        self.screen.set(x, y - 1, GREEN_IDX);
        self.screen.set(x, y - 2, GREEN_IDX);
        self.screen.set(x - 1, y - 2, GREEN_IDX);
        self.screen.set(x + 1, y - 2, GREEN_IDX);
        self.screen.set(x, y - 3, GREEN_IDX);
    }

    fn draw_obstacles(&mut self) {
        for i in 0..self.obstacle_count {
            let obs = self.obstacles[i];
            if obs.y >= 0 && obs.y < SCREEN_HEIGHT as i8 {
                // Draw bigger obstacle (2x2) in dark green
                self.screen
                    .set(obs.x as usize, obs.y as usize, DARK_GREEN_IDX);
                self.screen
                    .set(obs.x as usize + 1, obs.y as usize, DARK_GREEN_IDX);
                self.screen
                    .set(obs.x as usize, obs.y as usize + 1, DARK_GREEN_IDX);
                self.screen
                    .set(obs.x as usize + 1, obs.y as usize + 1, DARK_GREEN_IDX);
            }
        }
    }

    fn draw_bullets(&mut self) {
        for i in 0..self.bullet_count {
            let bullet = self.bullets[i];
            if bullet.y >= 0 && bullet.y < SCREEN_HEIGHT as i8 {
                self.screen
                    .set(bullet.x as usize, bullet.y as usize, RED_IDX);
            }
        }
    }

    fn draw_score(&mut self) {
        let score = self.cars_destroyed;

        // Draw left digit (tens)
        let tens = score / 10;
        let tens_figure = DIGITS.wrapping_at(tens);
        // Add extra space for digit one
        let tens_x = if tens == 1 { 1 } else { 0 };
        self.screen.draw_figure(tens_x, 0, &tens_figure, YELLOW_IDX);

        // Draw right digit (ones)
        let ones = score % 10;
        let ones_figure = DIGITS.wrapping_at(ones);
        // Add extra space for digit one
        let ones_x = if ones == 1 { 6 } else { 5 };
        self.screen.draw_figure(ones_x, 0, &ones_figure, YELLOW_IDX);

        // Draw vertical line of lives in the middle
        for y in 0..self.lives {
            self.screen.set(3, y as usize, GREEN_IDX);
        }

        // Draw bullet count to the right of lives in pink
        for y in 0..self.max_bullets {
            self.screen.set(4, y as usize, PINK_IDX);
        }
    }

    async fn game_over(
        &mut self,
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        joystick: &mut Joystick<'_>,
    ) {
        let mut leds = [RGB8::new(0, 0, 0); 256];

        // Flash screen
        for _ in 0..3 {
            self.screen.clear();
            self.screen.render(&mut leds);
            ws2812.write(&leds).await;
            Timer::after(Duration::from_millis(200)).await;

            self.draw_score();
            self.screen.render(&mut leds);
            ws2812.write(&leds).await;
            Timer::after(Duration::from_millis(200)).await;
        }

        // Wait for button press
        while !joystick.was_pressed() {
            Timer::after(Duration::from_millis(50)).await;
        }
    }
}

impl Game for RacesGame {
    async fn run(
        &mut self,
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        joystick: &mut Joystick<'_>,
    ) {
        let mut leds = [RGB8::new(0, 0, 0); 256];
        let mut ticker = Ticker::every(Duration::from_millis(100));
        let mut obstacle_timer = 0;
        let mut road_timer = 0;

        loop {
            // Handle joystick input
            let x = joystick.read_x().await;
            let y = joystick.read_y().await;

            // Move car horizontally
            if x != 0 {
                let new_x = self.car_pos.x + x;
                if new_x >= 1 && new_x <= SCREEN_WIDTH as i8 - 2 {
                    self.car_pos.x = new_x;
                }
            }

            // Move car vertically
            if y != 0 {
                let new_y = self.car_pos.y + y;
                if new_y >= 3 && new_y <= SCREEN_HEIGHT as i8 - 1 {
                    self.car_pos.y = new_y;
                }
            }

            // Update road animation automatically
            road_timer = (road_timer + 1) % 2; // Update every 2 frames
            if road_timer == 0 {
                self.road_animation = (self.road_animation + 1) % 4;
            }

            // Fire bullet on button press
            if joystick.was_pressed()
                && self.bullet_count < self.bullets.len()
                && self.max_bullets > 0
            {
                self.bullets[self.bullet_count] = Dot::new(self.car_pos.x, self.car_pos.y - 4);
                self.bullet_count += 1;
                self.max_bullets -= 1; // Decrement available bullets when firing
            }

            // Update game state
            self.spawn_obstacles();
            self.spawn_bullet_powerup();
            self.update_bullet_powerup();

            // Update obstacles every 3 frames
            obstacle_timer = (obstacle_timer + 1) % 3;
            if obstacle_timer == 0 {
                self.update_obstacles();
            }

            // Update racing cars
            self.update_racing_cars();

            self.update_bullets();
            self.check_collisions();

            // Check game over
            if self.lives == 0 {
                self.game_over(ws2812, joystick).await;
                break;
            }

            // Draw everything
            self.screen.clear();
            self.draw_road();
            self.draw_obstacles();
            self.draw_bullet_powerup();
            self.draw_bullets();
            self.draw_racing_cars();
            self.draw_car();
            self.draw_score();

            // Update display
            self.screen.render(&mut leds);
            ws2812.write(&leds).await;

            ticker.next().await;
        }
    }
}

#[embassy_executor::task]
async fn button_task(mut button_controller: ButtonController) {
    button_controller.run().await;
}

// Game title graphics (converted from Python GAMES array)
const TETRIS_TITLE: [u32; 8] = [
    0b_00000000000000000000000000000000,
    0b_00000000000000000000000000000000,
    0b_01110011100111001110010010011100,
    0b_00100010000010001010010010010000,
    0b_00100011100010001110010110010000,
    0b_00100010000010001000011010010000,
    0b_00100011100010001000010010011100,
    0b_00000000000000000000000000000000,
];

const RACES_TITLE: [u32; 8] = [
    0b_00000000000000000000000000000000,
    0b_00000000000000000000000000000000,
    0b_00001110011100101001010010010000,
    0b_00001000010100101001100010010000,
    0b_00001000010100111001100010110000,
    0b_00001000010100101001010011010000,
    0b_00001000011100101001010010010000,
    0b_00000000000000000000000000000000,
];

const TANKS_TITLE: [u32; 8] = [
    0b_00000000000000000000000000000000,
    0b_00000000000000000000000000000000,
    0b_00001110001000101001010010010000,
    0b_00000100010100101001100010010000,
    0b_00000100011100111001100010110000,
    0b_00000100010100101001010011010000,
    0b_00000100010100101001010010010000,
    0b_00000000000000000000000000000000,
];

const SNAKE_TITLE: [u32; 8] = [
    0b_00000000000000000000000000000000,
    0b_00000000000000000010000000000000,
    0b_01100110110111001000101001001100,
    0b_00010101010100001001101010010010,
    0b_01100100010111001010101100011110,
    0b_00010100010100001100101010010010,
    0b_01100100010111001000101001010010,
    0b_00000000000000000000000000000000,
];

// Game titles array
const GAME_TITLES: [&[u32; 8]; 4] = [&TETRIS_TITLE, &SNAKE_TITLE, &TANKS_TITLE, &RACES_TITLE];

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

    info!("Starting main menu loop");
    loop {
        // Read joystick for menu navigation
        let x_input = joystick.read_x().await;

        if x_input != 0 {
            game_idx = game_idx.wrapping_add(x_input as u8) % GAME_TITLES.len() as u8;
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
        let title = GAME_TITLES[game_idx as usize % GAME_TITLES.len()];
        let screen = framebuffer_from_rows(title, GREEN_IDX);
        screen.render(&mut leds);
        ws2812.write(&leds).await;

        // Phantom press detected due to ws2812 write ???
        _ = joystick.was_pressed();

        Timer::after_millis(100).await;
    }
}
