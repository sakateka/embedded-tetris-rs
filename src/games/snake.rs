use embassy_rp::pio_programs::ws2812::PioWs2812;
use embassy_time::{Duration, Ticker, Timer};
use smart_leds::RGB8;

use crate::{
    common::{GREEN_IDX, RED_IDX, SCREEN_HEIGHT, SCREEN_WIDTH},
    digits::DIGITS,
    Dot, FrameBuffer, Game, Joystick, Prng,
};

pub struct SnakeGame {
    body: [Dot; 256],
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
    pub fn new() -> Self {
        let prng = Prng::new();
        let mut game = Self {
            body: [Dot::new(0, 0); 256],
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
        let mut digits = [0u8; 4];
        let mut count = 0;

        while score > 0 && count < digits.len() {
            digits[count] = score % 10;
            score /= 10;
            count += 1;
        }

        for i in (0..count).rev() {
            self.screen.draw_figure(
                ((count - 1 - i) * 4).try_into().unwrap(),
                0,
                &DIGITS[digits[i] as usize],
                GREEN_IDX,
            );
        }
    }

    async fn game_over(
        &mut self,
        mut leds: [RGB8; 256],
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        joystick: &mut Joystick<'_>,
    ) {
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
            let new_dir = Dot::new(x, y).to_direction();

            if !new_dir.is_zero() {
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
                self.game_over(leds, ws2812, joystick).await;
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
