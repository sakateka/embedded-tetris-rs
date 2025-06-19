use std::io::{self, Read};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tetris_lib::common::GameController;

// Store original terminal state for restoration
static mut ORIGINAL_TERMIOS: Option<libc::termios> = None;

// Platform-specific raw terminal setup
#[cfg(unix)]
pub fn enable_raw_mode() {
    use std::os::unix::io::AsRawFd;
    unsafe {
        let fd = io::stdin().as_raw_fd();
        let mut termios: libc::termios = std::mem::zeroed();
        libc::tcgetattr(fd, &mut termios);

        // Save original settings
        ORIGINAL_TERMIOS = Some(termios);

        // Set raw mode (like tty.setcbreak in Python)
        termios.c_lflag &= !(libc::ICANON | libc::ECHO);
        termios.c_cc[libc::VMIN] = 0;
        termios.c_cc[libc::VTIME] = 0;

        libc::tcsetattr(fd, libc::TCSANOW, &termios);

        // Also set stdin to non-blocking mode
        let flags = libc::fcntl(fd, libc::F_GETFL);
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
}

#[cfg(unix)]
pub fn restore_terminal() {
    use std::os::unix::io::AsRawFd;
    unsafe {
        if let Some(original) = ORIGINAL_TERMIOS {
            let fd = io::stdin().as_raw_fd();
            libc::tcsetattr(fd, libc::TCSANOW, &original);

            // Also restore blocking mode
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK);
        }
    }
}

#[cfg(not(unix))]
pub fn enable_raw_mode() {
    // No-op for non-Unix systems
}

#[cfg(not(unix))]
pub fn restore_terminal() {
    // No-op for non-Unix systems
}

// Key input events
#[derive(Debug, Clone, PartialEq)]
enum KeyEvent {
    Left,
    Right,
    Up,
    Down,
    Space,
    Enter,
    Quit,
    None,
}

pub struct SimpleConsoleController {
    current_key: Arc<Mutex<KeyEvent>>,
    _input_thread: std::thread::JoinHandle<()>,
}

impl SimpleConsoleController {
    pub fn new() -> Self {
        let current_key = Arc::new(Mutex::new(KeyEvent::None));
        let current_key_clone = current_key.clone();

        // Input processing thread (like machine.py)
        let input_thread = std::thread::spawn(move || {
            loop {
                let key = Self::read_key();
                if key != KeyEvent::None {
                    {
                        let mut current = current_key_clone.lock().unwrap();
                        *current = key.clone();
                    }

                    if key == KeyEvent::Quit {
                        restore_terminal();
                        println!("\nTerminal restored. Goodbye!");
                        std::process::exit(0);
                    }
                }

                std::thread::sleep(Duration::from_millis(10)); // Faster polling
            }
        });

        Self {
            current_key,
            _input_thread: input_thread,
        }
    }

    fn read_key() -> KeyEvent {
        let mut buffer = [0; 1];
        let mut stdin = io::stdin();

        // Try to read one character (non-blocking with raw mode)
        match stdin.read(&mut buffer) {
            Ok(1) => {
                let ch = buffer[0];
                match ch {
                    27 => {
                        let mut seq = [0; 2];
                        if stdin.read(&mut seq).unwrap_or(0) == 2 {
                            match seq {
                                [91, 65] => KeyEvent::Up,    // [A
                                [91, 66] => KeyEvent::Down,  // [B
                                [91, 67] => KeyEvent::Right, // [C
                                [91, 68] => KeyEvent::Left,  // [D
                                _ => KeyEvent::None,
                            }
                        } else {
                            KeyEvent::None
                        }
                    }
                    b' ' => KeyEvent::Space,
                    b'\n' | b'\r' => KeyEvent::Enter,
                    b'a' | b'A' => KeyEvent::Left,
                    b'd' | b'D' => KeyEvent::Right,
                    b'w' | b'W' => KeyEvent::Up,
                    b's' | b'S' => KeyEvent::Down,
                    b'q' | b'Q' => KeyEvent::Quit,
                    _ => KeyEvent::None,
                }
            }
            _ => KeyEvent::None,
        }
    }
}

impl GameController for SimpleConsoleController {
    async fn read_x(&mut self) -> i8 {
        let mut key_guard = self.current_key.lock().unwrap();
        let key = key_guard.clone();

        match key {
            KeyEvent::Left => {
                *key_guard = KeyEvent::None; // Only clear if we consume it
                -1
            }
            KeyEvent::Right => {
                *key_guard = KeyEvent::None; // Only clear if we consume it
                1
            }
            _ => 0, // Don't clear other keys - let read_y() or was_pressed() handle them
        }
    }

    async fn read_y(&mut self) -> i8 {
        let mut key_guard = self.current_key.lock().unwrap();
        let key = key_guard.clone();

        match key {
            KeyEvent::Up => {
                *key_guard = KeyEvent::None; // Only clear if we consume it
                -1
            }
            KeyEvent::Down => {
                *key_guard = KeyEvent::None; // Only clear if we consume it
                1
            }
            _ => 0, // Don't clear other keys - let read_x() or was_pressed() handle them
        }
    }

    fn was_pressed(&self) -> bool {
        let mut key_guard = self.current_key.lock().unwrap();
        let key = key_guard.clone();

        match key {
            KeyEvent::Space | KeyEvent::Enter => {
                *key_guard = KeyEvent::None; // Clear the key since we consumed it
                true
            }
            _ => false, // No button press detected
        }
    }
}
