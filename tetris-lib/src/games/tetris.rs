use smart_leds::RGB8;

use crate::common::{
    Dot, FrameBuffer, Game, GameController, LedDisplay, Prng, Timer, BLACK_IDX, BLUE_IDX,
    BRICK_IDX, GREEN_IDX, LIGHT_BLUE_IDX, PINK_IDX, RED_IDX, SCREEN_HEIGHT, SCREEN_WIDTH,
    YELLOW_IDX,
};
use crate::figure::{Figure, TETRAMINO};

use crate::digits::DIGITS;

pub struct TetrisGame<'a, D, C, T> {
    screen: FrameBuffer,
    concrete: FrameBuffer,
    display: &'a mut D,
    controller: &'a mut C,
    timer: &'a T,
    score: u8,
    prng: Prng,
}

impl<'a, D: LedDisplay, C: GameController, T: Timer> TetrisGame<'a, D, C, T> {
    pub fn new(prng: Prng, display: &'a mut D, controller: &'a mut C, timer: &'a T) -> Self {
        Self {
            screen: FrameBuffer::new(),
            concrete: FrameBuffer::new(),
            display,
            controller,
            timer,
            score: 0,
            prng,
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

        self.screen.draw_figure(0, 0, speed_fig, GREEN_IDX);
        self.screen.draw_figure(5, 0, score_fig, GREEN_IDX);

        // Draw horizontal line
        for x in 0..SCREEN_WIDTH {
            self.screen.set(x, 5, PINK_IDX);
        }
    }

    fn reduce_concrete(&mut self) -> Option<usize> {
        for row in (6..SCREEN_HEIGHT).rev() {
            if self.concrete.row_is_full(row) {
                self.concrete
                    .clear_range(row * SCREEN_WIDTH, (row + 1) * SCREEN_WIDTH);
                return Some(row);
            }
        }
        None
    }

    fn shift_concrete(&mut self, cleared_row: usize) {
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

    async fn game_over(
        &mut self,
        mut leds: [RGB8; 256],
        last_pos: Dot,
        last_figure: &Figure,
        last_color: u8,
    ) {
        while !self.controller.was_pressed() {
            // Preserve the concrete blocks and score
            self.screen.copy_from(&self.concrete);
            self.draw_score();

            // Blink the last tetramino
            self.screen
                .draw_figure(last_pos.x, last_pos.y - 1, last_figure, last_color);
            self.screen.render(&mut leds);
            self.display.write(&leds).await;
            self.timer.sleep_millis(500).await;

            // Clear only the last tetramino
            self.screen
                .draw_figure(last_pos.x, last_pos.y - 1, last_figure, BLACK_IDX);
            self.screen.render(&mut leds);
            self.display.write(&leds).await;
            self.timer.sleep_millis(500).await;
        }
    }
}

impl<'a, D: LedDisplay, C: GameController, T: Timer> Game for TetrisGame<'a, D, C, T> {
    async fn run(&mut self) {
        const INIT_X: i8 = 3;
        const INIT_Y: i8 = 6;
        const RESPAWN_THRESHOLD: i8 = 11;

        let mut x = INIT_X;
        let mut y = INIT_Y;
        let mut ipass: i8 = 0;
        let mut mpass: u8 = 0;

        let mut curr_idx = self.prng.next_range(7);
        let mut next_idx = self.prng.next_range(7);
        let mut curr = TETRAMINO.wrapping_at(curr_idx);
        let mut next = TETRAMINO.wrapping_at(next_idx);
        let mut leds: [RGB8; 256] = [RGB8::default(); 256];

        loop {
            if ipass > 10 {
                ipass = 0;
                y += 1;
            }

            // Read joystick
            let x_diff = self.controller.read_x().await;
            let mut new_x = x + x_diff;
            if mpass % 2 == 0 {
                new_x = x;
            }
            mpass = mpass.wrapping_add(1);

            if new_x >= 0 && new_x < SCREEN_WIDTH as i8 && !self.concrete.collides(new_x, y, &curr)
            {
                x = new_x;
            }

            if self.controller.was_pressed() {
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

            if y > RESPAWN_THRESHOLD {
                self.screen.draw_figure(INIT_X, INIT_Y, &next, next_color);
            }

            if !self.concrete.collides(x, y, &curr) {
                self.screen.draw_figure(x, y, &curr, curr_color);
            } else {
                self.screen.draw_figure(x, y - 1, &curr, curr_color);
                self.concrete.draw_figure(x, y - 1, &curr, curr_color);

                x = INIT_X;
                y = INIT_Y + 1;

                if self.concrete.collides(x, y, &curr) {
                    self.game_over(leds, Dot::new(x, y), &curr, curr_color)
                        .await;
                    return;
                }

                curr_idx = next_idx;
                next_idx = self.prng.next_range(7);
                curr = TETRAMINO.wrapping_at(curr_idx);
                next = TETRAMINO.wrapping_at(next_idx);
            }
            if mpass % 2 == 0 {
                if let Some(row) = self.reduce_concrete() {
                    self.score += 1;
                    self.shift_concrete(row);
                }
            }

            self.screen.render(&mut leds);
            self.display.write(&leds).await;

            if self.score > 99 {
                self.score = 0;
            }

            let speed_bonus = (self.score / 2 / 10).max(1) as i8;
            let y_input = self.controller.read_y().await;
            let down_bonus: i8 = if y_input > 0 { 10 } else { 0 };

            ipass += speed_bonus + down_bonus;
            self.timer.sleep_millis(50).await;
        }
    }
}
