use std::error::Error;
use std::sync::mpsc::{channel, sync_channel};
use std::sync::{Arc, Mutex, PoisonError};

use core_foundation::base::OSStatus;
use std::thread;
use std::time::Duration;

use core_foundation::runloop::CFRunLoop;
use coremidi::{Client, Notification, Sources};
use log::{error, info};

static SANITY_CHECK_ERROR: &str = "MIDIClientCreate was called before receive_device_updates. \
    This often occurs when using midir::MidiInput::new, which prematurely sets the thread \
    for receiving MIDI device notifications.";

static VIRTUAL_DEVICE_NAME: &str = "device-detection-virtual-device";
static CLIENT_DEVICE_NAME: &str = "device-detection-client";

static CHANNEL_PANIC_MESSAGE: &str = "unable to communicate between notifier and main thread";

pub(crate) type Callback = Arc<Mutex<Option<Box<dyn Fn() + 'static + Send + Sync>>>>;

pub(crate) fn osstatus_error(osstatus: OSStatus) -> String {
    format!("OSStatus {osstatus}")
}

pub(crate) fn create_notification_client(callback: Callback) -> Result<Client, OSStatus> {
    Client::new_with_notifications(CLIENT_DEVICE_NAME, {
        move |_: &Notification| {
            info!("Received device update notification");

            if let Some(f) = callback.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
                f();
            } else {
                error!("No callback set");
            }
        }
    })
}

pub(crate) fn start_notification_loop(
    return_client: bool,
) -> Result<(Callback, Option<Client>), Box<dyn Error + Send + Sync + 'static>> {
    let (send_new_device_notification, receive_new_device_notification) = channel();
    let sanity_check_callback: Box<dyn Fn() + Send + Sync + 'static> =
        Box::new(move || send_new_device_notification.send(()).unwrap());

    let notification_callback = Arc::new(Mutex::new(Some(sanity_check_callback)));

    let (send_notifier_ready, receive_notifier_ready) = sync_channel(0);

    thread::spawn({
        let notification_callback = notification_callback.clone();
        move || {
            let client = match create_notification_client(notification_callback.clone()) {
                Ok(client) => client,
                Err(err) => {
                    send_notifier_ready.send(Err(err)).expect(CHANNEL_PANIC_MESSAGE);
                    return;
                }
            };

            let (send_get_new_devices_ready, receive_get_new_devices_ready) = sync_channel(0);
            send_notifier_ready
                .send(Ok((client, send_get_new_devices_ready)))
                .expect(CHANNEL_PANIC_MESSAGE);

            // note: client must not be dropped, or we won't receive notifications
            let _client = receive_get_new_devices_ready.recv().expect(CHANNEL_PANIC_MESSAGE);
            info!("starting CFRunLoop::run_current in device notifier thread");
            CFRunLoop::run_current();
            error!("CFRunLoop::run_current() returned, which is possibly a bug. The loop should never stop.");

            // this resets the sender as empty, closing the channel assuming only the client held
            // a reference. this helps detecting initialization issues
            *notification_callback.lock().unwrap_or_else(PoisonError::into_inner) = None;
        }
    });

    let (client, send_get_new_devices_ready) = receive_notifier_ready.recv()?.map_err(osstatus_error)?;

    let _virtual_source = client.virtual_source(VIRTUAL_DEVICE_NAME).map_err(osstatus_error)?;

    let (send_to_thread, return_value) = if return_client {
        (None, Some(client))
    } else {
        (Some(client), None)
    };

    send_get_new_devices_ready.send(send_to_thread)?;

    // we should get a notification shortly
    receive_new_device_notification
        .recv_timeout(Duration::from_secs(1))
        .map_err(|_| SANITY_CHECK_ERROR)?;

    if Sources
        .into_iter()
        .any(|s| s.name().map(|s| s == VIRTUAL_DEVICE_NAME).unwrap_or_default())
    {
        Ok((notification_callback, return_value))
    } else {
        Err(SANITY_CHECK_ERROR.into())
    }
}
