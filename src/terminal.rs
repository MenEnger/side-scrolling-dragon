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

use crate::game::Game;

/// CLI 版の更新テンポを決めるフレーム時間。
const FRAME_TIME: Duration = Duration::from_millis(50);

/// ターミナルの描画モードを設定し、終了時に元へ戻すガード。
struct TerminalGuard {
    stdout: Stdout,
}

impl TerminalGuard {
    /// 生入力モードと alternate screen を有効化して描画準備を行う。
    fn setup() -> io::Result<Self> {
        // alternate screen に入ると、ゲーム終了後に元のターミナル表示へ戻せる。
        enable_raw_mode()?;
        let mut stdout = stdout();
        stdout.execute(EnterAlternateScreen)?;
        stdout.execute(Hide)?;
        stdout.execute(Clear(ClearType::All))?;
        Ok(Self { stdout })
    }
}

impl Drop for TerminalGuard {
    /// ゲーム終了時にターミナル設定を必ず元へ戻す。
    fn drop(&mut self) {
        let _ = self.stdout.execute(Show);
        let _ = self.stdout.execute(LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

/// 入力イベントを読み取り、終了・再スタート・ジャンプを処理する。
fn handle_input(game: &mut Game) -> io::Result<bool> {
    // 非ブロッキングで入力を吸い出し、ゲームループが止まらないようにする。
    while event::poll(Duration::from_millis(0))? {
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(false),
                KeyCode::Char('r') => game.reset(),
                KeyCode::Char(' ') => game.jump(),
                _ => {}
            }
        }
    }

    Ok(true)
}

/// 画面全体を現在フレームの内容で描き直す。
fn draw(stdout: &mut Stdout, frame: &[String]) -> io::Result<()> {
    // 行ごとに座標指定して描くと、端末ごとの改行差異で崩れにくい。
    stdout.queue(MoveTo(0, 0))?;
    stdout.queue(Clear(ClearType::All))?;
    for (row, line) in frame.iter().enumerate() {
        stdout.queue(MoveTo(0, row as u16))?;
        stdout.queue(Print(line))?;
    }
    stdout.flush()?;
    Ok(())
}

/// ゲームループを起動し、入力・更新・描画を一定間隔で繰り返す。
pub fn run() -> io::Result<()> {
    let mut terminal = TerminalGuard::setup()?;
    let mut game = Game::new();

    loop {
        // 1 フレームごとの経過時間を測り、一定のテンポで更新する。
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
