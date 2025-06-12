use embassy_rp::gpio::{Input, Pull};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::signal::Signal;

static BUTTON_SIGNAL: Signal<CriticalSectionRawMutex, bool> = Signal::new();
static BUTTON_STATE: Mutex<CriticalSectionRawMutex, bool> = Mutex::new(false);

pub struct ButtonController {
    button: Input<'static>,
}

impl ButtonController {
    pub fn new(button: Input<'static>) -> Self {
        Self { button }
    }

    pub async fn run(&mut self) -> ! {
        let mut last_state = false;
        loop {
            let current_state = self.button.is_low();
            if current_state && !last_state {
                // Button pressed (falling edge)
                BUTTON_SIGNAL.signal(true);
            }
            last_state = current_state;
            embassy_time::Timer::after_millis(10).await;
        }
    }
}

pub async fn wait_for_button_press() -> bool {
    BUTTON_SIGNAL.wait().await
}

pub fn button_was_pressed() -> bool {
    BUTTON_SIGNAL.try_take().unwrap_or(false)
}
