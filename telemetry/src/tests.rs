use super::*;

#[test]
fn cpu_delta_excludes_guest_double_count_and_includes_iowait_as_idle() {
    let a = "cpu 100 0 20 200 10 0 0 0 90 0\ncpu0 100 0 20 200 10 0 0 0 90 0\n";
    let b = "cpu 120 0 20 220 20 0 0 0 100 0\ncpu0 120 0 20 220 20 0 0 0 100 0\n";
    let result = cpu_percent(a, b);
    assert_eq!(result["percent"], 40.0);
    assert_eq!(result["per_core"], json!([40.0]));
}

#[test]
fn cpu_reset_missing_and_zero_delta_are_unavailable() {
    for (a, b) in [
        ("", ""),
        ("cpu 9 0 0 9", "cpu 1 0 0 1"),
        ("cpu 1 0 0 1", "cpu 1 0 0 1"),
    ] {
        assert!(cpu_percent(a, b)["percent"].is_null());
    }
}

#[test]
fn memory_uses_available_and_swap_is_independent() {
    let (ram, swap) = memory_metrics(
        "MemTotal: 1000 kB\nMemAvailable: 250 kB\nSwapTotal: 500 kB\nSwapFree: 200 kB\n",
    );
    assert_eq!(ram["used"], 750 * 1024);
    assert_eq!(ram["percent"], 75.0);
    assert_eq!(swap["used"], 300 * 1024);
    assert_eq!(swap["percent"], 60.0);
    let (ram, swap) = memory_metrics("SwapTotal: 0 kB\nSwapFree: 0 kB\n");
    assert_eq!(ram["status"], "unavailable");
    assert_eq!(swap["status"], "not_configured");
    let (ram, swap) =
        memory_metrics("MemTotal: 1000 kB\nMemAvailable: 250 kB\nSwapTotal: broken\n");
    assert_eq!(ram["percent"], 75.0);
    assert!(swap["used"].is_null());
}

#[test]
fn nvidia_csv_quotes_and_unavailable_fields_remain_unknown() {
    let values =
        parse_nvidia("\"NVIDIA, Test\", 25, 1000, 400, N/A\nGPU, N/A, N/A, 12, 60\nbad,row\n");
    assert_eq!(values.len(), 2);
    assert_eq!(values[0]["name"], "NVIDIA, Test");
    assert_eq!(values[0]["memory_percent"], 40.0);
    assert!(values[0]["temperature"].is_null());
    assert!(values[1]["utilization"].is_null());
    assert!(values[1]["memory_total"].is_null());
}

fn service() -> Value {
    json!({"name":"Hermes","units":["hermes.service"],"process_names":["hermes"]})
}

#[test]
fn systemd_alias_names_are_resolved_even_after_nonzero_batch_exit() {
    let entries=parse_units("Id=hermes-gateway.service\nNames=hermes-gateway.service hermes.service\nLoadState=loaded\nActiveState=active\nSubState=running\n");
    let result = resolve_service(
        &service(),
        &[Some(entries), None],
        &HashSet::new(),
        Some(false),
    );
    assert_eq!(result["state"], "active");
}

#[test]
fn services_distinguish_stopped_failed_absent_unknown_and_exact_process_evidence() {
    for (active, expected) in [
        ("inactive", "stopped"),
        ("failed", "failed"),
        ("activating", "unknown"),
    ] {
        let block = format!("Id=hermes.service\nLoadState=loaded\nActiveState={active}\n");
        assert_eq!(
            resolve_service(
                &service(),
                &[Some(parse_units(&block)), None],
                &HashSet::new(),
                Some(false)
            )["state"],
            expected
        );
    }
    let absent = Some(parse_units(
        "Id=hermes.service\nLoadState=not-found\nActiveState=inactive\n",
    ));
    assert_eq!(
        resolve_service(
            &service(),
            &[absent.clone(), absent.clone()],
            &HashSet::new(),
            Some(false)
        )["state"],
        "not_installed"
    );
    assert_eq!(
        resolve_service(
            &service(),
            &[absent.clone(), None],
            &HashSet::new(),
            Some(false)
        )["state"],
        "unknown"
    );
    assert_eq!(
        resolve_service(
            &service(),
            &[absent.clone(), absent],
            &HashSet::new(),
            Some(true)
        )["state"],
        "unknown"
    );
    assert_eq!(
        resolve_service(
            &service(),
            &[None, None],
            &HashSet::from(["hermes-helper".into()]),
            Some(false)
        )["state"],
        "unknown"
    );
    let process = resolve_service(
        &service(),
        &[None, None],
        &HashSet::from(["hermes".into()]),
        Some(false),
    );
    assert_eq!(process["state"], "active");
    assert!(process["source"]
        .as_str()
        .unwrap()
        .contains("não verifica saúde"));
}

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("jarvis-telemetry-test-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, path: &str, content: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn process_scan_reads_only_comm_and_statm_and_keeps_top_ten() {
    let fixture = Fixture::new();
    for pid in 1..=12 {
        fixture.write(&format!("{pid}/comm"), &format!("worker-{pid}\n"));
        fixture.write(&format!("{pid}/statm"), &format!("200 {pid} 0\n"));
    }
    fixture.write("20/comm", "hermes\n"); // exited or unreadable statm: name evidence still exists.
    fixture.write("20/cmdline", "SHOULD_NEVER_APPEAR");
    fixture.write("20/environ", "SHOULD_NEVER_APPEAR");
    let (processes, names) = process_metrics(&fixture.0, Some(1024 * 1024));
    assert_eq!(processes["observed"], 12);
    assert_eq!(processes["items"].as_array().unwrap().len(), 10);
    assert_eq!(processes["items"][0]["pid"], 12);
    assert_eq!(processes["inaccessible_or_exited"], 1);
    assert!(names.contains("hermes"));
    assert!(!processes.to_string().contains("SHOULD_NEVER_APPEAR"));
    assert!(process_metrics(&fixture.0, None).0["items"][0]["memory_percent"].is_null());
}

#[test]
fn disks_omit_loop_and_personal_mount_paths_and_deduplicate_devices() {
    let fixture = Fixture::new();
    let mount = fixture.0.to_str().unwrap();
    fixture.write("self/mounts",&format!("/dev/sda1 {mount} ext4 rw 0 0\n/dev/sda1 /other ext4 rw 0 0\n/dev/loop0 /snap squashfs ro 0 0\ntmpfs /run tmpfs rw 0 0\n/dev/sdb1 /inaccessible/personal-name ext4 rw 0 0\n"));
    let result = disk_metrics(&fixture.0);
    assert_eq!(result["items"].as_array().unwrap().len(), 2);
    assert_eq!(result["items"][0]["volume"], "sda1");
    assert!(result["items"][0]["total"].as_u64().unwrap() > 0);
    assert!(result["items"][1]["total"].is_null());
    assert!(!result.to_string().contains("personal-name"));
    assert!(!result.to_string().contains(mount));
}

#[test]
fn disks_omit_remote_and_fuse_before_statvfs_even_at_root() {
    let fixture = Fixture::new();
    let mount = fixture.0.to_str().unwrap();
    let mut mounts = format!("/dev/sda1 {mount} ext4 rw 0 0\n");
    for filesystem in [
        "nfs",
        "nfs4",
        "cifs",
        "smb3",
        "sshfs",
        "fuse.sshfs",
        "fuse.rclone",
        "fuse",
        "fuseblk",
    ] {
        // Existing destinations would return a real capacity if statvfs were reached.
        mounts.push_str(&format!(
            "personal-server:/private-{filesystem} {mount} {filesystem} rw 0 0\n"
        ));
        mounts.push_str(&format!(
            "personal-root:/private-{filesystem} / {filesystem} rw 0 0\n"
        ));
        mounts.push_str(&format!(
            "/dev/private-{filesystem} {mount} {filesystem} rw 0 0\n"
        ));
    }
    fixture.write("self/mounts", &mounts);
    let result = disk_metrics(&fixture.0);
    assert_eq!(result["items"].as_array().unwrap().len(), 1);
    assert_eq!(result["items"][0]["device"], "sda1");
    assert!(result["items"][0]["total"].as_u64().unwrap() > 0);
    assert!(!result.to_string().contains("personal"));
    assert!(!result.to_string().contains("private"));
}

#[test]
fn temperature_sysfs_reads_labels_and_millidegrees_without_fabricating_limits() {
    let fixture = Fixture::new();
    fixture.write("class/hwmon/hwmon0/name", "coretemp\n");
    fixture.write("class/hwmon/hwmon0/temp1_input", "42500\n");
    fixture.write("class/hwmon/hwmon0/temp1_label", "Package id 0\n");
    fixture.write("class/hwmon/hwmon0/temp1_max", "85000\n");
    fixture.write("class/hwmon/hwmon0/temp2_input", "broken\n");
    let result = temperature_metrics(&fixture.0);
    assert_eq!(result["items"].as_array().unwrap().len(), 1);
    assert_eq!(result["items"][0]["current"], 42.5);
    assert_eq!(result["items"][0]["high"], 85.0);
    assert!(result["items"][0]["critical"].is_null());
}

#[test]
fn missing_cpu_ram_swap_and_uptime_do_not_prevent_other_metrics() {
    let fixture = Fixture::new();
    fixture.write("proc/42/comm", "example\n");
    fixture.write("proc/42/statm", "100 20\n");
    fixture.write("sys/class/hwmon/hwmon0/name", "chip\n");
    fixture.write("sys/class/hwmon/hwmon0/temp1_input", "50000\n");
    let mut collector = Collector::with_roots(2.0, fixture.0.join("proc"), fixture.0.join("sys"));
    collector.service_checked = Some(Instant::now());
    let data = collector.collect();
    assert_eq!(data["sequence"], 1);
    assert!(data["cpu"]["percent"].is_null());
    assert!(data["memory"]["total"].is_null());
    assert_eq!(data["swap"]["status"], "unavailable");
    assert!(data["uptime_seconds"].is_null());
    assert_eq!(data["processes"]["items"][0]["pid"], 42);
    assert_eq!(data["temperatures"]["items"][0]["current"], 50.0);
    assert!(data["diagnostics"]["limitations"].as_array().unwrap().len() >= 4);
    assert_eq!(collector.collect()["sequence"], 2);
}

#[test]
fn diagnostic_thresholds_and_coverage_are_explicit() {
    let data = json!({"cpu":{"percent":90.0},"memory":{"percent":89.9},"disks":{"items":[{"percent":90.0,"volume":"/"}]},
        "temperatures":{"items":[{"current":84.0}]},"gpu":{"devices":[{"temperature":85.0}]},
        "swap":{"status":"ok"},"processes":{"status":"ok"},"services":[],"uptime_seconds":10.0});
    let result = diagnostics(&data);
    assert_eq!(result["alerts"].as_array().unwrap().len(), 3);
    assert!(result["limitations"].as_array().unwrap().is_empty());
    assert!(result["rules"]
        .as_str()
        .unwrap()
        .contains("Não testa integridade"));
}

#[test]
fn subprocess_deadline_and_output_limit_are_enforced_on_owned_children() {
    static BUDGET: AtomicUsize = AtomicUsize::new(0);
    let run = |path, args: &[String], timeout| {
        run_command_with_budget(path, args, timeout, None, &BUDGET)
    };
    let await_reaper = || {
        let deadline = Instant::now() + Duration::from_millis(500);
        while BUDGET.load(Ordering::Acquire) > 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            BUDGET.load(Ordering::Acquire),
            0,
            "owned child was reaped and slot released"
        );
    };
    let start = Instant::now();
    assert!(run("/usr/bin/sleep", &["0.3".into()], Duration::from_millis(30)).is_none());
    assert!(start.elapsed() < Duration::from_millis(250));
    await_reaper();
    assert!(run("/usr/bin/yes", &[], Duration::from_millis(500)).is_none());
    await_reaper();
    let output = run(
        "/usr/bin/printf",
        &["hello".into()],
        Duration::from_millis(500),
    )
    .unwrap();
    assert_eq!(output, ("hello".into(), true));
    assert_eq!(BUDGET.load(Ordering::Acquire), 0);
}

#[test]
fn subprocess_budget_refuses_commands_until_a_slot_is_released() {
    static BUDGET: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let first = CommandSlot::acquire(&BUDGET).expect("first slot");
    let second = CommandSlot::acquire(&BUDGET).expect("second slot");
    assert!(CommandSlot::acquire(&BUDGET).is_none());
    assert!(run_command_with_budget(
        "/usr/bin/printf",
        &["hello".into()],
        Duration::from_millis(500),
        None,
        &BUDGET
    )
    .is_none());
    drop(first);
    assert_eq!(
        run_command_with_budget(
            "/usr/bin/printf",
            &["hello".into()],
            Duration::from_millis(500),
            None,
            &BUDGET
        ),
        Some(("hello".into(), true))
    );
    drop(second);
    assert_eq!(BUDGET.load(std::sync::atomic::Ordering::Acquire), 0);
}

#[test]
fn invalid_intervals_are_bounded_and_utc_timestamp_is_iso8601() {
    let fixture = Fixture::new();
    for (value, expected) in [(0.0, 1.0), (90.0, 60.0), (f64::NAN, 2.0), (2.5, 2.5)] {
        assert_eq!(
            Collector::with_roots(value, fixture.0.clone(), fixture.0.clone()).interval(),
            expected
        );
    }
    let stamp = timestamp().unwrap();
    assert!(stamp.ends_with("+00:00"));
    assert_eq!(stamp.len(), 25);
}
