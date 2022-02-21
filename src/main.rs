use std::{collections::HashSet, io, process::exit, time::Duration};
use tokio_i3ipc::{
    event as I3Event,
    event::{Event, Subscribe},
    msg::Msg,
    I3,
};
use tokio_stream::StreamExt;

mod types;
use types::config::{Config, TomlConfig};

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let config: Config = TomlConfig::new()
        .unwrap_or_else(|e| {
            eprintln!("Error reading input: {}", e);
            exit(8)
        })
        .into();
    listener(config).await
}

/// Continuously try to connect to i3 for the duration `time_limit`.
/// `interval` is the time to wait after a failed connection before retrying
/// Returns `Err(..)` if no successful connection after `time_limit`.
async fn try_i3_connection(
    time_limit: Duration,
    interval: Duration,
) -> Result<I3, tokio::time::error::Elapsed> {
    tokio::time::timeout(time_limit, async {
        loop {
            match I3::connect().await {
                Ok(i3) => {
                    return i3;
                }
                Err(_) => {
                    std::thread::sleep(interval);
                }
            }
        }
    })
    .await
}

/// Main listener loop
async fn listener(mut config: Config) -> io::Result<()> {
    // Set up event handlers
    let mut handlers = config.get_handlers();
    let mut subs = HashSet::new();
    for h in handlers.iter() {
        h.add_subscriptions(&mut subs);
    }
    let subs: Vec<Subscribe> = subs.iter().map(|&s| s.into()).collect();

    loop {
        let mut i3 =
            try_i3_connection(config.connection_timeout, config.reconnect_interval).await?;
        let _resp = i3.subscribe(&subs).await?;

        // Need separate tx and rx connections, since sending and receiving on the same connection
        // can cause messages to get missed/jumbled.
        let mut i3_tx = I3::connect().await?;
        let mut i3_rx = I3::connect().await?;

        let mut listener = i3.listen();
        let mut restart = false;
        while let Some(event) = listener.next().await {
            let event = event?;
            if let Event::Shutdown(sd) = &event {
                if sd.change == I3Event::ShutdownChange::Restart {
                    restart = true;
                    println!("Restart detected");
                }
            }
            for handler in handlers.iter_mut() {
                if let Some(msg) = handler.handle_event(&event, &mut i3_rx).await {
                    i3_tx.send_msg_body(Msg::RunCommand, msg).await?;
                }
            }
        }
        if !restart {
            break;
        }
    }
    Ok(())
}
