use embassy_time::Timer;
use smart_leds::RGB8;

use crate::{
    common::{
        Game, GameController, LedDisplay, BLACK_IDX, BLUE_IDX, BRICK_IDX, GREEN_IDX,
        LIGHT_BLUE_IDX, PINK_IDX, RED_IDX, SCREEN_HEIGHT, SCREEN_WIDTH, YELLOW_IDX,
    },
    digits::DIGITS,
    figure::{Figure, TETRAMINO},
    Dot, FrameBuffer, Prng,
};

pub struct TetrisGame {
    screen: FrameBuffer,
    concrete: FrameBuffer,
    score: u8,
    init_x: i8,
    init_y: i8,
    next_visible_y: i8,
    prng: Prng,
}

impl TetrisGame {
    pub fn new(prng: Prng) -> Self {
        Self {
            screen: FrameBuffer::new(),
            concrete: FrameBuffer::new(),
            score: 0,
            init_x: 3,
            init_y: 6,
            next_visible_y: 11,
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

    async fn animate_concrete_shift<D: LedDisplay>(
        &mut self,
        display: &mut D,
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
                display.write(&leds).await;
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
        display.write(&leds).await;
    }

    async fn game_over<D: LedDisplay, C: GameController>(
        &mut self,
        mut leds: [RGB8; 256],
        display: &mut D,
        controller: &mut C,
        last_pos: Dot,
        last_figure: &Figure,
        last_color: u8,
    ) {
        while !controller.was_pressed() {
            // Preserve the concrete blocks and score
            self.screen.copy_from(&self.concrete);
            self.draw_score();

            // Blink the last tetramino
            self.screen
                .draw_figure(last_pos.x, last_pos.y - 1, last_figure, last_color);
            self.screen.render(&mut leds);
            display.write(&leds).await;
            Timer::after_millis(500).await;

            // Clear only the last tetramino
            self.screen
                .draw_figure(last_pos.x, last_pos.y - 1, last_figure, BLACK_IDX);
            self.screen.render(&mut leds);
            display.write(&leds).await;
            Timer::after_millis(500).await;
        }
    }
}

impl Game for TetrisGame {
    async fn run<D, C>(&mut self, display: &mut D, controller: &mut C)
    where
        D: LedDisplay,
        C: GameController,
    {
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
            let x_diff = controller.read_x().await;
            let new_x = x + x_diff;

            if new_x >= 0 && new_x < SCREEN_WIDTH as i8 && !self.concrete.collides(new_x, y, &curr)
            {
                x = new_x;
            }

            if controller.was_pressed() {
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
                    self.game_over(leds, display, controller, Dot::new(x, y), &curr, curr_color)
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
                    self.animate_concrete_shift(display, &cleared_rows, count)
                        .await;
                }
            }

            self.screen.render(&mut leds);
            display.write(&leds).await;

            if self.score > 99 {
                self.score = 0;
            }

            let speed_bonus = (self.score / 10).max(1);
            let y_input = controller.read_y().await;
            let down_bonus = if y_input > 0 { 10 } else { 0 };

            ipass += speed_bonus + down_bonus;
            Timer::after_millis(50).await;
        }
    }
}
