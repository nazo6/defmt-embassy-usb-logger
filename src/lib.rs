#![allow(static_mut_refs)]

mod task;

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex, signal::Signal};
pub use task::start_logger;

static mut RESTORE: critical_section::RestoreState = critical_section::RestoreState::invalid();
static mut TAKEN: bool = false;
static mut ENCODER: defmt::Encoder = defmt::Encoder::new();

static QUEUE: Mutex<CriticalSectionRawMutex, heapless::Vec<u8, 1024>> =
    Mutex::new(heapless::Vec::new());
static LOG_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// The logger implementation.
#[defmt::global_logger]
struct USBLogger;

unsafe impl defmt::Logger for USBLogger {
    fn acquire() {
        unsafe {
            let restore = critical_section::acquire();
            if TAKEN {
                defmt::error!("defmt logger taken reentrantly");
                defmt::panic!();
            }
            TAKEN = true;
            RESTORE = restore;
            ENCODER.start_frame(inner);
        }
    }

    unsafe fn release() {
        unsafe {
            ENCODER.end_frame(inner);
            TAKEN = false;
            let restore = RESTORE;
            critical_section::release(restore);
        }
    }

    unsafe fn flush() {}

    unsafe fn write(bytes: &[u8]) {
        unsafe {
            ENCODER.write(bytes, inner);
        }
    }
}

fn inner(bytes: &[u8]) {
    let Ok(mut q) = QUEUE.try_lock() else {
        return;
    };
    if q.extend_from_slice(bytes).is_err() {
        return;
    }

    LOG_SIGNAL.signal(());
}
