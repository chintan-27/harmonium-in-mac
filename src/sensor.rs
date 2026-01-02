use std::sync::mpsc::Sender;
use std::time::Instant;

use futures_util::StreamExt;

#[derive(Debug, Clone)]
pub struct SensorSample {
    pub theta_deg: f32,
    pub source: String,
    pub t: Instant,
}

#[derive(Debug, Clone)]
pub enum SensorMsg {
    Sample(SensorSample),
    Status(String),
    Error(String),
}

pub fn spawn_sensor_thread(hz: f32, tx: Sender<SensorMsg>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                let _ = tx.send(SensorMsg::Error(format!("Failed to start Tokio runtime: {e}")));
                return;
            }
        };

        rt.block_on(async move {
            // Clone the sender so we can still use `tx` if run_sensor_loop fails.
            let tx_for_loop = tx.clone();

            if let Err(e) = run_sensor_loop(hz, tx_for_loop).await {
                let _ = tx.send(SensorMsg::Error(format!("Sensor loop stopped: {e:?}")));
            }
        });
    })
}

async fn run_sensor_loop(hz: f32, tx: Sender<SensorMsg>) -> booklid_rust::Result<()> {
    let dev = booklid_rust::open(hz).await?;
    let info = dev.info();
    let _ = tx.send(SensorMsg::Status(format!("Connected. device_source={:?}", info.source)));

    let mut stream = dev.subscribe();

    while let Some(s) = stream.next().await {
        let msg = SensorMsg::Sample(SensorSample {
            theta_deg: s.angle_deg,
            source: format!("{:?}", s.source),
            t: Instant::now(),
        });

        if tx.send(msg).is_err() {
            break;
        }
    }

    Ok(())
}
