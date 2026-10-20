use jarvis_telemetry::Collector;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
struct Telemetry(Arc<Mutex<Snapshot>>);
struct Snapshot {
    data: Option<Value>,
    updated: Instant,
    failed: bool,
}

#[tauri::command]
fn metrics(state: tauri::State<'_, Telemetry>) -> Result<Value, String> {
    let snapshot = state
        .0
        .lock()
        .map_err(|_| "Coleta temporariamente indisponível")?;
    let mut data = snapshot.data.clone().ok_or("Coleta inicial em andamento")?;
    data["sample_age_seconds"] = serde_json::json!(snapshot.updated.elapsed().as_secs_f64());
    data["collection_error"] = serde_json::json!(snapshot.failed);
    Ok(data)
}

fn interval_arg() -> f64 {
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--interval") {
        if let Some(n) = args
            .get(pos + 1)
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|n| n.is_finite() && (1.0..=60.0).contains(n))
        {
            return n;
        }
        eprintln!("--interval deve ser um número entre 1 e 60 segundos.");
        std::process::exit(2);
    }
    2.0
}

fn main() {
    let interval = interval_arg();
    let mut collector = Collector::new(interval);
    if std::env::args().any(|a| a == "--snapshot") {
        std::thread::sleep(Duration::from_millis(250));
        println!("{}", collector.collect());
        return;
    }
    let state = Telemetry(Arc::new(Mutex::new(Snapshot {
        data: None,
        updated: Instant::now(),
        failed: false,
    })));
    let sampler = state.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(250));
        loop {
            let started = Instant::now();
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| collector.collect()));
            if let Ok(mut snapshot) = sampler.0.lock() {
                match result {
                    Ok(data) => {
                        snapshot.data = Some(data);
                        snapshot.updated = Instant::now();
                        snapshot.failed = false;
                    }
                    Err(_) => snapshot.failed = true,
                }
            }
            std::thread::sleep(
                Duration::from_secs_f64(interval)
                    .saturating_sub(started.elapsed())
                    .max(Duration::from_millis(100)),
            );
        }
    });
    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![metrics])
        .setup(|app| {
            tauri::WebviewWindowBuilder::from_config(app, &app.config().app.windows[0])?
                .on_navigation(|url| {
                    url.scheme() == "tauri"
                        || (url.scheme() == "http" && url.host_str() == Some("tauri.localhost"))
                })
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Não foi possível abrir a janela do monitor");
}
