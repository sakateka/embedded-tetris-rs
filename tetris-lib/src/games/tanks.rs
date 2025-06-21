use core::default::Default;
use smart_leds::RGB8;

use crate::common::{
    Dot, FrameBuffer, Game, GameController, LedDisplay, Prng, Timer, BRICK_IDX, COLORS, GREEN_IDX,
    PINK_IDX, RED_IDX, SCREEN_HEIGHT, SCREEN_WIDTH,
};

use crate::digits::DIGITS;
use crate::figure::{Figure, TANK};

#[derive(Clone, Copy)]
struct Missile {
    x: i8,
    y: i8,
    dx: i8,
    dy: i8,
}

impl Missile {
    fn new(x: i8, y: i8, dx: i8, dy: i8) -> Self {
        Self { x, y, dx, dy }
    }

    fn move_(&mut self) {
        self.x += self.dx;
        self.y += self.dy;
    }

    fn visible(&self) -> bool {
        self.x >= 0 && self.x < 8 && self.y >= 0 && self.y < 32
    }

    fn hide(&mut self) {
        self.x = -1;
        self.y = -1;
    }
}

#[derive(Clone, Copy)]
struct Tank {
    missiles: [Missile; 2],
    pos: Dot,
    origin: i8,
    rotation: u8,
    rotations: [Dot; 4],
    figure: Figure,
    lives: i8,
}

impl Tank {
    pub fn new(pos: Dot, origin: i8, lives: i8) -> Self {
        Self {
            missiles: [Missile::new(-1, -1, 0, 0); 2],
            pos,
            origin,
            rotation: 2,
            rotations: [
                Dot::new(-1, 0),
                Dot::new(0, -1),
                Dot::new(1, 0),
                Dot::new(0, 1),
            ],
            figure: TANK,
            lives,
        }
    }

    fn direction(&self) -> Dot {
        self.rotations[self.rotation as usize % self.rotations.len()]
    }

    fn rotate(&mut self, direction: &Dot) -> bool {
        if direction.is_zero() {
            self.figure = self.figure.rotate();
            self.rotation = (self.rotation + 1) % self.rotations.len() as u8;
            return true;
        }

        let mut rotated = 0;
        while self.direction() != *direction {
            self.figure = self.figure.rotate();
            self.rotation = (self.rotation + 1) % self.rotations.len() as u8;
            rotated += 1;
            if rotated == self.rotations.len() {
                return false;
            }
        }
        rotated > 0
    }

    fn fire(&mut self) {
        let direction = self.direction();
        for m in &mut self.missiles {
            if !m.visible() {
                m.x = self.pos.x + 1 + direction.x;
                m.y = self.pos.y + 1 + direction.y;

                if direction.x < 0 {
                    m.x -= 1;
                }
                if direction.y < 0 {
                    m.y -= 1;
                }

                m.dx = direction.x;
                m.dy = direction.y;
                break;
            }
        }
    }

    fn move_missiles(&mut self) {
        for m in &mut self.missiles {
            if m.visible() {
                m.move_();
                if !m.visible() {
                    m.hide();
                }
            }
        }
    }

    fn collides(&self, pos: Dot) -> bool {
        for row in 0..self.figure.height() {
            for col in 0..self.figure.width() {
                if self.figure.get_bit(col, row) {
                    let tank_pixel_x = self.pos.x + col as i8;
                    let tank_pixel_y = self.pos.y + row as i8;

                    if tank_pixel_x == pos.x && tank_pixel_y == pos.y {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn overlaps_figure(&self, x: i8, y: i8, figure: &Figure) -> bool {
        for row in 0..self.figure.height() {
            for col in 0..self.figure.width() {
                if self.figure.get_bit(col, row) {
                    let pixel = Dot::new(self.pos.x + col as i8, self.pos.y + row as i8);

                    for other_row in 0..figure.height() {
                        for other_col in 0..figure.width() {
                            if figure.get_bit(other_col, other_row) {
                                let other_pixel =
                                    Dot::new(x + other_col as i8, y + other_row as i8);
                                if pixel.x == other_pixel.x && pixel.y == other_pixel.y {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }

    fn hit(&mut self) {
        self.lives -= 1;
    }

    fn is_dead(&self) -> bool {
        self.lives <= 0
    }

    fn is_dying(&self) -> bool {
        self.lives < 0
    }
}

// Tanks game implementation
pub struct TanksGame<'a, D, C, T> {
    screen: FrameBuffer,
    display: &'a mut D,
    controller: &'a mut C,
    timer: &'a T,

    tank: Tank,
    enemies: [Tank; 4],
    enemy_count: usize,
    score: u8,
    prng: Prng,
}

impl<'a, D: LedDisplay, C: GameController, T: Timer> TanksGame<'a, D, C, T> {
    pub fn new(prng: Prng, display: &'a mut D, controller: &'a mut C, timer: &'a T) -> Self {
        Self {
            screen: FrameBuffer::new(),
            display,
            controller,
            timer,

            tank: Tank::new(Dot::new(3, 16), -1, 3),
            enemies: [Tank::new(Dot::new(0, 0), 0, 1); 4],
            enemy_count: 0,
            score: 0,
            prng,
        }
    }

    fn collides(
        &self,
        x: i8,
        y: i8,
        tank: &Tank,
        exclude_player: bool,
        exclude_enemy_idx: Option<usize>,
    ) -> bool {
        self.screen.collides(x, y, &tank.figure)
            || (!exclude_player
                && !self.tank.is_dead()
                && self.tank.overlaps_figure(x, y, &tank.figure))
            || self.enemies[..self.enemy_count]
                .iter()
                .enumerate()
                .any(|(i, e)| {
                    !e.is_dead()
                        && Some(i) != exclude_enemy_idx
                        && e.overlaps_figure(x, y, &tank.figure)
                })
    }

    fn ai(&mut self) {
        self.spawn_enemies();
        self.move_enemies();
        self.remove_dead_enemies();
    }

    fn spawn_enemies(&mut self) {
        if self.enemy_count >= 3 || self.enemy_count >= self.enemies.len() {
            return;
        }

        let spawns = [
            Dot::new(0, 6),  // top-left (moved down to avoid delimiter)
            Dot::new(5, 6),  // top-right (moved down to avoid delimiter)
            Dot::new(0, 29), // bottom-left
            Dot::new(5, 29), // bottom-right
        ];

        // Find available spawn indices and randomly select one
        let mut available_spawns = [false; 4];
        let mut available_count = 0;

        for idx in 0..spawns.len() {
            if !self.enemies[..self.enemy_count]
                .iter()
                .any(|e| e.origin == idx as i8)
            {
                // Check if spawning here would collide with player
                let spawn_pos = spawns[idx];
                let test_tank = Tank::new(spawn_pos, idx as i8, 1);

                if !self
                    .tank
                    .overlaps_figure(spawn_pos.x, spawn_pos.y, &test_tank.figure)
                {
                    available_spawns[idx] = true;
                    available_count += 1;
                }
            }
        }

        if available_count > 0 {
            // Randomly select from available spawns
            let target = self.prng.next_range(available_count as u8) as usize;
            let mut current = 0;

            for idx in 0..spawns.len() {
                if available_spawns[idx] {
                    if current == target {
                        let mut enemy = Tank::new(spawns[idx], idx as i8, 1);
                        // Set random initial direction
                        let target_rotation = self.prng.next_range(4);
                        // Start from base figure and apply correct number of rotations
                        enemy.figure = TANK; // Reset to base tank sprite
                        for _ in 0..target_rotation {
                            enemy.rotate(&Dot::new(0, 0));
                        }

                        self.enemies[self.enemy_count] = enemy;
                        self.enemy_count += 1;
                        break;
                    }
                    current += 1;
                }
            }
        }
    }

    fn move_enemies(&mut self) {
        for i in 0..self.enemy_count {
            if self.enemies[i].is_dying() {
                continue;
            }

            if self.prng.next_range(10) == 0 {
                self.enemies[i].fire();
            }

            if self.prng.next_range(3) == 0 {
                // Increased movement frequency from 20% to 33%
                let mut enemy = self.enemies[i];
                if self.prng.next_range(2) == 0 {
                    self.try_move_enemy(&mut enemy, i);
                } else {
                    enemy.rotate(&Dot::new(0, 0));
                }
                self.enemies[i] = enemy;
            }
        }
    }

    fn try_move_enemy(&mut self, enemy: &mut Tank, enemy_idx: usize) {
        let direction = enemy.direction();
        let new_pos = enemy.pos.move_by(direction);

        // Check screen boundaries first
        if new_pos.x < 0 || new_pos.x > 5 || new_pos.y < 6 || new_pos.y > 29 {
            enemy.rotate(&Dot::new(0, 0));
            return;
        }

        // Check collisions (excluding self)
        let collides_with_player = !self.tank.is_dead()
            && self
                .tank
                .overlaps_figure(new_pos.x, new_pos.y, &enemy.figure);
        let collides_with_other_enemy =
            self.enemies[..self.enemy_count]
                .iter()
                .enumerate()
                .any(|(i, e)| {
                    i != enemy_idx
                        && !e.is_dead()
                        && e.overlaps_figure(new_pos.x, new_pos.y, &enemy.figure)
                });

        if collides_with_player || collides_with_other_enemy {
            enemy.rotate(&Dot::new(0, 0));
        } else {
            enemy.pos = new_pos;
        }
    }

    fn remove_dead_enemies(&mut self) {
        let mut write_idx = 0;
        for read_idx in 0..self.enemy_count {
            if !self.enemies[read_idx].is_dead() {
                self.enemies[write_idx] = self.enemies[read_idx];
                write_idx += 1;
            }
        }
        self.enemy_count = write_idx;
    }

    fn move_player(&mut self, direction: Dot) {
        // Tank movement: rotate OR move, not both
        if !direction.is_zero() {
            let was_facing_direction = self.tank.direction() == direction;

            // If not facing the desired direction, try to rotate
            if !was_facing_direction {
                let original_figure = self.tank.figure;
                let original_rotation = self.tank.rotation;

                if self.tank.rotate(&direction) {
                    // Check if rotation causes collision
                    if self.collides(self.tank.pos.x, self.tank.pos.y, &self.tank, true, None) {
                        // Revert rotation if it would cause collision
                        self.tank.figure = original_figure;
                        self.tank.rotation = original_rotation;
                    }
                }
            } else {
                // Already facing the right direction, try to move forward
                let new_pos = self.tank.pos.move_by(direction);
                if !self.collides(new_pos.x, new_pos.y, &self.tank, true, None) {
                    self.tank.pos = new_pos;
                }
            }
        }
    }

    fn move_missiles(&mut self) {
        self.tank.move_missiles();
        self.enemies.iter_mut().for_each(|e| e.move_missiles());
    }

    fn draw_player(&mut self) {
        self.screen.draw_figure(
            self.tank.pos.x,
            self.tank.pos.y,
            &self.tank.figure,
            GREEN_IDX,
        );
    }

    fn draw_enemy(&mut self, idx: usize) {
        let enemy = &self.enemies[idx];
        self.screen
            .draw_figure(enemy.pos.x, enemy.pos.y, &enemy.figure, BRICK_IDX);
        for m in &enemy.missiles {
            if m.visible() {
                self.screen.set(m.x as usize, m.y as usize, RED_IDX);
            }
        }
    }

    fn draw_player_missiles(&mut self) {
        for m in &self.tank.missiles {
            if m.visible() {
                self.screen.set(m.x as usize, m.y as usize, RED_IDX);
            }
        }
    }

    fn draw_score_delimiter(&mut self) {
        for x in 0..SCREEN_WIDTH {
            self.screen.set(x, 5, PINK_IDX);
        }
    }

    fn draw_score(&mut self) {
        let score_display = (self.score % 100) as usize;
        let tens = score_display / 10;
        let ones = score_display % 10;

        self.screen.draw_figure(0, 0, &DIGITS[tens], GREEN_IDX);
        self.screen.draw_figure(4, 0, &DIGITS[ones], GREEN_IDX);
    }

    fn draw_lives(&mut self) {
        for i in 0..self.tank.lives {
            self.screen.set(7, i as usize, PINK_IDX);
        }
    }

    fn check_collisions(&mut self) {
        for i in 0..self.enemy_count {
            let enemy = &mut self.enemies[i];
            for m in &mut enemy.missiles {
                if m.visible() && self.tank.collides(Dot::new(m.x, m.y)) {
                    self.tank.hit();
                    m.hide();
                }
            }
        }

        for m in &mut self.tank.missiles {
            if m.visible() {
                for j in 0..self.enemy_count {
                    let enemy = &mut self.enemies[j];
                    if enemy.collides(Dot::new(m.x, m.y)) {
                        enemy.hit();
                        m.hide();
                        if enemy.is_dead() {
                            self.score += 1;
                        }
                    }
                }
            }
        }
    }

    async fn game_over(&mut self, mut leds: [RGB8; 256]) {
        while !self.controller.joystick_was_pressed() {
            let x = self.prng.next_range(SCREEN_WIDTH as u8);
            let y = self.prng.next_range(SCREEN_HEIGHT as u8);
            let color = self.prng.next_range(COLORS.len() as u8);
            self.screen.set(x as usize, y as usize, color);
            self.screen.render(&mut leds);
            self.display.write(&leds).await;
            self.timer.sleep_millis(200).await;
        }
    }
}

impl<'a, D: LedDisplay, C: GameController, T: Timer> Game for TanksGame<'a, D, C, T> {
    async fn run(&mut self)
    where
        D: LedDisplay,
        C: GameController,
        T: Timer,
    {
        let mut leds: [RGB8; 256] = [RGB8::default(); 256];
        let mut step = 10;
        let round = 10;

        loop {
            self.screen.clear();
            self.draw_score();
            self.draw_lives();
            self.draw_score_delimiter();

            if self.tank.is_dead() {
                self.game_over(leds).await;
                return;
            }

            if self.controller.joystick_was_pressed() {
                self.tank.fire();
            }

            let x_input = self.controller.read_x().await;
            let y_input = self.controller.read_y().await;
            let direction = Dot::new(x_input, y_input).to_direction();

            self.move_player(direction);
            self.move_missiles();
            self.check_collisions();

            self.draw_player();
            for i in 0..self.enemy_count {
                self.draw_enemy(i);
            }
            self.draw_player_missiles();

            let speedup = self.score / 10;
            if step >= round {
                self.ai();
                step = 0;
            }
            step += 1 + speedup;

            self.screen.render(&mut leds);
            self.display.write(&leds).await;
            self.timer.sleep_millis(100).await;
        }
    }
}
