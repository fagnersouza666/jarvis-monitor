use jarvis_telemetry::Collector;
fn main() {
    let mut collector = Collector::new(2.0);
    std::thread::sleep(std::time::Duration::from_millis(250));
    println!(
        "{}",
        serde_json::to_string_pretty(&collector.collect()).expect("JSON snapshot")
    );
}
