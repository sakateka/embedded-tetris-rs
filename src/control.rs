use core::cell::RefCell;

use cortex_m::interrupt::{free as interrupt_free, Mutex};
use microbit::{
    hal::{
        gpio::{Input, Floating, Pin},
        gpiote::Gpiote,
    },
    pac::{self, interrupt},
};
use rtt_target::rprint;

pub static GPIO: Mutex<RefCell<Option<Gpiote>>> = Mutex::new(RefCell::new(None));
pub static PUSH: Mutex<RefCell<bool>> = Mutex::new(RefCell::new(false));
pub static TICK: Mutex<RefCell<u32>> = Mutex::new(RefCell::new(0));


#[pac::interrupt]
fn GPIOTE() {
    static mut LAST_TICK: u32 = 0;

    interrupt_free(|cs| {
        if let Some(gpiote) = GPIO.borrow(cs).borrow().as_ref() {
            let curr_tick = *TICK.borrow(cs).borrow();
            if curr_tick - *LAST_TICK > 2 {
                rprint!("chnage {} {}\n", curr_tick, *LAST_TICK);
                let push = gpiote.channel0().is_event_triggered();
                *PUSH.borrow(cs).borrow_mut() = push;
            } else {
                rprint!("skip {} {}\n", curr_tick, *LAST_TICK);
            }
            *LAST_TICK = curr_tick;
            gpiote.channel0().reset_events();
        }
    });
}

pub fn init_button(board_gpiote: pac::GPIOTE, pin: Pin<Input<Floating>>) {
    let gpiote = Gpiote::new(board_gpiote);
    let channel = gpiote.channel0();
    channel
        .input_pin(&pin)
        .hi_to_lo()
        .enable_interrupt();
    channel.reset_events();
    interrupt_free(move |cs| {
        *GPIO.borrow(cs).borrow_mut() = Some(gpiote);
        unsafe {
            pac::NVIC::unmask(pac::Interrupt::GPIOTE);
        }
        pac::NVIC::unpend(pac::Interrupt::GPIOTE);
    });
}

pub fn button_was_pressed(reset: bool) -> bool {
    interrupt_free(|cs| {
        let push = *PUSH.borrow(cs).borrow();
        if reset {
            *PUSH.borrow(cs).borrow_mut() = false;
        }
        *TICK.borrow(cs).borrow_mut() += 1;
        push
    })
}
