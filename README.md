# coremidi-hotplug-notification

This crate is a simple helper providing a way to receive device updates notifications. It also allows users of crates such as midir to refresh the list of devices without having to restart the program, the notification is optional.

## Prerequisites

```toml
[target.'cfg(target_os = "macos")'.dependencies]
coremidi-hotplug-notification = "0.1.1"
```

## Usage

```rust
fn main() {
    #[cfg(target_os = "macos")]
    // Register for device update notifications
    coremidi_hotplug_notification::receive_device_updates(|| {
        println!("A MIDI device was connected or disconnected.");
        // Insert your handling logic here
    }).expect("Failed to register for MIDI device updates");
}
```

In practice, you'll most likely want to use a channel to receive updates. The closure does not receive any parameter, just re-read the list of devices from there.

If you are not interested in notifications and just intend to refresh manually, you can pass an empty closure ( `|| ()` ).

## Caveats

MacOS will set the thread on which notifications are sent at the first call to create a coremidi client. Due to that, `receive_device_updates` will fail if you use any MIDI functionality before calling it.

This crate will spawn a thread dedicated to the runloop, which is necessary in order to receive device updates. If you are not willing to run a dedicated thread, you'll probably want to directly use the coremidi crate and call `CFRunLoop::run_current` or `run_in_mode` yourself.