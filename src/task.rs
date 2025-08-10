//! Main task that runs the USB transport layer.

use embassy_usb::{
    Builder, Config,
    class::cdc_acm::Sender,
    driver::{Driver, EndpointError},
};

use super::{LOG_SIGNAL, QUEUE};

macro_rules! singleton {
    ($val:expr, $type:ty) => {{
        static STATIC_CELL: ::static_cell::StaticCell<$type> = ::static_cell::StaticCell::new();
        STATIC_CELL.init($val)
    }};
}

/// Runs the logger task with a default USB CDC ACM driver.
///
/// If you need to use USB for purposes other than logging, use logger_task_custom_sender.
pub async fn logger_task<D: Driver<'static>>(driver: D, vid: u16, pid: u16) {
    let config = Config::new(vid, pid);

    let mut builder = Builder::new(
        driver,
        config,
        singleton!([0; 256], [u8; 256]),
        singleton!([0; 256], [u8; 256]),
        singleton!([0; 256], [u8; 256]),
        singleton!([0; 64], [u8; 64]),
    );

    let defmt_usb = embassy_usb::class::cdc_acm::CdcAcmClass::new(
        &mut builder,
        singleton!(
            embassy_usb::class::cdc_acm::State::new(),
            embassy_usb::class::cdc_acm::State
        ),
        64,
    );
    let (sender, _) = defmt_usb.split();

    logger_task_custom_sender(sender, 64, true).await;
}

/// Runs the logger task with user-provided cdc acm sender.
pub async fn logger_task_custom_sender<'d, D: Driver<'d>>(
    mut sender: Sender<'d, D>,
    chunk_size: usize,
    use_dtr: bool,
) {
    sender.wait_connection().await;

    loop {
        if use_dtr {
            loop {
                if sender.dtr() {
                    break;
                }
                embassy_time::Timer::after_millis(500).await;
            }
        }

        LOG_SIGNAL.wait().await;
        embassy_time::Timer::after_millis(100).await;
        LOG_SIGNAL.reset();

        let q = core::mem::take(&mut *QUEUE.lock().await);

        for chunk in q.chunks(chunk_size) {
            if let Err(EndpointError::Disabled) = sender.write_packet(chunk).await {
                sender.wait_connection().await;
            }
        }
    }
}
