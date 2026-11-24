const { invoke } = window.__TAURI__.core;

const $ = (id) => document.getElementById(id);
const screens = ["start", "question", "results"];
const keys = { 1: "ja", 2: "teilweise", 3: "nein", 4: "weiss_nicht" };
let busy = false;
let answered = 0;

function show(name) {
  for (const s of screens) $(s).hidden = s !== name;
}

function renderQuestion(step) {
  answered = step.answered;
  $("q-area").textContent = step.area;
  $("q-meta").textContent =
    `Bereich ${step.area_index + 1}/${step.area_count} · Phase ${step.phase} · Frage ${step.condition_index + 1}/${step.condition_count}`;
  $("q-bar").style.width = `${(step.condition_index / step.condition_count) * 100}%`;
  $("q-title").textContent = step.title;
  $("q-example").textContent = step.example;
  $("q-example").hidden = !step.example;
  show("question");
}

const NS = "http://www.w3.org/2000/svg";
const SHORT = {
  "Körperbewusstsein & -pflege": "Körper",
  "Umgebungsbewusstsein": "Umgebung",
  "Sozial-emotionale Entwicklung": "Sozial-emotional",
  "Spieltätigkeit": "Spiel",
  "Sprache & Literacy": "Sprache",
  "Kognition": "Kognition",
  "Grobmotorik": "Grobmotorik",
  "Feinmotorik": "Feinmotorik",
};

const fmt = (v) => (Number.isInteger(v) ? String(v) : v.toFixed(2).replace(".", ","));

function svgEl(tag, attrs, parent, text) {
  const e = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  if (text !== undefined) e.textContent = text;
  parent.append(e);
  return e;
}

function drawChart(results, median) {
  const svg = $("r-chart");
  svg.replaceChildren();
  const W = Math.max(300, Math.round(svg.parentElement.clientWidth - 32));
  const narrow = W < 520;
  const H = Math.round(Math.min(420, Math.max(300, W * 0.62))) + (narrow ? 20 : 0);
  const L = 34, R = 10, T = 12, B = narrow ? 92 : 80;
  svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
  const pw = W - L - R, ph = H - T - B;
  const x = (i) => L + ((i + 0.5) * pw) / results.length;
  const y = (v) => T + ph - (v / 18) * ph;

  for (let v = 0; v <= 18; v += 2) {
    svgEl("line", { x1: L, x2: W - R, y1: y(v), y2: y(v), class: "grid" }, svg);
    svgEl("text", { x: L - 8, y: y(v) + 4, class: "axis", "text-anchor": "end" }, svg, v);
  }
  results.forEach((r, i) => {
    const lx = x(i), ly = H - B + 18;
    svgEl("text", { x: lx, y: ly, class: "axis", "text-anchor": "end", transform: `rotate(${narrow ? -50 : -35} ${lx} ${ly})` }, svg, SHORT[r.area] ?? r.area);
  });

  svgEl("line", { x1: L, x2: W - R, y1: y(median), y2: y(median), class: "median" }, svg);

  for (const key of ["ceiling", "floor", "total"]) {
    const points = results.map((r, i) => `${x(i)},${y(r[key])}`).join(" ");
    svgEl("polyline", { points, class: `line ${key}` }, svg);
    results.forEach((r, i) => {
      const dot = svgEl("circle", { cx: x(i), cy: y(r[key]), r: key === "total" ? 5.5 : 4, class: `dot ${key}` }, svg);
      svgEl("title", {}, dot, `${r.area}: ${fmt(r[key])}`);
    });
  }
  $("r-median").textContent = fmt(median);
}

function td(text) {
  const cell = document.createElement("td");
  cell.textContent = text;
  return cell;
}

function fillTable(results) {
  const body = $("r-table").querySelector("tbody");
  body.replaceChildren();
  for (const r of results) {
    const tr = document.createElement("tr");
    const th = document.createElement("th");
    th.textContent = r.area;
    const values = r.phases.map((p) => `${p.phase}: ${p.value === null ? "–" : fmt(p.value)}`).join(" · ");
    tr.append(
      th,
      td(r.table.ja.join(", ") || "–"),
      td(r.table.teilweise.join(", ") || "–"),
      td(r.table.nein.join(", ") || "–"),
      td(r.table.weiss_nicht.join(", ") || "–"),
      td(values),
      td(fmt(r.floor)),
      td(fmt(r.ceiling)),
      td(fmt(r.total)),
    );
    body.append(tr);
  }
}

let lastResult = null;

window.addEventListener("resize", () => {
  if (lastResult && !$("results").hidden) drawChart(lastResult.results, lastResult.median);
});

function renderResults(step) {
  lastResult = step;
  $("r-saved").hidden = true;
  $("r-age").textContent = `Alter: ${step.age_months} Monate`;
  show("results");
  drawChart(step.results, step.median);
  fillTable(step.results);
}

async function call(cmd, args) {
  if (busy) return;
  busy = true;
  $("error").hidden = true;
  try {
    const step = await invoke(cmd, args);
    if (step.kind === "question") renderQuestion(step);
    else renderResults(step);
  } catch (e) {
    $("error").textContent = String(e);
    $("error").hidden = false;
  } finally {
    busy = false;
  }
}

$("age-form").addEventListener("submit", (e) => {
  e.preventDefault();
  const age = parseInt($("age").value, 10);
  call("start_assessment", { ageMonths: age });
});

for (const b of document.querySelectorAll("[data-answer]")) {
  b.addEventListener("click", () => call("answer", { answer: b.dataset.answer }));
}

function goBack() {
  if (!$("question").hidden && answered === 0) {
    show("start");
    refreshResume();
    $("age").focus();
    return;
  }
  call("go_back", {});
}

$("r-pdf").addEventListener("click", async () => {
  $("r-pdf").disabled = true;
  $("error").hidden = true;
  try {
    const path = await invoke("export_pdf");
    $("r-saved").textContent = `Gespeichert: ${path}`;
    $("r-saved").hidden = false;
  } catch (e) {
    $("error").textContent = String(e);
    $("error").hidden = false;
  } finally {
    $("r-pdf").disabled = false;
  }
});

let toastTimer;

function toast(message) {
  $("toast").textContent = message;
  $("toast").hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => ($("toast").hidden = true), 4500);
}

function formatStamp(stamp) {
  const m = /^(\d{4})-(\d{2})-(\d{2}) (\d{2}:\d{2})$/.exec(stamp ?? "");
  return m ? `${m[3]}.${m[2]}.${m[1]}, ${m[4]} Uhr` : stamp ?? "";
}

async function refreshResume() {
  try {
    const info = await invoke("autosave_info");
    if (!info || info.answered === 0) {
      $("resume").hidden = true;
      return;
    }
    const status = info.finished ? "abgeschlossen" : `${info.answered} Antworten`;
    $("resume-info").textContent = `${info.age_months} Monate · ${status} · ${formatStamp(info.saved_at)}`;
    $("resume").hidden = false;
  } catch {
    $("resume").hidden = true;
  }
}

async function saveSession() {
  $("error").hidden = true;
  try {
    const path = await invoke("save_session");
    toast(`Gespeichert: ${path}`);
  } catch (e) {
    $("error").textContent = String(e);
    $("error").hidden = false;
  }
}

$("save").addEventListener("click", saveSession);
$("r-save").addEventListener("click", saveSession);
$("resume-btn").addEventListener("click", () => call("resume_autosave", {}));
$("open-btn").addEventListener("click", () => $("open-file").click());

$("open-file").addEventListener("change", async () => {
  const file = $("open-file").files[0];
  $("open-file").value = "";
  if (!file) return;
  try {
    const content = await file.text();
    await call("load_session", { content });
  } catch (e) {
    $("error").textContent = String(e);
    $("error").hidden = false;
  }
});

$("back").addEventListener("click", goBack);
$("r-back").addEventListener("click", goBack);

document.addEventListener("keydown", (e) => {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s" && $("start").hidden) {
    e.preventDefault();
    saveSession();
    return;
  }
  if ($("question").hidden) return;
  if (keys[e.key]) call("answer", { answer: keys[e.key] });
  if (e.key === "Backspace") {
    e.preventDefault();
    goBack();
  }
});

$("restart").addEventListener("click", () => {
  $("age").value = "";
  show("start");
  refreshResume();
  $("age").focus();
});

refreshResume();
