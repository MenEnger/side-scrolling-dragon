use std::io::{self, Stdout, Write, stdout};
use std::thread::sleep;
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::style::Print;
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use crossterm::{ExecutableCommand, QueueableCommand};

const WIDTH: usize = 56;
const HEIGHT: usize = 18;
const GROUND_ROW: usize = HEIGHT - 3;
const PLAYER_X: usize = 8;
const MAX_JUMPS: u8 = 2;
const GRAVITY: f32 = 42.0;
const JUMP_VELOCITY: f32 = -15.0;
const FRAME_TIME: Duration = Duration::from_millis(50);

#[derive(Clone, Copy)]
struct Dragon {
    y: f32,
    velocity_y: f32,
    jumps_used: u8,
}

impl Dragon {
    fn new() -> Self {
        Self {
            y: ground_y(),
            velocity_y: 0.0,
            jumps_used: 0,
        }
    }

    fn jump(&mut self) {
        if self.jumps_used < MAX_JUMPS {
            self.velocity_y = JUMP_VELOCITY;
            self.jumps_used += 1;
        }
    }

    fn update(&mut self, dt: f32) {
        self.velocity_y += GRAVITY * dt;
        self.y += self.velocity_y * dt;

        if self.y >= ground_y() {
            self.y = ground_y();
            self.velocity_y = 0.0;
            self.jumps_used = 0;
        }
    }

    fn row(&self) -> usize {
        self.y.round().clamp(0.0, ground_y()) as usize
    }
}

#[derive(Clone, Copy)]
struct Obstacle {
    x: f32,
    height: usize,
    passed: bool,
}

impl Obstacle {
    fn new(rng: &mut Lcg, score: u32) -> Self {
        Self {
            x: (WIDTH - 1) as f32,
            height: obstacle_height_for_score(rng, score),
            passed: false,
        }
    }

    fn update(&mut self, speed: f32, dt: f32) {
        self.x -= speed * dt;
    }

    fn col(&self) -> isize {
        self.x.round() as isize
    }

    fn offscreen(&self) -> bool {
        self.x < -1.0
    }
}

struct Game {
    dragon: Dragon,
    obstacles: Vec<Obstacle>,
    spawn_timer: f32,
    spawn_interval: f32,
    score: u32,
    game_over: bool,
    rng: Lcg,
}

impl Game {
    fn new() -> Self {
        let mut rng = Lcg::new(0xC0FFEE_u64);
        let spawn_interval = next_spawn_interval(&mut rng, 0, 1);

        Self {
            dragon: Dragon::new(),
            obstacles: Vec::new(),
            spawn_timer: 0.0,
            spawn_interval,
            score: 0,
            game_over: false,
            rng,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn speed(&self) -> f32 {
        18.0 + self.score as f32 * 0.45
    }

    fn update(&mut self, dt: f32) {
        if self.game_over {
            return;
        }

        self.dragon.update(dt);
        self.spawn_timer += dt;

        if self.spawn_timer >= self.spawn_interval {
            self.spawn_timer = 0.0;
            let obstacle = Obstacle::new(&mut self.rng, self.score);
            self.spawn_interval = next_spawn_interval(&mut self.rng, self.score, obstacle.height);
            self.obstacles.push(obstacle);
        }

        let speed = self.speed();
        for obstacle in &mut self.obstacles {
            obstacle.update(speed, dt);
            if !obstacle.passed && obstacle.x < PLAYER_X as f32 {
                obstacle.passed = true;
                self.score += 1;
            }
        }

        self.obstacles.retain(|obstacle| !obstacle.offscreen());

        let dragon_row = self.dragon.row();
        let dragon_col = PLAYER_X as isize;
        for obstacle in &self.obstacles {
            let obstacle_col = obstacle.col();
            let obstacle_top = GROUND_ROW.saturating_sub(obstacle.height.saturating_sub(1));
            if obstacle_col == dragon_col && dragon_row >= obstacle_top {
                self.game_over = true;
                break;
            }
        }
    }

    fn render(&self) -> Vec<String> {
        let mut grid = vec![vec![' '; WIDTH]; HEIGHT];

        for cell in &mut grid[GROUND_ROW + 1] {
            *cell = '=';
        }
        for cell in &mut grid[GROUND_ROW + 2] {
            *cell = '=';
        }

        for x in 0..WIDTH {
            if x % 9 == 0 {
                grid[2][x] = '.';
            }
            if x % 13 == 4 {
                grid[4][x] = '.';
            }
        }

        for obstacle in &self.obstacles {
            let col = obstacle.col();
            if !(0..WIDTH as isize).contains(&col) {
                continue;
            }

            let col = col as usize;
            for y in 0..obstacle.height {
                let row = GROUND_ROW.saturating_sub(y);
                grid[row][col] = '#';
            }
        }

        let dragon_row = self.dragon.row();
        let dragon_sprite = if self.dragon.jumps_used == 2 { '&' } else { '@' };
        grid[dragon_row][PLAYER_X] = dragon_sprite;

        if dragon_row > 0 {
            grid[dragon_row - 1][PLAYER_X] = '^';
        }
        if PLAYER_X > 0 {
            grid[dragon_row][PLAYER_X - 1] = '<';
        }
        if PLAYER_X + 1 < WIDTH {
            grid[dragon_row][PLAYER_X + 1] = '>';
        }
        if PLAYER_X + 2 < WIDTH {
            grid[dragon_row][PLAYER_X + 2] = '~';
        }

        let jumps_left = MAX_JUMPS.saturating_sub(self.dragon.jumps_used);
        let mut lines = Vec::with_capacity(HEIGHT + 6);
        lines.push("Side-Scrolling Dragon  |  SPACE: jump  Q: quit  R: restart".to_string());
        lines.push(format!(
            "Score: {:>3}  Jumps Left: {}  Speed: {:>4.1}\n",
            self.score,
            jumps_left,
            self.speed()
        ).trim_end().to_string());
        lines.push(format!("+{}+", "-".repeat(WIDTH)));

        for row in grid {
            let mut line = String::with_capacity(WIDTH + 2);
            line.push('|');
            for cell in row {
                line.push(cell);
            }
            line.push('|');
            lines.push(line);
        }

        lines.push(format!("+{}+", "-".repeat(WIDTH)));

        if self.game_over {
            lines.push(String::new());
            lines.push("Game Over! Press R to play again.".to_string());
        }

        lines
    }
}

struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        (self.state >> 32) as u32
    }

    fn next_f32(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }

    fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }

}

struct TerminalGuard {
    stdout: Stdout,
}

impl TerminalGuard {
    fn setup() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        stdout.execute(EnterAlternateScreen)?;
        stdout.execute(Hide)?;
        stdout.execute(Clear(ClearType::All))?;
        Ok(Self { stdout })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.stdout.execute(Show);
        let _ = self.stdout.execute(LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

fn ground_y() -> f32 {
    GROUND_ROW as f32
}

fn obstacle_height_for_score(rng: &mut Lcg, score: u32) -> usize {
    let roll = rng.next_f32();

    match score {
        0..=5 => {
            if roll < 0.45 {
                1
            } else if roll < 0.9 {
                2
            } else {
                3
            }
        }
        6..=14 => {
            if roll < 0.3 {
                1
            } else if roll < 0.7 {
                2
            } else if roll < 0.95 {
                3
            } else {
                4
            }
        }
        _ => {
            if roll < 0.25 {
                1
            } else if roll < 0.6 {
                2
            } else if roll < 0.9 {
                3
            } else {
                4
            }
        }
    }
}

fn next_spawn_interval(rng: &mut Lcg, score: u32, obstacle_height: usize) -> f32 {
    let (base_min, base_max) = if score < 10 { (1.2, 1.9) } else { (1.05, 1.65) };
    let height_padding = match obstacle_height {
        4 => 0.45,
        3 => 0.2,
        _ => 0.0,
    };

    rng.range_f32(base_min + height_padding, base_max + height_padding)
}

fn handle_input(game: &mut Game) -> io::Result<bool> {
    while event::poll(Duration::from_millis(0))? {
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(false),
                KeyCode::Char('r') => game.reset(),
                KeyCode::Char(' ') => game.dragon.jump(),
                _ => {}
            }
        }
    }

    Ok(true)
}

fn draw(stdout: &mut Stdout, frame: &[String]) -> io::Result<()> {
    stdout.queue(MoveTo(0, 0))?;
    stdout.queue(Clear(ClearType::All))?;
    for (row, line) in frame.iter().enumerate() {
        stdout.queue(MoveTo(0, row as u16))?;
        stdout.queue(Print(line))?;
    }
    stdout.flush()?;
    Ok(())
}

fn main() -> io::Result<()> {
    let mut terminal = TerminalGuard::setup()?;
    let mut game = Game::new();

    loop {
        let frame_start = Instant::now();
        if !handle_input(&mut game)? {
            break;
        }

        game.update(FRAME_TIME.as_secs_f32());
        draw(&mut terminal.stdout, &game.render())?;

        let elapsed = frame_start.elapsed();
        if elapsed < FRAME_TIME {
            sleep(FRAME_TIME - elapsed);
        }
    }

    Ok(())
}
