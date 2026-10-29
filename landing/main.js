document.documentElement.classList.add("js");

const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
const $ = (s, el = document) => el.querySelector(s);
const $$ = (s, el = document) => [...el.querySelectorAll(s)];

// Same rules as the app: "1 234" under 10k, then 12,3k, 1,24M.
function compact(n) {
  const fmt = (v, d) => v.toFixed(d).replace(".", ",").replace(/,0+$/, "");
  if (n < 10_000) return Math.round(n).toLocaleString("fr-FR");
  if (n < 1_000_000) return fmt(n / 1e3, n < 100_000 ? 1 : 0) + "k";
  if (n < 1e9) return fmt(n / 1e6, n < 10e6 ? 2 : 1) + "M";
  return fmt(n / 1e9, 1) + "B";
}

// Seeded sample days so every visit draws the same believable month.
function days(count, seed = 7) {
  let s = seed;
  const rnd = () => ((s = (s * 16807) % 2147483647) / 2147483647);
  return Array.from({ length: count }, (_, i) => {
    const weekend = (i + 3) % 7 >= 5;
    const base = weekend ? 0.25 : 0.7 + rnd() * 0.5;
    return { claude: base * (0.6 + rnd() * 0.4), codex: base * rnd() * 0.45 };
  });
}

function drawChart(el, n) {
  const data = days(n);
  const max = Math.max(...data.map((d) => d.claude + d.codex));
  el.replaceChildren(
    ...data.map((d, i) => {
      const bar = document.createElement("span");
      bar.className = "bar" + (i === n - 1 ? " is-today" : "");
      for (const [key, v] of [["claude", d.claude], ["codex", d.codex]]) {
        const seg = document.createElement("i");
        seg.style.height = `${(v / max) * 100}%`;
        seg.style.setProperty("--d", `${i * 18}ms`);
        seg.dataset.agent = key;
        bar.append(seg);
      }
      return bar;
    }),
  );
}
$$("[data-chart]").forEach((el) => drawChart(el, Number(el.dataset.days || 30)));

// Entrances: each part of a section rises in turn, once.
const io = new IntersectionObserver(
  (entries) => {
    for (const e of entries) {
      if (!e.isIntersecting) continue;
      e.target.classList.add("in");
      io.unobserve(e.target);
    }
  },
  { threshold: 0.15 },
);
$$("section, .foot__mark, .step__tile").forEach((sec) => {
  $$(".reveal", sec).forEach((el, i) => el.style.setProperty("--d", `${i * 80}ms`));
});
$$(".reveal, .hero, .foot__mark, [data-chart]").forEach((el) => io.observe(el));
$$(".hero .stroke").forEach((el, i) => el.style.setProperty("--d", `${200 + i * 140}ms`));

// The clock in the taskbar is the visitor's own.
function tick() {
  const now = new Date();
  $$("[data-clock]").forEach((el) => (el.textContent = now.toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" })));
  $$("[data-date]").forEach((el) => (el.textContent = now.toLocaleDateString("fr-FR")));
}
tick();
setInterval(tick, 15_000);

// Today's count grows the way it does while an agent works: bursts, then pauses.
let today = 1_238_400;
const withoutCache = () => today * 0.106;
let cacheOn = true;
function paint() {
  $$("[data-today]").forEach((el) => (el.textContent = compact(today)));
  $("[data-cache-total]").textContent = compact(today);
  $("[data-cache-value]").textContent = compact(cacheOn ? today : withoutCache());
}
function burst() {
  today += 2_000 + Math.random() * 14_000;
  paint();
  setTimeout(burst, 1_200 + Math.random() * 2_600);
}
if (!reduced) setTimeout(burst, 2_400);

// The tray icon opens and closes the window, like the real one.
const popup = $("#popup");
const tray = $("[data-tray]");
tray.addEventListener("click", () => {
  const open = popup.dataset.open !== "true";
  popup.dataset.open = String(open);
  tray.setAttribute("aria-expanded", String(open));
  tray.classList.remove("pulse");
});
setTimeout(() => {
  if (popup.dataset.open === "true" && !reduced) tray.classList.add("pulse");
}, 5_000);

// The cache switch.
const sw = $("[data-cache-toggle]");
sw.addEventListener("click", () => {
  cacheOn = !cacheOn;
  sw.setAttribute("aria-checked", String(cacheOn));
  sw.closest(".showcase").dataset.cache = cacheOn ? "on" : "off";
  const v = $("[data-cache-value]");
  v.classList.add("fade-swap", "out");
  setTimeout(() => {
    paint();
    v.classList.remove("out");
  }, reduced ? 0 : 160);
});

// Agents index: the hovered or chosen row shows what Tally reads.
$$(".index__row").forEach((row) => {
  row.tabIndex = 0;
  const show = () => {
    $$(".index__row").forEach((r) => r.classList.toggle("is-active", r === row));
    $$(".preview").forEach((p) => p.classList.toggle("is-on", p.dataset.for === row.dataset.preview));
  };
  row.addEventListener("mouseenter", show);
  row.addEventListener("focus", show);
  row.addEventListener("click", show);
});

// Signature move: the window slowly crossfades between its three periods.
const cards = $$(".pcard");
let lit = 0;
setInterval(() => {
  if (reduced || document.hidden) return;
  lit = (lit + 1) % cards.length;
  cards.forEach((c, i) => {
    c.style.transition = "background-color 1.6s";
    c.style.backgroundColor = i === lit ? "var(--panel-3)" : "";
  });
}, 5_500);

// GitHub stars; stays hidden if the API is unreachable.
fetch("https://api.github.com/repos/erwancodes/tally")
  .then((r) => (r.ok ? r.json() : null))
  .then((repo) => {
    if (!repo || typeof repo.stargazers_count !== "number") return;
    $$("[data-stars]").forEach((el) => {
      el.textContent = compact(repo.stargazers_count);
      el.hidden = false;
    });
  })
  .catch(() => {});

$("[data-top]").addEventListener("click", (e) => {
  e.preventDefault();
  scrollTo({ top: 0, behavior: reduced ? "auto" : "smooth" });
});
