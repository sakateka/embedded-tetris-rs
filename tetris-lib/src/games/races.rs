use core::option::Option;
use smart_leds::RGB8;

use crate::{
    common::{Dot, FrameBuffer, Prng},
    common::{
        Game, GameController, LedDisplay, Timer, BLACK_IDX, BLUE_IDX, BRICK_IDX, DARK_GREEN_IDX,
        GREEN_IDX, PINK_IDX, RED_IDX, SCREEN_HEIGHT, SCREEN_WIDTH, YELLOW_IDX,
    },
    digits::DIGITS,
};

static ROAD_UPDATE_STEP_SIZE: u8 = 10;
static UPDATE_STEP_SIZE: u8 = ROAD_UPDATE_STEP_SIZE * 2;

// Races game implementation
pub struct RacesGame<'a, D, C, T> {
    screen: FrameBuffer,
    display: &'a mut D,
    controller: &'a mut C,
    timer: &'a T,

    update_step: u8,
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
    update_road: u8,
    road_animation: u8,
    bullet_powerup: Option<Dot>,
    prng: Prng,
}

impl<'a, D: LedDisplay, C: GameController, T: Timer> RacesGame<'a, D, C, T> {
    pub fn new(prng: Prng, display: &'a mut D, controller: &'a mut C, timer: &'a T) -> Self {
        let mut game = Self {
            screen: FrameBuffer::new(),
            display,
            controller,
            timer,

            update_step: 0,
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
            update_road: 0,
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

                // Draw car body (check bounds to prevent underflow)
                if x > 0 && x < SCREEN_WIDTH - 1 && y < SCREEN_HEIGHT {
                    self.screen.set(x - 1, y, BLUE_IDX);
                    self.screen.set(x, y, BLUE_IDX);
                    self.screen.set(x + 1, y, BLUE_IDX);
                }
                if y > 0 && x < SCREEN_WIDTH {
                    self.screen.set(x, y - 1, BLUE_IDX);
                }
                if y > 1 && x > 0 && x < SCREEN_WIDTH - 1 {
                    self.screen.set(x, y - 2, BLUE_IDX);
                    self.screen.set(x - 1, y - 2, BLUE_IDX);
                    self.screen.set(x + 1, y - 2, BLUE_IDX);
                }
                if y > 2 && x < SCREEN_WIDTH {
                    self.screen.set(x, y - 3, BLUE_IDX);
                }
            }
        }
    }
    fn update_road(&mut self) {
        self.update_road = (self.update_road + 1) % 4;
        self.road_animation = (self.road_animation + 1) % SCREEN_HEIGHT as u8;
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
            self.bullets[i].y = self.bullets[i].y.saturating_sub(1);

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
        let mut bricks = 0i8;
        let mut part = true;
        for y in 0..SCREEN_HEIGHT {
            if bricks == 4 {
                bricks = 0;
                part = !part;
            };
            bricks += 1;

            let color = if part { BRICK_IDX } else { BLACK_IDX };

            // Left edge
            self.screen
                .set(0, (y + self.road_animation as usize) % SCREEN_HEIGHT, color);
            // Right edge
            self.screen
                .set(7, (y + self.road_animation as usize) % SCREEN_HEIGHT, color);
        }
    }

    fn draw_car(&mut self) {
        let x = self.car_pos.x as usize;
        let y = self.car_pos.y as usize;

        if self.invulnerable_time > 0 && (self.invulnerable_time / 4) % 2 == 0 {
            // Blink car when invulnerable
            return;
        }

        // Draw car body (check bounds to prevent underflow)
        if x > 0 && x < SCREEN_WIDTH - 1 && y < SCREEN_HEIGHT {
            self.screen.set(x - 1, y, GREEN_IDX);
            self.screen.set(x, y, GREEN_IDX);
            self.screen.set(x + 1, y, GREEN_IDX);
        }
        if y > 0 && x < SCREEN_WIDTH {
            self.screen.set(x, y - 1, GREEN_IDX);
        }
        if y > 1 && x > 0 && x < SCREEN_WIDTH - 1 {
            self.screen.set(x, y - 2, GREEN_IDX);
            self.screen.set(x - 1, y - 2, GREEN_IDX);
            self.screen.set(x + 1, y - 2, GREEN_IDX);
        }
        if y > 2 && x < SCREEN_WIDTH {
            self.screen.set(x, y - 3, GREEN_IDX);
        }
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
        self.screen.draw_figure(tens_x, 0, tens_figure, YELLOW_IDX);

        // Draw right digit (ones)
        let ones = score % 10;
        let ones_figure = DIGITS.wrapping_at(ones);
        // Add extra space for digit one
        let ones_x = if ones == 1 { 6 } else { 5 };
        self.screen.draw_figure(ones_x, 0, ones_figure, YELLOW_IDX);

        // Draw vertical line of lives in the middle
        for y in 0..self.lives {
            self.screen.set(3, y as usize, GREEN_IDX);
        }

        // Draw bullet count to the right of lives in pink
        for y in 0..self.max_bullets {
            self.screen.set(4, y as usize, PINK_IDX);
        }
    }

    async fn game_over(&mut self, mut leds: [RGB8; 256]) {
        for _ in 0..3 {
            self.screen.clear();
            self.screen.render(&mut leds);
            self.display.write(&leds).await;
            self.timer.sleep_millis(200).await;

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

    fn road_should_update(&mut self) -> bool {
        self.update_step = (self.update_step + 1) % UPDATE_STEP_SIZE;
        self.update_step % ROAD_UPDATE_STEP_SIZE == 0
    }
    fn can_move_car_horizontally(&mut self) -> bool {
        self.update_step % (ROAD_UPDATE_STEP_SIZE / 4) == 0
    }

    fn should_update(&mut self) -> bool {
        self.update_step == 0
    }
}

impl<'a, D: LedDisplay, C: GameController, T: Timer> Game for RacesGame<'a, D, C, T> {
    async fn run(&mut self) {
        let mut leds = [RGB8::new(0, 0, 0); 256];

        loop {
            // Fire bullet on button press
            if self.controller.was_pressed()
                && self.bullet_count < self.bullets.len()
                && self.max_bullets > 0
            {
                self.bullets[self.bullet_count] = Dot::new(self.car_pos.x, self.car_pos.y - 4);
                self.bullet_count += 1;
                self.max_bullets -= 1; // Decrement available bullets when firing
            }

            self.spawn_obstacles();
            self.spawn_bullet_powerup();
            // Handle joystick input
            let x = self.controller.read_x().await;
            let y = self.controller.read_y().await;

            if self.can_move_car_horizontally() {
                // Move car horizontally
                if x != 0 {
                    let new_x = self.car_pos.x + x;
                    if new_x >= 1 && new_x <= SCREEN_WIDTH as i8 - 2 {
                        self.car_pos.x = new_x;
                    }
                }
            }

            if self.road_should_update() {
                // Move car vertically
                if y != 0 {
                    let new_y = self.car_pos.y + y;
                    if new_y >= 3 && new_y < SCREEN_HEIGHT as i8 {
                        self.car_pos.y = new_y;
                    }
                }

                self.update_obstacles();
                self.update_road();
            }

            // Update game state
            if self.should_update() {
                self.update_bullet_powerup();
                self.update_racing_cars();
            }
            self.update_bullets();
            self.check_collisions();

            // Check game over
            if self.lives == 0 {
                self.game_over(leds).await;
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
            self.display.write(&leds).await;

            self.timer.sleep_millis(20).await;
        }
    }
}
