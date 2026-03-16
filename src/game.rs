use std::time::{SystemTime, UNIX_EPOCH};

/// ターミナル上の見た目を固定するため、描画領域は定数で管理する。
const WIDTH: usize = 56;
const HEIGHT: usize = 18;
const GROUND_ROW: usize = HEIGHT - 3;
const PLAYER_X: usize = 8;
const MAX_JUMPS: u8 = 2;
const GRAVITY: f32 = 42.0;
const JUMP_VELOCITY: f32 = -15.0;

/// プレイヤーであるドラゴンの位置とジャンプ状態を管理する。
#[derive(Clone, Copy)]
struct Dragon {
    // y は「行」ではなく物理演算用の連続値として持っておく。
    y: f32,
    velocity_y: f32,
    jumps_used: u8,
}

impl Dragon {
    /// 地面に立っている初期状態のドラゴンを作る。
    fn new() -> Self {
        Self {
            y: ground_y(),
            velocity_y: 0.0,
            jumps_used: 0,
        }
    }

    /// まだジャンプ回数に余裕があれば上向き速度を与える。
    fn jump(&mut self) {
        // 空中でも 2 回目まではジャンプ可能にする。
        if self.jumps_used < MAX_JUMPS {
            self.velocity_y = JUMP_VELOCITY;
            self.jumps_used += 1;
        }
    }

    /// 重力と速度を反映してドラゴンの縦位置を更新する。
    fn update(&mut self, dt: f32) {
        self.velocity_y += GRAVITY * dt;
        self.y += self.velocity_y * dt;

        if self.y >= ground_y() {
            self.y = ground_y();
            self.velocity_y = 0.0;
            self.jumps_used = 0;
        }
    }

    /// 連続値の y 座標を描画用の行番号へ変換する。
    fn row(&self) -> usize {
        self.y.round().clamp(0.0, ground_y()) as usize
    }
}

/// 右から左へ流れてくる柱状の障害物を表す。
#[derive(Clone, Copy, Debug)]
struct Obstacle {
    x: f32,
    height: usize,
    // すでにドラゴンを通過した障害物かどうか。スコアの二重加算を防ぐ。
    passed: bool,
}

impl Obstacle {
    /// 現在のスコア帯に応じた高さで新しい障害物を生成する。
    fn new(rng: &mut Lcg, score: u32) -> Self {
        Self {
            x: (WIDTH - 1) as f32,
            height: obstacle_height_for_score(rng, score),
            passed: false,
        }
    }

    /// 横スクロール速度に応じて障害物を左へ移動させる。
    fn update(&mut self, speed: f32, dt: f32) {
        self.x -= speed * dt;
    }

    /// 浮動小数の x 座標を描画・当たり判定用の列番号へ変換する。
    fn col(&self) -> isize {
        self.x.round() as isize
    }

    /// 画面外まで流れ去ったかどうかを返す。
    fn offscreen(&self) -> bool {
        self.x < -1.0
    }
}

/// ゲーム全体の進行状態をまとめて持つ。
pub struct Game {
    dragon: Dragon,
    obstacles: Vec<Obstacle>,
    // 次の障害物が出るまでの経過時間と目標時間。
    spawn_timer: f32,
    spawn_interval: f32,
    score: u32,
    game_over: bool,
    rng: Lcg,
}

impl Game {
    /// 新しいゲームを初期状態で作る。
    pub fn new() -> Self {
        Self::with_seed(random_seed())
    }

    /// テストや再現用にシードを固定したゲームを作る。
    fn with_seed(seed: u64) -> Self {
        let mut rng = Lcg::new(seed);
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

    /// 現在のゲーム状態を初期状態に戻す。
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// プレイヤー入力としてジャンプを試みる。
    pub fn jump(&mut self) {
        self.dragon.jump();
    }

    /// 現在のスコアから障害物のスクロール速度を計算する。
    fn speed(&self) -> f32 {
        // スコアに応じて少しずつ速度を上げるが、上がり方は緩めにしてある。
        18.0 + self.score as f32 * 0.45
    }

    /// 1 フレーム分のゲーム進行を更新する。
    pub fn update(&mut self, dt: f32) {
        if self.game_over {
            return;
        }

        self.dragon.update(dt);
        self.spawn_timer += dt;

        if self.spawn_timer >= self.spawn_interval {
            self.spawn_timer = 0.0;
            // 出現した障害物の高さに応じて、次の間隔も少し広げる。
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
            // 同じ列に重なり、かつドラゴンの足元が障害物の上端より下なら衝突。
            if obstacle_col == dragon_col && dragon_row >= obstacle_top {
                self.game_over = true;
                break;
            }
        }
    }

    /// 現在のゲーム状態をターミナル表示用の文字列配列へ変換する。
    pub fn render(&self) -> Vec<String> {
        // まずは空の二次元グリッドを作り、あとから地面や障害物を重ねていく。
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
        let sprite_top = dragon_row.saturating_sub(2) as isize;
        let sprite_left = PLAYER_X.saturating_sub(2) as isize;

        // ドラゴンは複数行の AA スプライトとして重ねて描画する。
        blit_sprite(
            &mut grid,
            sprite_top,
            sprite_left,
            dragon_sprite(self.dragon.jumps_used),
        );

        let jumps_left = MAX_JUMPS.saturating_sub(self.dragon.jumps_used);
        let mut lines = Vec::with_capacity(HEIGHT + 6);
        lines.push("Side-Scrolling Dragon  |  SPACE: jump  Q: quit  R: restart".to_string());
        lines.push(format!(
            "Score: {:>3}  Jumps Left: {}  Speed: {:>4.1}",
            self.score,
            jumps_left,
            self.speed()
        ));
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

/// 外部依存を増やさずに使う簡易な疑似乱数生成器。
#[derive(Clone, Copy)]
struct Lcg {
    state: u64,
}

impl Lcg {
    /// 指定したシードで乱数生成器を初期化する。
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// 次の 32bit 疑似乱数を生成する。
    fn next_u32(&mut self) -> u32 {
        // 外部 crate を増やさずに済むよう、簡単な疑似乱数を自前で使う。
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        (self.state >> 32) as u32
    }

    /// 0.0 以上 1.0 以下の範囲の浮動小数乱数を返す。
    fn next_f32(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }

    /// 指定範囲の浮動小数乱数を返す。
    fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}

/// 地面の描画行を浮動小数で返す。
fn ground_y() -> f32 {
    GROUND_ROW as f32
}

/// 実行ごとに少し変化するシード値を生成する。
fn random_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

/// ジャンプ状態に応じたドラゴンの AA スプライトを返す。
fn dragon_sprite(jumps_used: u8) -> &'static [&'static str] {
    match jumps_used {
        0 => &["  /^\\   ", "<(o )==>", " /_|_\\  "],
        1 => &[" _/^^\\_ ", "<(o )~>>", "  / \\   "],
        _ => &["_/^^^^\\_", "<(O )**>", "  / \\   "],
    }
}

/// 文字グリッドの指定位置に、空白を透過扱いでスプライトを描き込む。
fn blit_sprite(grid: &mut [Vec<char>], top: isize, left: isize, sprite: &[&str]) {
    for (dy, row) in sprite.iter().enumerate() {
        let y = top + dy as isize;
        if !(0..grid.len() as isize).contains(&y) {
            continue;
        }

        for (dx, ch) in row.chars().enumerate() {
            let x = left + dx as isize;
            if ch == ' ' || !(0..WIDTH as isize).contains(&x) {
                continue;
            }
            grid[y as usize][x as usize] = ch;
        }
    }
}

/// スコアに応じて障害物の高さ分布を決める。
fn obstacle_height_for_score(rng: &mut Lcg, score: u32) -> usize {
    // 序盤は低め、中盤以降にだけ 4 段を混ぜる。
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

/// 障害物の高さと進行度に応じて次の出現間隔を決める。
fn next_spawn_interval(rng: &mut Lcg, score: u32, obstacle_height: usize) -> f32 {
    let (base_min, base_max) = if score < 10 {
        (1.2, 1.9)
    } else {
        (1.05, 1.65)
    };
    let height_padding = match obstacle_height {
        4 => 0.45,
        3 => 0.2,
        _ => 0.0,
    };

    // 高い障害物の直後は少し猶予を増やして、理不尽さを減らす。
    rng.range_f32(base_min + height_padding, base_max + height_padding)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 3 回目のジャンプ入力は無視され、2 段ジャンプまでに制限される。
    #[test]
    fn dragon_can_only_double_jump() {
        let mut dragon = Dragon::new();

        dragon.jump();
        dragon.jump();
        dragon.jump();

        assert_eq!(dragon.jumps_used, MAX_JUMPS);
        assert_eq!(dragon.velocity_y, JUMP_VELOCITY);
    }

    /// 地面に戻ったらジャンプ回数が回復する。
    #[test]
    fn landing_resets_jump_counter() {
        let mut dragon = Dragon::new();
        dragon.jump();
        dragon.jump();

        for _ in 0..20 {
            dragon.update(0.1);
        }

        assert_eq!(dragon.y, ground_y());
        assert_eq!(dragon.jumps_used, 0);
    }

    /// 障害物を通り過ぎたとき、スコアは一度だけ加算される。
    #[test]
    fn passing_obstacle_scores_once() {
        let mut game = Game::with_seed(1);
        game.obstacles.push(Obstacle {
            x: PLAYER_X as f32 - 0.6,
            height: 1,
            passed: false,
        });

        game.update(0.0);
        game.update(0.0);

        assert_eq!(game.score, 1);
    }

    /// 同じ列で障害物に重なったらゲームオーバーになる。
    #[test]
    fn collision_sets_game_over() {
        let mut game = Game::with_seed(2);
        game.obstacles.push(Obstacle {
            x: PLAYER_X as f32,
            height: 2,
            passed: false,
        });

        game.update(0.0);

        assert!(game.game_over);
    }

    /// 高い障害物の後は次の出現間隔が広めになる。
    #[test]
    fn tall_obstacles_increase_spawn_gap() {
        let mut rng = Lcg::new(3);

        let short_gap = next_spawn_interval(&mut rng, 0, 1);
        let tall_gap = next_spawn_interval(&mut rng, 0, 4);

        assert!((1.2..=1.9).contains(&short_gap));
        assert!((1.65..=2.35).contains(&tall_gap));
    }

    /// 2 段ジャンプ時は羽を広げたリッチなスプライトに切り替わる。
    #[test]
    fn double_jump_uses_richer_sprite() {
        let sprite = dragon_sprite(2);

        assert_eq!(sprite[0], "_/^^^^\\_");
        assert_eq!(sprite[1], "<(O )**>");
    }
}
