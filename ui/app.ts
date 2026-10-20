'use strict';
const $ = (id: string): any => document.getElementById(id);
const exists = n => typeof n === 'number' && Number.isFinite(n);
const pct = n => exists(n) ? (n > 0 && n < 1 ? '<1%' : `${n.toFixed(0)}%`) : 'N/D';
const bytes = n => {
  if (!exists(n)) return 'Indisponível';
  if (n >= 1024 ** 3) return `${(n / 1024 ** 3).toFixed(1)} GiB`;
  return `${(n / 1024 ** 2).toFixed(n < 1024 ** 2 ? 1 : 0)} MiB`;
};
const temperature = n => exists(n) ? `${n.toFixed(0)} °C` : 'Indisponível';
const el = (tag: string, className?: string, text?: any): any => {
  const n = document.createElement(tag);
  if (className) n.className = className;
  if (text !== undefined) n.textContent = text;
  return n;
};
const setText = (id, value) => { $(id).textContent = value; };
const svg = (tag: string, attributes: Record<string, any>): any => {
  const n = document.createElementNS('http://www.w3.org/2000/svg', tag);
  Object.entries(attributes).forEach(([k, v]) => n.setAttribute(k, String(v)));
  return n;
};

function gauge(label: string, value: number | null | undefined, detail = '') {
  const container = el('div', 'gauge');
  container.setAttribute('role', 'img');
  container.setAttribute('aria-label', `${label}: ${exists(value) ? pct(value) : 'indisponível'}. ${detail}`);
  const drawing = svg('svg', { viewBox: '0 0 120 106', 'aria-hidden': 'true' });
  drawing.append(svg('path', { d: 'M 24 85 A 43 43 0 1 1 96 85', class: 'track', pathLength: 100 }));
  if (exists(value)) drawing.append(svg('path', { d: 'M 24 85 A 43 43 0 1 1 96 85', class: 'fill', pathLength: 100, 'stroke-dasharray': `${Math.min(100, Math.max(0, value))} 100` }));
  const labelText = svg('text', { x: 60, y: 60, 'text-anchor': 'middle', class: exists(value) ? '' : 'missing' });
  labelText.textContent = pct(value);
  drawing.append(labelText);
  container.append(drawing, el('div', 'gauge-label', label), el('div', 'gauge-detail', detail));
  return container;
}

function bar(title: string, used: number | null, total: number | null, percent: number | null, description = '') {
  const group = el('div', 'bar-group');
  const heading = el('div', 'bar-title');
  heading.append(el('span', '', title), el('span', '', exists(total) ? `${bytes(used)} / ${bytes(total)}` : 'Indisponível'));
  const track = el('div', 'bar');
  track.setAttribute('role', 'img');
  track.setAttribute('aria-label', `${title}: ${exists(percent) ? pct(percent) : 'indisponível'}`);
  if (exists(percent)) {
    const fill = el('div', `bar-fill${percent >= 90 ? ' warning' : ''}`);
    fill.style.width = `${Math.min(100, Math.max(0, percent))}%`;
    track.append(fill);
  }
  group.append(heading, track, el('div', 'bar-description', description || (exists(percent) ? `${pct(percent)} utilizado` : 'Indisponível')));
  return group;
}

function showPage(page) {
  const hardware = page === 'hardware';
  $('overview').hidden = hardware;
  $('hardware').hidden = !hardware;
  setText('page-number', hardware ? '02' : '01');
  setText('page-title', hardware ? 'Hardware' : 'Visão geral');
  setText('page-subtitle', hardware ? 'Recursos, sensores e processos desta máquina.' : 'Uma visão em tempo real do seu sistema.');
  document.querySelectorAll<HTMLAnchorElement>('[data-page]').forEach(a => {
    const active = a.dataset.page === page;
    a.classList.toggle('active', active);
    if (active) a.setAttribute('aria-current', 'page'); else a.removeAttribute('aria-current');
  });
  document.title = `J.A.R.V.I.S. · ${hardware ? 'Hardware' : 'Visão geral'}`;
}
document.querySelectorAll<HTMLAnchorElement>('[data-page]').forEach(a => a.addEventListener('click', event => {
  if (event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
  event.preventDefault();
  history.pushState({}, '', a.getAttribute('href'));
  showPage(a.dataset.page);
}));
window.addEventListener('hashchange', () => showPage(location.hash === '#/hardware' ? 'hardware' : 'overview'));
showPage(location.hash === '#/hardware' ? 'hardware' : 'overview');

let motionPreference = false;
try { motionPreference = localStorage.getItem('jarvis-reduced-motion') === 'true'; } catch {}
$('motion').checked = motionPreference || window.matchMedia('(prefers-reduced-motion: reduce)').matches;
document.body.classList.toggle('reduced-motion', $('motion').checked);
$('motion').addEventListener('change', () => {
  document.body.classList.toggle('reduced-motion', $('motion').checked);
  try { localStorage.setItem('jarvis-reduced-motion', String($('motion').checked)); } catch {}
});

function tick() {
  const now = new Date();
  const clock = now.toLocaleTimeString('pt-BR', { hour12: false });
  setText('header-clock', clock);
  setText('main-clock', clock.slice(0, 5));
  setText('clock-seconds', `${clock.slice(-2)} SEGUNDOS`);
  setText('main-date', now.toLocaleDateString('pt-BR', { weekday: 'long', day: 'numeric', month: 'long' }));
}
tick();
setInterval(tick, 1000);

function diagnosticNodes(d) {
  const nodes = [];
  if (!d.alerts.length) nodes.push(el('div', 'notice', 'Sem alertas pelos limites desta amostra.'));
  d.alerts.forEach(text => nodes.push(el('div', 'notice warning', text)));
  d.limitations.forEach(text => nodes.push(el('div', 'notice muted', text)));
  nodes.push(el('p', 'footnote', 'Leitura parcial de sensores; não certifica a saúde do hardware.'));
  return nodes;
}

function render(data) {
  const cpuTempReadings = data.temperatures.items.filter(t => /coretemp|k10temp|zenpower|cpu/i.test(`${t.sensor} ${t.label}`));
  const cpuTemp = cpuTempReadings.length ? Math.max(...cpuTempReadings.map(t => t.current)) : null;
  const cpuDetail = temperature(cpuTemp);
  $('overview-cpu').replaceChildren(gauge('CPU', data.cpu.percent, cpuDetail), gauge('RAM', data.memory.percent, `${bytes(data.memory.used)} / ${bytes(data.memory.total)}`));
  setText('cpu-model', data.cpu.model);
  const cpuMeta = `${data.cpu.physical_cores ?? 'N/D'} núcleos · ${data.cpu.logical_cores ?? 'N/D'} threads${exists(data.cpu.frequency_mhz) ? ` · ${(data.cpu.frequency_mhz / 1000).toFixed(2)} GHz` : ''}`;
  setText('cpu-meta', cpuMeta);
  $('hardware-cpu-gauge').replaceChildren(gauge('CPU', data.cpu.percent, cpuDetail));
  setText('hardware-cpu-model', data.cpu.model);
  setText('hardware-cpu-meta', `${cpuMeta}. Barras: carga por thread.`);
  $('cores').replaceChildren(...data.cpu.per_core.map((value, i) => {
    const core = el('div', 'core');
    core.title = `Thread ${i + 1}: ${pct(value)}`;
    const fill = el('div', 'core-fill');
    fill.style.height = exists(value) ? `${Math.min(100, Math.max(0, value))}%` : '0%';
    if (!exists(value)) core.setAttribute('aria-label', `Thread ${i + 1}: indisponível`);
    core.append(fill, el('span', '', String(i + 1)));
    return core;
  }));
  const gpu = data.gpu.devices[0];
  $('overview-gpu').replaceChildren(gauge('GPU', gpu?.utilization, temperature(gpu?.temperature)), gauge('VRAM', gpu?.memory_percent, gpu ? `${bytes(gpu.memory_used)} / ${bytes(gpu.memory_total)}` : 'Indisponível'));
  setText('gpu-model', gpu?.name || 'Telemetria NVIDIA indisponível');
  setText('gpu-meta', gpu ? `Sensor NVIDIA${data.gpu.devices.length > 1 ? ' · Primeira GPU; demais em Hardware' : ' · Memória dedicada'}` : data.gpu.reason);
  const gpuHardware = [];
  data.gpu.devices.forEach((device, index) => {
    const group = el('div', 'gpu-device');
    group.append(el('p', 'model', `${index + 1}. ${device.name}`));
    const gauges = el('div', 'gauges');
    gauges.append(gauge('GPU', device.utilization, temperature(device.temperature)), gauge('VRAM', device.memory_percent, `${bytes(device.memory_used)} / ${bytes(device.memory_total)}`));
    group.append(gauges);
    gpuHardware.push(group);
  });
  if (!gpuHardware.length) gpuHardware.push(gauge('GPU', null), gauge('VRAM', null));
  $('hardware-gpu-gauges').replaceChildren(...gpuHardware);
  setText('hardware-gpu-model', gpu ? `${data.gpu.devices.length} GPU NVIDIA detectada(s)` : 'Indisponível');
  setText('hardware-gpu-details', gpu ? 'GPU e VRAM medidas via nvidia-smi.' : data.gpu.reason);
  const stateLabels = { active: 'ATIVO', stopped: 'PARADO', failed: 'FALHOU', not_installed: 'NÃO INSTALADO*', unknown: 'DESCONHECIDO' };
  $('services-list').replaceChildren(...data.services.map(s => {
    const row = el('div', 'service');
    row.append(el('i', `dot ${s.state}`), el('span', 'service-name', s.name), el('span', `state ${s.state}`, stateLabels[s.state] || 'DESCONHECIDO'), el('span', 'service-source', s.source));
    return row;
  }));
  setText('services-count', `${data.services.length} CONFIGURADOS`);
  setText('services-note', `Consulta há ${data.services_age_seconds.toFixed(0)} s · a cada ${Math.max(10, data.interval_seconds)} s. *Não encontrado nos nomes e unidades configurados. Não testa saúde da aplicação.`);
  $('diagnostic-summary').replaceChildren(...diagnosticNodes(data.diagnostics));
  $('hardware-diagnostic-summary').replaceChildren(...diagnosticNodes(data.diagnostics));
  setText('diagnostic-rules', data.diagnostics.rules);
  setText('hardware-diagnostic-rules', data.diagnostics.rules);
  $('memory-bars').replaceChildren(bar('RAM', data.memory.used, data.memory.total, data.memory.percent), bar('Disponível', data.memory.available, data.memory.total, data.memory.available / data.memory.total * 100, 'Memória livre e cache recuperável'), bar('Swap', data.swap.used, data.swap.total, data.swap.percent, data.swap.status === 'not_configured' ? 'Swap não configurada' : (data.swap.status === 'unavailable' ? data.swap.reason : `${pct(data.swap.percent)} utilizado`)));
  $('disk-bars').replaceChildren(...(data.disks.items.length ? data.disks.items.map(d => bar(d.volume, d.used, d.total, d.percent, d.reason || `${d.device} · ${d.filesystem} · ${bytes(d.free)} livres`)) : [el('p', 'muted', 'Volumes indisponíveis.')]));
  $('process-table').replaceChildren(...data.processes.items.map(p => {
    const row = el('tr');
    [p.name, p.pid, bytes(p.rss), exists(p.memory_percent) ? `${p.memory_percent.toFixed(1)}%` : 'N/D'].forEach(value => row.append(el('td', '', value)));
    return row;
  }));
  if (!data.processes.items.length) {
    const row = el('tr'); const cell = el('td', '', 'Processos indisponíveis nesta amostra.');
    cell.colSpan = 4; row.append(cell); $('process-table').append(row);
  }
  setText('process-note', `RSS pode incluir páginas compartilhadas. ${data.processes.inaccessible_or_exited} processo(s) sem acesso ou encerrado(s) durante a leitura. Sem linhas de comando.`);
  const sensorRows = data.temperatures.items.map(t => ({ label: t.label, sensor: t.sensor, value: t.current }));
  data.gpu.devices.forEach(g => { if (exists(g.temperature)) sensorRows.push({ label: g.name, sensor: 'NVIDIA', value: g.temperature }); });
  $('temperature-list').replaceChildren(...(sensorRows.length ? sensorRows.map(s => {
    const row = el('div', 'temperature-row');
    const name = el('span', '', s.label);
    name.append(el('small', '', s.sensor));
    row.append(name, el('strong', s.value >= 85 ? 'hot' : '', temperature(s.value)));
    return row;
  }) : [el('p', 'muted', data.temperatures.reason || 'Sensores indisponíveis.')]));
  const rootDisk = data.disks.items.find(d => d.volume === '/') || data.disks.items[0];
  setText('quick-disk', rootDisk ? `${pct(rootDisk.percent)} · ${bytes(rootDisk.free)} livres` : 'Indisponível');
  setText('quick-temp', sensorRows.length ? temperature(Math.max(...sensorRows.map(s => s.value))) : 'Indisponível');
  setText('quick-process', data.processes.status === 'ok' ? String(data.processes.observed) : 'Indisponível');
  setText('quick-cost', `${data.collection_ms.toFixed(0)} ms`);
  const hours = Math.floor(data.uptime_seconds / 3600);
  setText('uptime', exists(data.uptime_seconds) ? `LIGADO HÁ ${Math.floor(hours / 24)}D ${hours % 24}H ${Math.floor(data.uptime_seconds / 60) % 60}MIN` : 'UPTIME INDISPONÍVEL');
  setText('footer-system', data.system.toUpperCase());
  setText('cadence', `Coleta: ${data.interval_seconds} s`);
  setText('last-sample', `AMOSTRA ${String(data.sequence).padStart(4, '0')} · ${new Date(data.sampled_at).toLocaleTimeString('pt-BR')} · ${data.collection_ms.toFixed(0)} ms`);
}

let interval = 2000;
let timer;
let inflight = false;
let lastReceived = 0;
let lastAge = 0;
let collectionError = false;
let hasData = false;
function markConnection(reason) {
  const stale = Boolean(reason);
  document.body.classList.toggle('stale', stale && hasData);
  $('connection').classList.toggle('warning', stale);
  $('connection').querySelector('span').textContent = stale ? 'COLETA INDISPONÍVEL' : 'TELEMETRIA CONECTADA';
  $('error-banner').hidden = !stale;
  $('error-banner').textContent = stale ? `${reason} ${hasData ? 'Os valores exibidos são da última amostra recebida.' : 'Aguardando dados reais.'}` : '';
  document.querySelector<HTMLElement>('.observing').textContent = stale ? 'AGUARDANDO TELEMETRIA' : 'MONITORAMENTO ATIVO';
}
async function poll() {
  clearTimeout(timer);
  if (inflight) return;
  if (document.hidden) { timer = setTimeout(poll, interval); return; }
  inflight = true;
  try {
    const data = await readMetrics();
    interval = Math.max(1000, data.interval_seconds * 1000);
    lastReceived = Date.now();
    lastAge = data.sample_age_seconds;
    collectionError = data.collection_error;
    hasData = true;
    render(data);
    markConnection(collectionError || lastAge > Math.max(5, interval / 1000 * 3) ? 'A coleta está atrasada ou falhou.' : '');
  } catch {
    markConnection('O coletor interno ainda não respondeu.');
  } finally {
    inflight = false;
    timer = setTimeout(poll, interval);
  }
}
setInterval(() => {
  if (lastReceived && (Date.now() - lastReceived) / 1000 + lastAge > Math.max(5, interval / 1000 * 3)) markConnection('Dados antigos: a coleta não está atualizando.');
}, 1000);
document.addEventListener('visibilitychange', () => { if (!document.hidden) poll(); });
poll();

// Única operação IPC: ler uma cópia da última amostra. Sem rede ou shell no frontend.
async function readMetrics(): Promise<any> {
  const invoke = (window as any).__TAURI__?.core?.invoke;
  if (!invoke) throw new Error('Aplicativo desktop necessário');
  let timeout: ReturnType<typeof setTimeout>;
  try {
    return await Promise.race([invoke('metrics'), new Promise((_, reject) => {
      timeout = setTimeout(() => reject(new Error('Coleta indisponível')), 4000);
    })]);
  } finally { clearTimeout(timeout); }
}
export {};
