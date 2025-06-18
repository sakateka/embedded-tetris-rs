use embassy_time::Timer;
use smart_leds::RGB8;

use crate::{
    common::{GameController, LedDisplay, COLORS, GREEN_IDX, RED_IDX, SCREEN_HEIGHT, SCREEN_WIDTH},
    digits::DIGITS,
    figure::{Figure, TANK},
    Dot, FrameBuffer, Game, Prng,
};

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
    missiles: [Missile; 8],
    pos: Dot,
    origin: i8,
    rotation: u8,
    rotations: [Dot; 4],
    figure: Figure,
    lives: i8,
}

impl Tank {
    pub fn new(pos: Dot, origin: i8) -> Self {
        Self {
            missiles: [Missile::new(-1, -1, 0, 0); 8],
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
            lives: 1,
        }
    }

    fn player(&self) -> bool {
        self.origin == -1
    }

    fn direction(&self) -> Dot {
        self.rotations[self.rotation as usize % self.rotations.len()]
    }

    fn rotate(&mut self, direction: &Dot) -> bool {
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

    fn move_(
        &mut self,
        direction: &Dot,
        collides: impl Fn(i8, i8, &Figure) -> bool,
        allow_backward: bool,
    ) {
        if !direction.is_zero()
            && ((allow_backward && self.direction().is_opposite(direction))
                || !self.rotate(direction))
        {
            let pos = self.pos.move_by(*direction);
            if !collides(pos.x, pos.y, &self.figure) {
                self.pos = pos;
            }
        }
    }

    fn move_forward(&mut self) {
        let direction = self.direction();
        self.pos = self.pos.move_by(direction);
    }

    fn fire(&mut self) {
        let direction = self.direction();
        for m in &mut self.missiles {
            if !m.visible() {
                m.x = self.pos.x + direction.x;
                m.y = self.pos.y + direction.y;
                m.dx = direction.x;
                m.dy = direction.y;
                break;
            }
        }
    }

    fn move_missiles(&mut self) {
        for m in &mut self.missiles {
            m.move_()
        }
    }

    fn collides(&self, pos: Dot) -> bool {
        pos.x >= self.pos.x
            && pos.x < self.pos.x + 2
            && pos.y >= self.pos.y
            && pos.y < self.pos.y + 2
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
pub struct TanksGame {
    screen: FrameBuffer,
    tank: Tank,
    enemies: [Tank; 4],
    enemy_count: usize,
    score: u32,
    lives: i8,
    prng: Prng,
}

impl TanksGame {
    const STAGE_SPAWN: u8 = 0;
    const STAGE_MOVE: u8 = 1;
    const STAGE_ROTATE: u8 = 2;
    const STAGE_FIRE: u8 = 3;
    const STAGE_NONE: u8 = 4;

    pub fn new(prng: Prng) -> Self {
        Self {
            screen: FrameBuffer::new(),
            tank: Tank::new(Dot::new(3, 16), -1),
            enemies: [Tank::new(Dot::new(0, 0), 0); 4],
            enemy_count: 0,
            score: 0,
            lives: 3,
            prng,
        }
    }

    fn can_hit_player(&self, tank: &Tank) -> bool {
        let direction = tank.direction();
        let pos = tank.pos.move_by(Dot::new(1, 1)); // Tank center
        let player_center = self.tank.pos.move_by(Dot::new(1, 1));

        if direction.x != 0 {
            pos.y == player_center.y
                && ((direction.x > 0 && pos.x < player_center.x)
                    || (direction.x < 0 && pos.x > player_center.x))
        } else if direction.y != 0 {
            pos.x == player_center.x
                && ((direction.y > 0 && pos.y < player_center.y)
                    || (direction.y < 0 && pos.y > player_center.y))
        } else {
            false
        }
    }

    fn has_line_of_sight(&self, tank: &Tank, target_pos: Dot) -> bool {
        let tank_center = tank.pos.move_by(Dot::new(1, 1));
        let target_center = target_pos.move_by(Dot::new(1, 1));

        let dx = target_center.x - tank_center.x;
        let dy = target_center.y - tank_center.y;

        if dx != 0 && dy != 0 {
            return false;
        }

        let steps = dx.abs().max(dy.abs());
        if steps == 0 {
            return true;
        }

        let step_x = if dx == 0 {
            0
        } else if dx > 0 {
            1
        } else {
            -1
        };
        let step_y = if dy == 0 {
            0
        } else if dy > 0 {
            1
        } else {
            -1
        };

        for i in 1..steps.min(5) {
            let check_pos = Dot::new(tank_center.x + i * step_x, tank_center.y + i * step_y);
            if self.screen.collides(check_pos.x, check_pos.y, &tank.figure) {
                return false;
            }
        }

        true
    }

    fn _should_fire(&mut self, tank: &Tank) -> bool {
        if self.can_hit_player(tank) {
            return true;
        }
        self.prng.next_range(10) == 0
    }

    fn _smart_move(&mut self, tank: &mut Tank) {
        let player_pos = self.tank.pos;
        let tank_pos = tank.pos;

        let dx = player_pos.x - tank_pos.x;
        let dy = player_pos.y - tank_pos.y;

        let mut preferred_directions = [Dot::new(0, 0); 4];
        let mut dir_count = 0;

        if dx.abs() > dy.abs() {
            if dx > 0 {
                preferred_directions[dir_count] = Dot::new(1, 0);
                dir_count += 1;
            } else {
                preferred_directions[dir_count] = Dot::new(-1, 0);
                dir_count += 1;
            }
            if dy > 0 {
                preferred_directions[dir_count] = Dot::new(0, 1);
                dir_count += 1;
            } else if dy < 0 {
                preferred_directions[dir_count] = Dot::new(0, -1);
                dir_count += 1;
            }
        } else {
            if dy > 0 {
                preferred_directions[dir_count] = Dot::new(0, 1);
                dir_count += 1;
            } else {
                preferred_directions[dir_count] = Dot::new(0, -1);
                dir_count += 1;
            }
            if dx > 0 {
                preferred_directions[dir_count] = Dot::new(1, 0);
                dir_count += 1;
            } else if dx < 0 {
                preferred_directions[dir_count] = Dot::new(-1, 0);
                dir_count += 1;
            }
        }

        for i in 0..dir_count {
            let pos = tank.pos.move_by(preferred_directions[i]);
            if !self.screen.collides(pos.x, pos.y, &tank.figure) {
                tank.move_(
                    &preferred_directions[i],
                    |x, y, _| self.screen.collides(x, y, &TANK),
                    true,
                );
                return;
            }
        }

        let direction = tank.direction();
        let pos = tank.pos.move_by(direction);
        if self.screen.collides(pos.x, pos.y, &tank.figure) {
            tank.rotate(&direction);
        }
        tank.move_(
            &direction,
            |x, y, _| self.screen.collides(x, y, &TANK),
            false,
        );
    }

    fn ai(&mut self) {
        let stage = if self.enemy_count > 1 {
            // Weight stages based on game state
            let weighted_stages = if self.tank.lives <= 1 {
                // More aggressive when player has fewer lives
                [
                    Self::STAGE_FIRE,
                    Self::STAGE_FIRE,
                    Self::STAGE_FIRE,
                    Self::STAGE_FIRE,
                    Self::STAGE_MOVE,
                    Self::STAGE_MOVE,
                    Self::STAGE_ROTATE,
                    Self::STAGE_ROTATE,
                    Self::STAGE_NONE,
                ]
            } else {
                // Normal stage distribution
                [
                    Self::STAGE_FIRE,
                    Self::STAGE_FIRE,
                    Self::STAGE_MOVE,
                    Self::STAGE_MOVE,
                    Self::STAGE_MOVE,
                    Self::STAGE_ROTATE,
                    Self::STAGE_ROTATE,
                    Self::STAGE_ROTATE,
                    Self::STAGE_NONE,
                ]
            };
            weighted_stages[self.prng.next_range(weighted_stages.len() as u8) as usize]
        } else {
            Self::STAGE_SPAWN
        };

        match stage {
            Self::STAGE_SPAWN => {
                if self.enemy_count < self.enemies.len() {
                    let spawns = [
                        Dot::new(0, 6),
                        Dot::new(5, 6),
                        Dot::new(0, 29),
                        Dot::new(5, 29),
                    ];

                    // Smart spawning - prefer spawns closer to player
                    let mut spawn_priorities: [(usize, i32); 32] = [(0, 0); 32];
                    let mut spawn_count = 0;
                    let player_pos = self.tank.pos;

                    for (idx, &spawn_pos) in spawns.iter().enumerate() {
                        if !self.enemies.iter().any(|e| e.origin == idx as i8) {
                            // Calculate distance to player (Manhattan distance)
                            let distance = (spawn_pos.x - player_pos.x).abs()
                                + (spawn_pos.y - player_pos.y).abs();
                            // Closer spawns get higher priority (lower distance = higher priority)
                            let priority = 100 - distance;
                            spawn_priorities[spawn_count] = (idx, priority.into());
                            spawn_count += 1;
                        }
                    }

                    if spawn_count > 0 {
                        // Weighted random selection favoring closer spawns
                        let total_weight: i32 = spawn_priorities[..spawn_count]
                            .iter()
                            .map(|(_, p)| *p)
                            .sum();
                        if total_weight > 0 {
                            let rand_val = self.prng.next_range(total_weight as u8) as i32;
                            let mut current_weight = 0;
                            for i in 0..spawn_count {
                                let (idx, priority) = spawn_priorities[i];
                                current_weight += priority;
                                if rand_val < current_weight {
                                    let pos = spawns[idx];
                                    self.enemies[self.enemy_count] = Tank::new(pos, idx as i8);
                                    self.enemy_count += 1;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            Self::STAGE_NONE => {}
            _ => {
                // Select tank based on strategic priority
                let mut tank_priorities: [(usize, i32); 32] = [(0, 0); 32];
                let mut tank_count = 0;
                let player_pos = self.tank.pos;

                for i in 0..self.enemy_count {
                    let enemy = &self.enemies[i];
                    if enemy.is_dying() {
                        continue;
                    }

                    let mut priority = 1;

                    // Higher priority for tanks closer to player
                    let distance =
                        (enemy.pos.x - player_pos.x).abs() + (enemy.pos.y - player_pos.y).abs();
                    priority += (20 - distance).max(0);

                    // Higher priority for tanks that can hit player in current direction
                    if self.can_hit_player(enemy) {
                        priority += 15;
                    }

                    // Higher priority for tanks with clear line of sight
                    if self.has_line_of_sight(enemy, player_pos) {
                        priority += 10;
                    }

                    tank_priorities[tank_count] = (i, priority.into());
                    tank_count += 1;
                }

                if tank_count > 0 {
                    // Weighted selection
                    let total_weight: i32 =
                        tank_priorities[..tank_count].iter().map(|(_, p)| *p).sum();
                    let mut selected_idx = tank_priorities[0].0; // fallback

                    if total_weight > 0 {
                        let rand_val = self.prng.next_range(total_weight as u8) as i32;
                        let mut current_weight = 0;
                        for i in 0..tank_count {
                            let (idx, priority) = tank_priorities[i];
                            current_weight += priority;
                            if rand_val < current_weight {
                                selected_idx = idx;
                                break;
                            }
                        }
                    }

                    let mut enemy = self.enemies[selected_idx];
                    match stage {
                        Self::STAGE_MOVE => {
                            let direction = enemy.direction();
                            let pos = enemy.pos.move_by(direction);
                            if !self.screen.collides(pos.x, pos.y, &enemy.figure) {
                                enemy.move_forward();
                            } else {
                                enemy.rotate(&Dot::new(0, 0));
                            }
                        }
                        Self::STAGE_FIRE => {
                            let can_hit = self.can_hit_player(&enemy);
                            let random_fire = self.prng.next_range(10) == 0;
                            if can_hit || random_fire {
                                enemy.fire();
                            }
                        }
                        Self::STAGE_ROTATE => {
                            // Smart rotation towards player
                            let dx = player_pos.x - enemy.pos.x;
                            let dy = player_pos.y - enemy.pos.y;

                            let target_direction = if dx.abs() > dy.abs() {
                                Dot::new(if dx > 0 { 1 } else { -1 }, 0)
                            } else {
                                Dot::new(0, if dy > 0 { 1 } else { -1 })
                            };

                            if enemy.direction() != target_direction {
                                enemy.rotate(&target_direction);
                            } else if self.prng.next_range(4) == 0 {
                                // Occasionally rotate randomly for unpredictability
                                enemy.rotate(&Dot::new(0, 0));
                            }
                        }
                        _ => {}
                    }
                    enemy.move_missiles();
                    self.enemies[selected_idx] = enemy;
                }
            }
        }
    }

    fn update_enemies(&mut self) {
        let stage = if self.enemy_count > 1 {
            let stages = if self.tank.lives <= 1 {
                [
                    Self::STAGE_FIRE,
                    Self::STAGE_FIRE,
                    Self::STAGE_FIRE,
                    Self::STAGE_FIRE,
                    Self::STAGE_MOVE,
                    Self::STAGE_MOVE,
                    Self::STAGE_ROTATE,
                    Self::STAGE_ROTATE,
                    Self::STAGE_NONE,
                ]
            } else {
                [
                    Self::STAGE_FIRE,
                    Self::STAGE_FIRE,
                    Self::STAGE_MOVE,
                    Self::STAGE_MOVE,
                    Self::STAGE_MOVE,
                    Self::STAGE_ROTATE,
                    Self::STAGE_ROTATE,
                    Self::STAGE_ROTATE,
                    Self::STAGE_NONE,
                ]
            };
            stages[self.prng.next_range(stages.len() as u8) as usize]
        } else {
            Self::STAGE_SPAWN
        };

        match stage {
            Self::STAGE_SPAWN => {
                if self.enemy_count < self.enemies.len() {
                    let spawns = [
                        Dot::new(0, 6),
                        Dot::new(5, 6),
                        Dot::new(0, 29),
                        Dot::new(5, 29),
                    ];

                    for (idx, &spawn_pos) in spawns.iter().enumerate() {
                        if !self.enemies.iter().any(|e| e.origin == idx as i8) {
                            self.enemies[self.enemy_count] = Tank::new(spawn_pos, idx as i8);
                            self.enemy_count += 1;
                            break;
                        }
                    }
                }
            }
            Self::STAGE_NONE => {}
            _ => {
                for i in 0..self.enemy_count {
                    let mut enemy = self.enemies[i];
                    if enemy.is_dying() {
                        continue;
                    }

                    match stage {
                        Self::STAGE_MOVE => {
                            let direction = enemy.direction();
                            let pos = enemy.pos.move_by(direction);
                            if !self.screen.collides(pos.x, pos.y, &enemy.figure) {
                                enemy.move_forward();
                            } else {
                                enemy.rotate(&Dot::new(0, 0));
                            }
                        }
                        Self::STAGE_FIRE => {
                            let can_hit = self.can_hit_player(&enemy);
                            let random_fire = self.prng.next_range(10) == 0;
                            if can_hit || random_fire {
                                enemy.fire();
                            }
                        }
                        Self::STAGE_ROTATE => {
                            enemy.rotate(&Dot::new(0, 0));
                        }
                        _ => {}
                    }
                    enemy.move_missiles();
                    self.enemies[i] = enemy
                }
            }
        }
    }

    fn draw_enemy_tank(&mut self, idx: usize) {
        let color = if self.tank.player() {
            GREEN_IDX
        } else {
            RED_IDX
        };
        self.screen.draw_figure(
            self.enemies[idx].pos.x,
            self.enemies[idx].pos.y,
            &self.enemies[idx].figure,
            color,
        );
    }

    fn draw_enemy_missiles(&mut self, idx: usize) {
        for m in &self.enemies[idx].missiles {
            if m.visible() {
                self.screen.set(m.x as usize, m.y as usize, RED_IDX);
            }
        }
    }
    fn draw_tank(&mut self) {
        let color = if self.tank.player() {
            GREEN_IDX
        } else {
            RED_IDX
        };
        self.screen
            .draw_figure(self.tank.pos.x, self.tank.pos.y, &self.tank.figure, color);
    }

    fn draw_missiles(&mut self) {
        for m in &self.tank.missiles {
            if m.visible() {
                self.screen.set(m.x as usize, m.y as usize, RED_IDX);
            }
        }
    }

    fn draw_score(&mut self) {
        let mut score = self.score;
        let mut digits = [0u8; 4];
        let mut count = 0;

        while score > 0 && count < digits.len() {
            digits[count] = (score % 10) as u8;
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

    fn draw_lives(&mut self) {
        for i in 0..self.lives {
            self.screen.set(i as usize, 0, GREEN_IDX);
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
                            self.score += 100;
                        }
                    }
                }
            }
        }
    }

    async fn game_over<D, C>(&mut self, mut leds: [RGB8; 256], display: &mut D, controller: &mut C)
    where
        D: LedDisplay,
        C: GameController,
    {
        while !controller.was_pressed() {
            let x = self.prng.next_range(SCREEN_WIDTH as u8);
            let y = self.prng.next_range(SCREEN_HEIGHT as u8);
            let color = self.prng.next_range(COLORS.len() as u8);
            self.screen.set(x as usize, y as usize, color);
            self.screen.render(&mut leds);
            display.write(&leds).await;
        }
    }
}

impl Game for TanksGame {
    async fn run<D, C>(&mut self, display: &mut D, controller: &mut C)
    where
        D: LedDisplay,
        C: GameController,
    {
        let mut leds: [RGB8; 256] = [RGB8::default(); 256];

        let mut step = 10;
        let mut round = 10;

        loop {
            self.screen.clear();
            if self.tank.is_dead() {
                self.game_over(leds, display, controller).await;
                return;
            }

            if controller.was_pressed() {
                self.tank.fire();
            }

            let x_input = controller.read_x().await;
            let y_input = controller.read_y().await;
            let direction = Dot::new(x_input, y_input).to_direction();

            (0..self.enemy_count).for_each(|i| {
                self.draw_enemy_tank(i);
            });

            let fig = self.tank.figure;
            self.tank
                .move_(&direction, |x, y, _| self.screen.collides(x, y, &fig), true);

            self.tank.move_missiles();
            self.update_enemies();
            self.check_collisions();

            self.draw_tank();
            for i in 0..self.enemy_count {
                self.draw_enemy_tank(i);
                self.draw_enemy_missiles(i);
            }
            self.draw_missiles();
            self.draw_score();
            self.draw_lives();

            let speedup = self.score / 10;
            if step >= round - speedup {
                self.ai();
                step = 0;
                round += 1;
            }
            step += 1;

            self.screen.render(&mut leds);
            display.write(&leds).await;
            Timer::after_millis(100).await;
        }
    }
}
