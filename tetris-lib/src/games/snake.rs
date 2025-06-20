use smart_leds::RGB8;

use crate::{
    common::{
        Dot, FrameBuffer, Game, GameController, LedDisplay, Prng, Timer, GREEN_IDX, RED_IDX,
        SCREEN_HEIGHT, SCREEN_WIDTH,
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

    async fn game_over(&mut self, mut leds: [RGB8; 256]) {
        for _ in 0..3 {
            self.screen.clear();
            self.screen.render(&mut leds);
            self.display.write(&leds).await;
            self.timer.sleep_millis(200).await;

            self.draw_snake();
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
        let mut speedup = 1;

        loop {
            // Handle joystick input
            let x = self.controller.read_x().await;
            let y = self.controller.read_y().await;
            let new_dir = Dot::new(x, y).to_direction();

            if !new_dir.is_zero() {
                if !new_dir.is_opposite(&self.direction) {
                    // If pressing same direction as current movement, speed up
                    if new_dir.x == self.direction.x && new_dir.y == self.direction.y {
                        speedup = 5;
                    }
                    self.direction = new_dir;
                }
            } else {
                // Reset to normal speed when no direction is pressed
                speedup = 1;
            }

            if step >= 30 {
                step = 0;
                // Move snake
                if !self.move_forward() {
                    self.game_over(leds).await;
                    break;
                }

                // Draw and update display
                self.draw_snake();
                self.screen.render(&mut leds);
                self.display.write(&leds).await;
            }
            step += speedup;
            self.timer.sleep_millis(20).await;
        }
    }
}
