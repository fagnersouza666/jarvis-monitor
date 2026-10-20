//! Read-only Linux host telemetry. Process inspection uses only comm and statm.
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::fs;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

type UnitFields = HashMap<String, String>;
type Units = HashMap<String, UnitFields>;
const MAX_OUTPUT: usize = 128 * 1024;
const DEFAULT_SERVICES: &str = include_str!("../../services.json");

/// Owns CPU warm-up counters and the ten-second systemd observation cache.
/// Construction and collection never change host configuration or service state.
pub struct Collector {
    interval: f64,
    sequence: u64,
    previous_cpu: String,
    cpu_model: String,
    logical_cores: Option<usize>,
    physical_cores: Option<usize>,
    specs: Vec<Value>,
    scopes: [Option<Units>; 2],
    service_checked: Option<Instant>,
    proc_root: PathBuf,
    sys_root: PathBuf,
}

impl Collector {
    pub fn new(interval: f64) -> Self {
        Self::with_roots(interval, PathBuf::from("/proc"), PathBuf::from("/sys"))
    }

    fn with_roots(interval: f64, proc_root: PathBuf, sys_root: PathBuf) -> Self {
        let info = fs::read_to_string(proc_root.join("cpuinfo")).unwrap_or_default();
        let previous_cpu = fs::read_to_string(proc_root.join("stat")).unwrap_or_default();
        let (cpu_model, logical_cores, physical_cores) = cpu_info(&info);
        Self {
            interval: if interval.is_finite() {
                interval.clamp(1.0, 60.0)
            } else {
                2.0
            },
            sequence: 0,
            previous_cpu,
            cpu_model,
            logical_cores,
            physical_cores,
            specs: serde_json::from_str(DEFAULT_SERVICES).unwrap_or_default(),
            scopes: [None, None],
            service_checked: None,
            proc_root,
            sys_root,
        }
    }

    pub fn interval(&self) -> f64 {
        self.interval
    }

    pub fn collect(&mut self) -> Value {
        let start = Instant::now();
        self.sequence = self.sequence.saturating_add(1);
        let current_cpu = fs::read_to_string(self.proc_root.join("stat")).unwrap_or_default();
        let mut cpu = cpu_percent(&self.previous_cpu, &current_cpu);
        self.previous_cpu = current_cpu;
        cpu["model"] = json!(self.cpu_model);
        cpu["logical_cores"] = json!(self.logical_cores);
        cpu["physical_cores"] = json!(self.physical_cores);
        cpu["frequency_mhz"] = json!(fs::read_to_string(self.proc_root.join("cpuinfo"))
            .ok()
            .and_then(|s| cpu_frequency(&s)));
        let (memory, swap) =
            memory_metrics(&fs::read_to_string(self.proc_root.join("meminfo")).unwrap_or_default());
        let (processes, names) = process_metrics(&self.proc_root, memory["total"].as_u64());
        if self.service_checked.is_none_or(|last| {
            start.duration_since(last).as_secs_f64() >= 10.0_f64.max(self.interval)
        }) {
            let units: HashSet<String> = self
                .specs
                .iter()
                .flat_map(|s| strings(s, "units"))
                .collect();
            let mut units: Vec<_> = units.into_iter().collect();
            units.sort();
            self.scopes = [systemd_units(&units, false), systemd_units(&units, true)];
            self.service_checked = Some(Instant::now());
        }
        let services: Vec<_> = self
            .specs
            .iter()
            .map(|s| {
                let probes: Vec<_> = strings(s, "executables")
                    .iter()
                    .map(|name| executable_present(name))
                    .collect();
                let executable = if probes.contains(&Some(true)) {
                    Some(true)
                } else if probes.contains(&None) {
                    None
                } else {
                    Some(false)
                };
                resolve_service(s, &self.scopes, &names, executable)
            })
            .collect();
        let uptime = fs::read_to_string(self.proc_root.join("uptime"))
            .ok()
            .and_then(|s| s.split_whitespace().next().and_then(number));
        let system = if fs::read_to_string("/etc/os-release")
            .unwrap_or_default()
            .lines()
            .any(|s| s == "ID=pop" || s == "ID=\"pop\"")
        {
            "Pop!_OS"
        } else {
            "Linux"
        };
        let mut data = json!({
            "sequence":self.sequence, "sampled_at":timestamp(), "interval_seconds":self.interval,
            "uptime_seconds":uptime, "system":system, "cpu":cpu, "memory":memory, "swap":swap,
            "gpu":gpu_metrics(), "temperatures":temperature_metrics(&self.sys_root),
            "disks":disk_metrics(&self.proc_root), "processes":processes, "services":services,
            "services_age_seconds":self.service_checked.map(|t| rounded(t.elapsed().as_secs_f64(),1))
        });
        data["diagnostics"] = diagnostics(&data);
        data["collection_ms"] = json!(rounded(start.elapsed().as_secs_f64() * 1000.0, 1));
        data
    }
}

fn rounded(value: f64, digits: i32) -> f64 {
    let scale = 10_f64.powi(digits);
    (value * scale).round() / scale
}
fn number(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|x| x.is_finite() && *x >= 0.0)
}
fn limited(value: &str, count: usize) -> String {
    value.chars().take(count).collect()
}
fn strings(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .map(|xs| {
            xs.iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn cpu_info(info: &str) -> (String, Option<usize>, Option<usize>) {
    let mut model = None;
    let mut logical = 0;
    let mut physical = HashSet::new();
    for block in info.split("\n\n") {
        let fields: HashMap<_, _> = block
            .lines()
            .filter_map(|line| line.split_once(':').map(|(k, v)| (k.trim(), v.trim())))
            .collect();
        if fields.contains_key("processor") {
            logical += 1;
        }
        if model.is_none() {
            model = fields
                .get("model name")
                .or_else(|| fields.get("Hardware"))
                .map(|s| limited(s, 160));
        }
        if let (Some(socket), Some(core)) = (fields.get("physical id"), fields.get("core id")) {
            physical.insert((socket.to_string(), core.to_string()));
        }
    }
    (
        model.unwrap_or_else(|| "Processador não identificado".into()),
        (logical > 0).then_some(logical),
        (!physical.is_empty()).then_some(physical.len()),
    )
}

fn cpu_frequency(info: &str) -> Option<f64> {
    let values: Vec<_> = info
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(key, _)| key.trim() == "cpu MHz")
        .filter_map(|(_, value)| number(value))
        .collect();
    (!values.is_empty()).then(|| rounded(values.iter().sum::<f64>() / values.len() as f64, 0))
}

fn cpu_ticks(input: &str) -> HashMap<String, Vec<u64>> {
    input
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let name = fields.next()?;
            if name != "cpu"
                && !name
                    .strip_prefix("cpu")
                    .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            {
                return None;
            }
            let values: Option<Vec<u64>> = fields.take(8).map(|s| s.parse().ok()).collect();
            let values = values?;
            (values.len() >= 4).then(|| (name.to_owned(), values))
        })
        .collect()
}

fn usage(before: &[u64], after: &[u64]) -> Option<f64> {
    if before.len() != after.len() {
        return None;
    }
    let deltas: Option<Vec<_>> = before
        .iter()
        .zip(after)
        .map(|(a, b)| b.checked_sub(*a))
        .collect();
    let deltas = deltas?;
    let total = deltas.iter().try_fold(0u64, |a, b| a.checked_add(*b))?;
    let idle = deltas[3].checked_add(*deltas.get(4).unwrap_or(&0))?;
    (total > 0).then(|| rounded((total - idle) as f64 / total as f64 * 100.0, 1))
}

fn cpu_percent(before: &str, after: &str) -> Value {
    let a = cpu_ticks(before);
    let b = cpu_ticks(after);
    let percent = a
        .get("cpu")
        .zip(b.get("cpu"))
        .and_then(|(a, b)| usage(a, b));
    let mut cores: Vec<_> = b
        .keys()
        .filter_map(|k| {
            k.strip_prefix("cpu")
                .and_then(|n| n.parse::<usize>().ok())
                .map(|n| (n, k))
        })
        .collect();
    cores.sort_by_key(|(n, _)| *n);
    let per_core: Vec<_> = cores
        .iter()
        .map(|(_, key)| a.get(*key).zip(b.get(*key)).and_then(|(a, b)| usage(a, b)))
        .collect();
    json!({"percent":percent,"per_core":per_core,"status":if percent.is_some(){"ok"}else{"unavailable"},
        "reason":if percent.is_some(){None}else{Some("CPU sem delta válido; aguardando nova amostra ou acesso a /proc/stat.")}})
}

fn memory_metrics(input: &str) -> (Value, Value) {
    let values: HashMap<&str, u64> = input
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            let mut words = value.split_whitespace();
            let value = words.next()?.parse::<u64>().ok()?;
            (words.next() == Some("kB"))
                .then(|| value.checked_mul(1024).map(|v| (key, v)))
                .flatten()
        })
        .collect();
    let unavailable = |reason: &str| json!({"total":null,"used":null,"available":null,"percent":null,"status":"unavailable","reason":reason});
    let memory = match values.get("MemTotal").zip(values.get("MemAvailable")) {
        Some((&total, &available)) if total > 0 && available <= total => {
            json!({"total":total,"available":available,"used":total-available,"percent":rounded((total-available) as f64/total as f64*100.0,1),"status":"ok","reason":null})
        }
        _ => unavailable("RAM sem leitura válida de MemTotal/MemAvailable."),
    };
    let swap = match values.get("SwapTotal").zip(values.get("SwapFree")) {
        Some((&total, &free)) if free <= total => {
            json!({"total":total,"used":total-free,"percent":if total>0 {Some(rounded((total-free) as f64/total as f64*100.0,1))}else{None},"status":if total>0{"ok"}else{"not_configured"},"reason":null})
        }
        _ => {
            json!({"total":null,"used":null,"percent":null,"status":"unavailable","reason":"Swap sem leitura válida de SwapTotal/SwapFree."})
        }
    };
    (memory, swap)
}

fn csv_row(line: &str) -> Option<Vec<String>> {
    let mut columns = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                columns.push(field.trim().to_owned());
                field.clear();
            }
            _ => field.push(ch),
        }
    }
    if quoted {
        return None;
    }
    columns.push(field.trim().to_owned());
    Some(columns)
}
fn parse_nvidia(input: &str) -> Vec<Value> {
    input.lines().filter_map(|line| {
        let row=csv_row(line)?;
        if row.len()!=5 || row[0].is_empty(){return None;}
        let utilization=number(&row[1]).filter(|n|*n<=100.0);
        let total=number(&row[2]); let used=number(&row[3]);
        Some(json!({"name":limited(&row[0],120),"utilization":utilization,
            "memory_total":total.map(|v|v*1048576.0),"memory_used":used.map(|v|v*1048576.0),
            "memory_percent":total.zip(used).filter(|(t,u)|*t>0.0 && u<=t).map(|(t,u)|rounded(u/t*100.0,1)),
            "temperature":number(&row[4])}))
    }).collect()
}

// Reserve before spawn, so active commands plus pending reapers can never exceed two.
// A child stuck in an uninterruptible kernel wait retains its slot indefinitely.
static COMMAND_BUDGET: AtomicUsize = AtomicUsize::new(0);
const MAX_COMMANDS: usize = 2;

struct CommandSlot {
    budget: &'static AtomicUsize,
    release_on_drop: bool,
}
impl CommandSlot {
    fn acquire(budget: &'static AtomicUsize) -> Option<Self> {
        let mut used = budget.load(Ordering::Acquire);
        loop {
            if used >= MAX_COMMANDS {
                return None;
            }
            match budget.compare_exchange_weak(used, used + 1, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => {
                    return Some(Self {
                        budget,
                        release_on_drop: true,
                    })
                }
                Err(actual) => used = actual,
            }
        }
    }
}
impl Drop for CommandSlot {
    fn drop(&mut self) {
        if self.release_on_drop {
            self.budget.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

/// Only the child created by run_command is signaled. Blocking wait is confined to
/// a detached reaper: collection never joins it or waits for kernel/driver cleanup.
fn reap_child(mut child: Child, mut slot: CommandSlot) {
    let _ = child.kill();
    // On thread creation or wait failure, retain the occupied budget conservatively.
    // This also prevents repeated spawns when the OS cannot provide reaper threads.
    slot.release_on_drop = false;
    let budget = slot.budget;
    let _ = std::thread::Builder::new()
        .name("jarvis-child-reaper".into())
        .spawn(move || {
            if child.wait().is_ok() {
                budget.fetch_sub(1, Ordering::AcqRel);
            }
        });
}

/// Bounded nonblocking reads avoid deadlocks on pipes or descendants retaining stdout.
fn run_command(
    path: &str,
    args: &[String],
    timeout: Duration,
    runtime: Option<&Path>,
) -> Option<(String, bool)> {
    run_command_with_budget(path, args, timeout, runtime, &COMMAND_BUDGET)
}

fn run_command_with_budget(
    path: &str,
    args: &[String],
    timeout: Duration,
    runtime: Option<&Path>,
    budget: &'static AtomicUsize,
) -> Option<(String, bool)> {
    let slot = CommandSlot::acquire(budget)?;
    let mut command = Command::new(path);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("LC_ALL", "C");
    if let Some(runtime) = runtime {
        command.env("XDG_RUNTIME_DIR", runtime);
    }
    let mut child = command.spawn().ok()?;
    let Some(mut pipe) = child.stdout.take() else {
        reap_child(child, slot);
        return None;
    };
    let fd = pipe.as_raw_fd();
    // SAFETY: fd belongs to our ChildStdout and remains open for this scope.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        reap_child(child, slot);
        return None;
    }
    let start = Instant::now();
    let mut output = Vec::new();
    let mut status = None;
    loop {
        let mut chunk = [0u8; 8192];
        match pipe.read(&mut chunk) {
            Ok(0) => {
                if let Some(status) = status {
                    return String::from_utf8(output).ok().map(|s| (s, status));
                }
            }
            Ok(n) => {
                if output.len() + n > MAX_OUTPUT {
                    reap_child(child, slot);
                    return None;
                }
                output.extend_from_slice(&chunk[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => {
                reap_child(child, slot);
                return None;
            }
        }
        match child.try_wait() {
            Ok(Some(result)) => status = Some(result.success()),
            Ok(None) => {}
            Err(_) => {
                reap_child(child, slot);
                return None;
            }
        }
        if start.elapsed() >= timeout {
            if status.is_none() {
                reap_child(child, slot);
            }
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn gpu_metrics() -> Value {
    let args = vec![
        "--query-gpu=name,utilization.gpu,memory.total,memory.used,temperature.gpu".into(),
        "--format=csv,noheader,nounits".into(),
    ];
    let result = run_command(
        "/usr/bin/nvidia-smi",
        &args,
        Duration::from_millis(1200),
        None,
    );
    let devices = result
        .filter(|(_, success)| *success)
        .map(|(output, _)| parse_nvidia(&output))
        .unwrap_or_default();
    let available = !devices.is_empty();
    json!({"status":if available{"ok"}else{"unavailable"},"devices":devices,"reason":if available{None}else{Some("NVIDIA sem leitura; dispositivo, driver, permissão ou nvidia-smi indisponível, ou consulta expirou.")}})
}

fn parse_units(input: &str) -> Units {
    let mut units = HashMap::new();
    for block in input.split("\n\n") {
        let fields: UnitFields = block
            .lines()
            .filter_map(|line| {
                line.split_once('=')
                    .map(|(k, v)| (k.to_owned(), v.to_owned()))
            })
            .collect();
        if let (Some(id), Some(_)) = (fields.get("Id"), fields.get("LoadState")) {
            units.insert(id.clone(), fields.clone());
            if let Some(names) = fields.get("Names") {
                for name in names.split_whitespace() {
                    units.insert(name.into(), fields.clone());
                }
            }
        }
    }
    units
}
fn systemd_units(units: &[String], user: bool) -> Option<Units> {
    if units.is_empty() {
        return Some(HashMap::new());
    }
    let mut args = Vec::new();
    if user {
        args.push("--user".into());
    }
    args.push("show".into());
    args.extend_from_slice(units);
    args.extend([
        "--property=Id,Names,LoadState,ActiveState,SubState".into(),
        "--no-pager".into(),
    ]);
    // SAFETY: getuid has no memory preconditions and does not change credentials.
    let uid = unsafe { libc::getuid() };
    let runtime = PathBuf::from(format!("/run/user/{uid}"));
    let runtime = if user
        && runtime
            .metadata()
            .is_ok_and(|m| m.is_dir() && m.uid() == uid)
    {
        Some(runtime.as_path())
    } else {
        None
    };
    let (output, _) = run_command(
        "/usr/bin/systemctl",
        &args,
        Duration::from_millis(1500),
        runtime,
    )?;
    let entries = parse_units(&output);
    (!entries.is_empty()).then_some(entries)
}
fn executable_present(name: &str) -> Option<bool> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.@:+-".contains(&c))
    {
        return None;
    }
    // Inspect executable metadata only; never collect PATH or executable contents.
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    dirs.extend(["/usr/bin", "/usr/local/bin", "/bin", "/usr/sbin", "/sbin"].map(PathBuf::from));
    let mut uncertain = false;
    for dir in dirs {
        match fs::metadata(dir.join(name)) {
            Ok(m) if m.is_file() && m.permissions().mode() & 0o111 != 0 => return Some(true),
            Ok(_) => {}
            Err(e)
                if [
                    std::io::ErrorKind::NotFound,
                    std::io::ErrorKind::NotADirectory,
                ]
                .contains(&e.kind()) => {}
            Err(_) => uncertain = true,
        }
    }
    if uncertain {
        None
    } else {
        Some(false)
    }
}
fn resolve_service(
    spec: &Value,
    scopes: &[Option<Units>; 2],
    names: &HashSet<String>,
    executable: Option<bool>,
) -> Value {
    let result =
        |state: &str, source: String| json!({"name":spec["name"],"state":state,"source":source});
    if strings(spec, "process_names")
        .iter()
        .any(|n| names.contains(n))
    {
        return result(
            "active",
            "Processo com nome exato observado; não verifica saúde da aplicação.".into(),
        );
    }
    let units = strings(spec, "units");
    let mut installed = Vec::new();
    for (scope, entries) in ["system", "user"].into_iter().zip(scopes) {
        if let Some(entries) = entries {
            for unit in &units {
                if let Some(fields) = entries.get(unit) {
                    let active = fields
                        .get("ActiveState")
                        .map(String::as_str)
                        .unwrap_or("unknown");
                    let loaded = fields.get("LoadState").is_some_and(|s| s != "not-found");
                    if loaded && ["active", "reloading"].contains(&active) {
                        return result(
                            "active",
                            format!("systemd {scope}: {unit}; não verifica saúde da aplicação."),
                        );
                    }
                    if loaded {
                        installed.push((scope, unit, active));
                    }
                }
            }
        }
    }
    // Failed evidence wins over an unrelated stopped alternate unit.
    if let Some((scope, unit, active)) = installed
        .iter()
        .find(|(_, _, a)| *a == "failed")
        .or_else(|| installed.first())
    {
        let state = match *active {
            "failed" => "failed",
            "inactive" => "stopped",
            _ => "unknown",
        };
        return result(state, format!("systemd {scope}: {unit} ({active})"));
    }
    if executable == Some(true) {
        return result(
            "unknown",
            "Executável encontrado, mas nenhuma unidade ou processo confirmado.".into(),
        );
    }
    if executable.is_none() {
        return result(
            "unknown",
            "Sem acesso suficiente aos executáveis configurados.".into(),
        );
    }
    if !units.is_empty()
        && scopes.iter().all(|s| {
            s.as_ref().is_some_and(|entries| {
                units.iter().all(|u| {
                    entries
                        .get(u)
                        .and_then(|f| f.get("LoadState"))
                        .is_some_and(|s| s == "not-found")
                })
            })
        })
    {
        return result(
            "not_installed",
            "Unidades system/user e executáveis configurados não encontrados.".into(),
        );
    }
    result(
        "unknown",
        "Sem evidência suficiente; systemd ou processos podem estar inacessíveis.".into(),
    )
}

fn process_metrics(root: &Path, total_memory: Option<u64>) -> (Value, HashSet<String>) {
    let mut names = HashSet::new();
    let mut items = Vec::new();
    let mut skipped = 0;
    // SAFETY: sysconf simply queries the page-size constant.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse::<u32>().ok())
            else {
                continue;
            };
            let dir = entry.path();
            let Ok(name) = fs::read_to_string(dir.join("comm")) else {
                skipped += 1;
                continue;
            };
            let name = name.trim_end_matches('\n');
            names.insert(name.to_owned());
            let rss = fs::read_to_string(dir.join("statm"))
                .ok()
                .and_then(|s| {
                    s.split_whitespace()
                        .nth(1)
                        .and_then(|n| n.parse::<u64>().ok())
                })
                .and_then(|pages| {
                    u64::try_from(page_size)
                        .ok()
                        .filter(|p| *p > 0)
                        .and_then(|p| pages.checked_mul(p))
                });
            let Some(rss) = rss else {
                skipped += 1;
                continue;
            };
            items.push(json!({"pid":pid,"name":limited(name,80),"rss":rss,"memory_percent":total_memory.filter(|t|*t>0).map(|t|rounded(rss as f64/t as f64*100.0,2))}));
        }
    }
    items.sort_by_key(|p| std::cmp::Reverse(p["rss"].as_u64().unwrap_or(0)));
    let observed = items.len();
    items.truncate(10);
    (
        json!({"status":if observed>0{"ok"}else{"unavailable"},"items":items,"observed":observed,"inaccessible_or_exited":skipped}),
        names,
    )
}

fn mount_unescape(value: &str) -> String {
    value
        .replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
}
fn disk_metrics(root: &Path) -> Value {
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    for line in fs::read_to_string(root.join("self/mounts"))
        .unwrap_or_default()
        .lines()
    {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 3 {
            continue;
        }
        let filesystem = fields[2];
        // Remote mounts and userspace filesystems can block statvfs indefinitely.
        // Skip them before decoding or exposing remote device/mount names, even at /.
        if ["nfs", "nfs4", "cifs", "smb3", "sshfs"].contains(&filesystem)
            || filesystem.starts_with("fuse")
        {
            continue;
        }
        let device = mount_unescape(fields[0]);
        let mount = mount_unescape(fields[1]);
        if [
            "squashfs",
            "iso9660",
            "tmpfs",
            "devtmpfs",
            "proc",
            "sysfs",
            "cgroup",
            "cgroup2",
            "securityfs",
            "debugfs",
            "tracefs",
            "configfs",
            "fusectl",
            "mqueue",
            "pstore",
            "hugetlbfs",
            "autofs",
            "binfmt_misc",
            "devpts",
            "efivarfs",
            "ramfs",
        ]
        .contains(&filesystem)
            || device.starts_with("/dev/loop")
            || !(device.starts_with("/dev/")
                || mount == "/"
                || ["zfs", "btrfs"].contains(&filesystem))
            || !seen.insert(device.clone())
        {
            continue;
        }
        let basename = Path::new(&device)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("volume");
        let label = if ["/", "/home", "/boot", "/boot/efi", "/recovery"].contains(&mount.as_str()) {
            mount.as_str()
        } else {
            basename
        };
        let mut item = json!({"volume":limited(label,80),"device":limited(basename,80),"filesystem":limited(filesystem,40),"total":null,"used":null,"free":null,"percent":null,"reason":"Volume sem acesso."});
        if let Ok(path) = CString::new(mount.as_bytes()) {
            let mut data = std::mem::MaybeUninit::<libc::statvfs>::uninit();
            // SAFETY: path is NUL-terminated and data points to writable statvfs storage.
            if unsafe { libc::statvfs(path.as_ptr(), data.as_mut_ptr()) } == 0 {
                let data = unsafe { data.assume_init() };
                if let (Some(total), Some(free), Some(available)) = (
                    data.f_blocks.checked_mul(data.f_frsize),
                    data.f_bfree.checked_mul(data.f_frsize),
                    data.f_bavail.checked_mul(data.f_frsize),
                ) {
                    if let Some(used) = total.checked_sub(free) {
                        item["total"] = json!(total);
                        item["used"] = json!(used);
                        item["free"] = json!(available);
                        item["percent"] = json!(used
                            .checked_add(available)
                            .filter(|v| *v > 0)
                            .map(|v| rounded(used as f64 / v as f64 * 100.0, 1)));
                        item["reason"] = Value::Null;
                    }
                }
            }
        }
        items.push(item);
    }
    json!({"status":if items.iter().any(|i|i["total"].is_number()){"ok"}else{"unavailable"},"items":items})
}

fn sensor_value(path: &Path) -> Option<f64> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .filter(|n| n.is_finite())
        .map(|n| n / 1000.0)
}
fn temperature_metrics(root: &Path) -> Value {
    let mut items = Vec::new();
    if let Ok(groups) = fs::read_dir(root.join("class/hwmon")) {
        for group in groups.flatten() {
            let dir = group.path();
            let sensor = fs::read_to_string(dir.join("name")).unwrap_or_else(|_| "hwmon".into());
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let file = entry.file_name();
                    let file = file.to_string_lossy();
                    let Some(prefix) = file.strip_suffix("_input").filter(|s| {
                        s.strip_prefix("temp")
                            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                    }) else {
                        continue;
                    };
                    let Some(current) = sensor_value(&entry.path()) else {
                        continue;
                    };
                    let label = fs::read_to_string(dir.join(format!("{prefix}_label")))
                        .unwrap_or_else(|_| sensor.clone());
                    items.push(json!({"sensor":limited(sensor.trim(),80),"label":limited(label.trim(),80),"current":current,
                "high":sensor_value(&dir.join(format!("{prefix}_max"))),"critical":sensor_value(&dir.join(format!("{prefix}_crit")))}));
                }
            }
        }
    }
    json!({"status":if items.is_empty(){"unavailable"}else{"ok"},"reason":if items.is_empty(){Some("Nenhum sensor térmico acessível.")}else{None},"items":items})
}

fn diagnostics(data: &Value) -> Value {
    let mut alerts = Vec::new();
    let mut limitations = Vec::new();
    for (key, label) in [("cpu", "CPU"), ("memory", "RAM")] {
        match data[key]["percent"].as_f64() {
            Some(n) if n >= 90.0 => {
                alerts.push(format!("{label} em {n:.0}% nesta amostra (limiar: 90%)."))
            }
            None => limitations.push(format!("{label} sem leitura.")),
            _ => {}
        }
    }
    if let Some(disks) = data["disks"]["items"].as_array() {
        if disks.is_empty() {
            limitations.push("Sem dados de volumes.".into());
        }
        for disk in disks {
            if let Some(n) = disk["percent"].as_f64().filter(|n| *n >= 90.0) {
                alerts.push(format!(
                    "Volume {} em {n:.0}% (limiar: 90%).",
                    disk["volume"].as_str().unwrap_or("desconhecido")
                ));
            }
            if disk["percent"].is_null() {
                limitations.push("Volume sem leitura de ocupação.".into());
            }
        }
    }
    let temps = data["temperatures"]["items"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let gpus = data["gpu"]["devices"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if let Some(temp) = temps
        .iter()
        .filter_map(|s| s["current"].as_f64())
        .chain(gpus.iter().filter_map(|g| g["temperature"].as_f64()))
        .reduce(f64::max)
        .filter(|n| *n >= 85.0)
    {
        alerts.push(format!(
            "Temperatura observada de {temp:.0}°C (limiar: 85°C)."
        ));
    }
    if temps.is_empty() {
        limitations.push("Sensores do sistema indisponíveis.".into());
    }
    if gpus.is_empty() {
        limitations.push("Telemetria NVIDIA indisponível.".into());
    }
    if data["swap"]["status"] == "unavailable" {
        limitations.push("Swap sem leitura.".into());
    }
    if data["processes"]["status"] == "unavailable" {
        limitations.push("Processos sem leitura.".into());
    }
    if data["services"]
        .as_array()
        .is_some_and(|s| s.iter().any(|s| s["state"] == "unknown"))
    {
        limitations.push("Serviços com estado desconhecido; permissões ou sessão systemd podem limitar a observação.".into());
    }
    if data["uptime_seconds"].is_null() {
        limitations.push("Tempo de atividade sem leitura.".into());
    }
    json!({"alerts":alerts,"limitations":limitations,"rules":"Avisos por amostra: CPU ou RAM ≥90%, volume ≥90%, temperatura ≥85°C. Não testa integridade, SMART, falhas de memória ou saúde dos serviços."})
}

fn timestamp() -> Option<String> {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    let seconds = libc::time_t::try_from(seconds).ok()?;
    let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: seconds and tm are valid pointers; gmtime_r initializes tm on success.
    if unsafe { libc::gmtime_r(&seconds, tm.as_mut_ptr()) }.is_null() {
        return None;
    }
    let tm = unsafe { tm.assume_init() };
    Some(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+00:00",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    ))
}

#[cfg(test)]
mod tests;
