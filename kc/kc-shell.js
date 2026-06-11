/* KC shell — injects the shared sidebar + mobile topbar + theme toggle into every
   knowledge-center page, and stamps an automatic reading-time badge under the first
   <h1>. Single source of truth for the nav: edit NAV here, every page follows.
   Pages opt in with:  <link rel="stylesheet" href="kc-shell.css">  +
   <script defer src="kc-shell.js"></script>  (plus the tiny FOUC theme snippet). */
(function () {
  "use strict";

  var NAV = [
    { heading: "안내", items: [
      { href: "index.html", icon: "🏠", label: "처음 오신 분을 위한 안내" },
      { href: "summary.html", icon: "⏱️", label: "5분 요약" },
      { href: "news.html", icon: "📰", label: "업데이트 소식 (NEWS)" },
    ]},
    { heading: "여정 — 5개 챕터", items: [
      { href: "tt-assignment-session-ko.html", chap: "01", label: "문제 — 스마트 TT 배차" },
      { href: "tos-db-reference-ko.html", chap: "02", label: "데이터 — TOS DB 레퍼런스" },
      { href: "websocket-data-ko.html", chap: "02", label: "데이터 — 실시간 websocket" },
      { href: "websocket-fields-ko.html", chap: "02", label: "데이터 — websocket 원본 필드" },
      { href: "kpi-computation-ko.html", chap: "03", label: "지표 — KPI 산출" },
      { href: "websocket-kpi-accuracy-ko.html", chap: "03", label: "지표 — 정확도 보강" },
      { href: "dispatch-pools-ko.html", chap: "04", label: "운영 — 배차 풀 (라이브)" },
      { href: "cycle-detection-ko.html", chap: "04", label: "운영 — 사이클 감지 로직" },
      { href: "tt-prediction-research-ko.html", chap: "05", label: "다음 — 예측 모형 연구" },
    ]},
    { heading: "참고 자료", items: [
      { href: "glossary.html", icon: "📚", label: "용어집 & FAQ" },
      { href: "research-log.html", icon: "📝", label: "연구 일지 (시간순)" },
      { href: "cycle-v2-design-ko.html", icon: "📐", label: "사이클 v2 설계 (그림자)" },
      { href: "ops-runbook-ko.html", icon: "🛠️", label: "운영 런북 & 로드맵" },
      { href: "capacity-planning-ko.html", icon: "☁️", label: "클라우드 용량 산정" },
    ]},
  ];

  function esc(s) {
    return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }

  function currentPage() {
    var p = location.pathname.split("/").pop();
    return p && p.length ? p : "index.html";
  }

  function buildSidebar() {
    var cur = currentPage();
    var h = '<div class="kc-brand"><div class="t">TT <span class="accent">AiOps</span> 지식센터</div>' +
            '<div class="s">Westports · Knowledge Center</div></div>';
    NAV.forEach(function (sec) {
      h += '<div class="kc-nav-section"><div class="heading">' + esc(sec.heading) + "</div>";
      sec.items.forEach(function (it) {
        var badge = it.chap
          ? '<span class="chap">' + it.chap + "</span>"
          : '<span class="icon">' + (it.icon || "·") + "</span>";
        h += '<a class="kc-nav-item' + (it.href === cur ? " active" : "") + '" href="' + it.href + '">' +
             badge + "<span>" + esc(it.label) + "</span></a>";
      });
      h += "</div>";
    });
    h += '<div class="kc-side-spacer"></div>';
    h += '<div class="kc-theme-section"><button class="kc-theme-toggle" type="button">' +
         '<span class="icon theme-icon"></span><span class="theme-label"></span></button></div>';
    h += '<div class="kc-side-footer"><strong>wp-tt-dashboard</strong> 내부 기술 문서<br>' +
         "tos-db-research · /kc/ 정적 서빙</div>";
    var aside = document.createElement("aside");
    aside.className = "kc-sidebar";
    aside.innerHTML = h;
    return aside;
  }

  function buildTopbar(aside) {
    var bar = document.createElement("div");
    bar.className = "kc-topbar";
    bar.innerHTML = '<button type="button" aria-label="menu">☰</button><span class="t">TT AiOps 지식센터</span>';
    bar.querySelector("button").addEventListener("click", function () {
      aside.classList.toggle("open");
    });
    return bar;
  }

  function applyThemeLabel(btn) {
    var theme = document.documentElement.getAttribute("data-theme") || "dark";
    btn.querySelector(".theme-icon").textContent = theme === "dark" ? "☀️" : "🌙";
    btn.querySelector(".theme-label").textContent =
      theme === "dark" ? "라이트 모드로 전환" : "다크 모드로 전환";
  }

  function injectReadtime() {
    if (currentPage() === "index.html") return; // the home shows per-card times instead
    var h1 = document.querySelector("h1");
    if (!h1 || document.querySelector(".kc-readtime")) return;
    var clone = document.body.cloneNode(true);
    ["aside", ".kc-topbar", "script", "style"].forEach(function (sel) {
      clone.querySelectorAll(sel).forEach(function (n) { n.remove(); });
    });
    var chars = (clone.textContent || "").replace(/\s+/g, "").length;
    var min = Math.max(1, Math.round(chars / 600)); // ~600 chars/min Korean technical read
    var b = document.createElement("span");
    b.className = "kc-readtime";
    b.textContent = "📖 읽기 약 " + min + "분";
    h1.insertAdjacentElement("afterend", b);
  }

  function init() {
    var aside = buildSidebar();
    document.body.prepend(aside);
    document.body.prepend(buildTopbar(aside));
    var btn = aside.querySelector(".kc-theme-toggle");
    applyThemeLabel(btn);
    btn.addEventListener("click", function () {
      var cur = document.documentElement.getAttribute("data-theme") || "dark";
      var next = cur === "dark" ? "light" : "dark";
      document.documentElement.setAttribute("data-theme", next);
      try { localStorage.setItem("kc-theme", next); } catch (e) { /* private mode */ }
      applyThemeLabel(btn);
    });
    injectReadtime();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
