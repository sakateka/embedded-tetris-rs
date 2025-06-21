use smart_leds::RGB8;

use crate::{
    common::{
        Dot, FrameBuffer, Game, GameController, LedDisplay, Prng, Timer, DARK_GREEN_IDX, GREEN_IDX,
        LIGHT_GREEN_IDX, PINK_IDX, RED_IDX, SCREEN_HEIGHT, SCREEN_WIDTH,
    },
    digits::DIGITS,
};

pub struct SnakeGame<'a, D, C, T> {
    screen: FrameBuffer,
    display: &'a mut D,
    controller: &'a mut C,
    timer: &'a T,

    body: [Dot; 256],
    body_len: usize,
    direction: Dot,
    next_direction: Dot,
    apple: Dot,
    prng: Prng,
    score: u8,
}

impl<'a, D: LedDisplay, C: GameController, T: Timer> SnakeGame<'a, D, C, T> {
    pub fn new(prng: Prng, display: &'a mut D, controller: &'a mut C, timer: &'a T) -> Self {
        let mut game = Self {
            screen: FrameBuffer::new(),
            display,
            controller,
            timer,
            body: [Dot::new(0, 0); 256],
            body_len: 3,
            direction: Dot::new(1, 0),
            next_direction: Dot::new(1, 0),
            apple: Dot::new(0, 0),
            prng,
            score: 0,
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
            let y = self
                .prng
                .next_range(SCREEN_HEIGHT as u8)
                .clamp(6, SCREEN_HEIGHT as u8) as i8;
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
        if !self.direction.is_opposite(&self.next_direction) {
            self.direction = self.next_direction;
        }
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
        for i in 0..self.body_len {
            let dot = self.body[i];
            let color = match i {
                0 => LIGHT_GREEN_IDX,
                i if i == self.body_len - 1 => DARK_GREEN_IDX,
                _ => GREEN_IDX,
            };
            self.screen.set(dot.x as usize, dot.y as usize, color);
        }
    }

    fn draw_score(&mut self) {
        let score_display = (self.score % 100) as usize;
        let tens = score_display / 10;
        let ones = score_display % 10;

        self.screen.draw_figure(0, 0, &DIGITS[tens], GREEN_IDX);
        self.screen.draw_figure(4, 0, &DIGITS[ones], GREEN_IDX);
        for x in 0..SCREEN_WIDTH {
            self.screen.set(x, 5, PINK_IDX);
        }
    }

    async fn game_over(&mut self, mut leds: [RGB8; 256]) {
        for _ in 0..3 {
            self.screen.clear();
            self.screen.render(&mut leds);
            self.display.write(&leds).await;
            self.timer.sleep_millis(200).await;

            self.draw_snake();
            self.draw_score();
            self.screen.render(&mut leds);
            self.display.write(&leds).await;
            self.timer.sleep_millis(200).await;
        }

        // Wait for button press
        while !self.controller.was_pressed() {
            self.timer.sleep_millis(50).await;
        }
    }
}

impl<'a, D: LedDisplay, C: GameController, T: Timer> Game for SnakeGame<'a, D, C, T> {
    async fn run(&mut self) {
        let mut leds = [RGB8::new(0, 0, 0); 256];
        let mut step = 30;
        let mut speedup;

        loop {
            // Handle joystick input
            let x = self.controller.read_x().await;
            let y = self.controller.read_y().await;
            let direction = Dot::new(x, y).to_direction();
            if !direction.is_zero() {
                self.next_direction = direction;
            }

            if self.direction == direction {
                speedup = 5;
            } else {
                // Reset to normal speed when no direction is pressed
                speedup = 1;
            }
            // Adjust the snake's speed based on the score.
            speedup += self.score / 10;
            if self.score > 99 {
                self.score = 0;
            }

            if step >= 30 {
                step = 0;
                // Move snake
                if !self.move_forward() {
                    self.game_over(leds).await;
                    break;
                }

                // Draw and update display
                self.screen.clear();
                self.draw_score();
                self.draw_snake();
                // Draw apple
                self.screen
                    .set(self.apple.x as usize, self.apple.y as usize, RED_IDX);
                self.screen.render(&mut leds);
                self.display.write(&leds).await;
            }
            step += speedup;
            self.timer.sleep_millis(20).await;
        }
    }
}
