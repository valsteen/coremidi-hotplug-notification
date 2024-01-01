mod runloop;

use crate::runloop::{start_notification_loop, Callback};
use coremidi::Client;
use once_cell::sync::OnceCell;
use std::error::Error;
use std::sync::PoisonError;

pub(crate) static DEVICE_UPDATE_TX: OnceCell<Callback> = OnceCell::new();

fn handle_device_updates<T: Fn() + Send + Sync + 'static>(
    device_update: T,
    get_client: bool,
) -> Result<Option<Client>, Box<dyn Error + Send + Sync + 'static>> {
    let mut client = None;
    let mut current_device_update_tx = DEVICE_UPDATE_TX
        .get_or_try_init(|| {
            let (callback, maybe_client) = start_notification_loop(get_client)?;
            client = maybe_client;
            Ok::<_, Box<dyn Error + Send + Sync + 'static>>(callback)
        })?
        .lock()
        .unwrap_or_else(PoisonError::into_inner);

    *current_device_update_tx = Some(Box::new(device_update));

    Ok(client)
}

/// This function is used to receive device updates notifications. Be aware that the state of this function is a
/// global static, calling it a second time will replace the previous callback.
///
/// # Arguments
///
/// * `device_update`: The closure function that will be called when new devices are detected. Note that upon device
/// changes, the callback may be called several times within milliseconds.
///
/// # Errors
/// A sanity check is performed before returning, and will fail if the eventloop fails to receive device notifications.
/// This can happen if `MIDIClientCreate` was called before `receive_device_updates`,  which sets the thread for receiving
/// such notification from the OS. This would mean `midir::MidiInput::new` or similar would have been called before
/// calling `receive_device_updates`.
pub fn receive_device_updates<T: Fn() + Send + Sync + 'static>(
    device_update: T,
) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    handle_device_updates(device_update, false)?;
    Ok(())
}

/// This variant also receives the coremidi client created to receive notifications. Important: do not drop the client
/// if doing so. Dropping it will also disable notifications and the ability to refresh.
///
/// # Errors
/// same as `receive_device_updates`, may additionally fail if any of `get_client_and_receive_device_updates` or
/// `receive_device_updates` was already called, because the client can only be obtained the first time the loop
/// is run.
pub fn get_client_and_receive_device_updates<T: Fn() + Send + Sync + 'static>(
    device_update: T,
) -> Result<Client, Box<dyn Error + Send + Sync + 'static>> {
    handle_device_updates(device_update, true)?.ok_or_else(|| "Client was already initialized".into())
}
