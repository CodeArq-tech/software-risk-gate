/* Shared, seek-safe scene engine for the Risk Gate demo film.

   Every scene describes its cursor path, clicks, camera poses, and typing as
   data. RG.scene() turns that data into one pure render(t) function and one
   paused GSAP timeline that calls it, so any frame can be seeked in any order
   and always paints the same pixels. Geometry is measured from layout offsets,
   which ignore the camera transform and any player scaling. */
(function () {
  "use strict";

  var W = 1920;
  var H = 1080;

  function clamp01(u) {
    return u < 0 ? 0 : u > 1 ? 1 : u;
  }
  function mix(a, b, u) {
    return a + (b - a) * u;
  }
  function ease(name) {
    return gsap.parseEase(name || "power2.inOut");
  }
  function seg(t, at, dur, easing) {
    var u = clamp01((t - at) / Math.max(dur, 0.0001));
    return easing ? ease(easing)(u) : u;
  }
  // Clamp v into [lo, hi]; when the view is wider than the bounds, centre it.
  // The clamp fades in with zoom so a zoom-out lands exactly on the rest frame.
  function clampRange(v, lo, hi, weight) {
    var target = lo > hi ? (lo + hi) / 2 : Math.max(lo, Math.min(hi, v));
    return v + (target - v) * (weight == null ? 1 : weight);
  }
  function val(v, t) {
    return typeof v === "function" ? v(t) : v;
  }

  /* ---------- language ---------- */

  function lang() {
    // assets/lang.js (written by scripts/render.sh) wins over the composition
    // variable, so a render never depends on how variables reach sub-scenes.
    var vars = window.__hfVariables || {};
    var chosen = window.RG_LANG || vars.lang || "en";
    return window.RG_I18N && window.RG_I18N[chosen] ? chosen : "en";
  }
  function tr(key) {
    var table = window.RG_I18N || {};
    var current = table[lang()] || {};
    if (current[key] != null) return current[key];
    return (table.en || {})[key] != null ? table.en[key] : key;
  }
  function applyText(root) {
    root.querySelectorAll("[data-i18n]").forEach(function (el) {
      el.textContent = tr(el.getAttribute("data-i18n"));
    });
  }

  /* ---------- geometry ---------- */

  // Position of el's box relative to stage, from offset chains (transform-free).
  function box(el, stage) {
    var x = 0;
    var y = 0;
    var node = el;
    while (node && node !== stage) {
      x += node.offsetLeft;
      y += node.offsetTop;
      node = node.offsetParent;
    }
    return { x: x, y: y, w: el.offsetWidth, h: el.offsetHeight };
  }
  function anchor(el, stage, ax, ay) {
    var b = box(el, stage);
    return { x: b.x + b.w * (ax == null ? 0.5 : ax), y: b.y + b.h * (ay == null ? 0.5 : ay) };
  }

  /* ---------- syntax highlighting (YAML) ---------- */

  function esc(s) {
    return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }
  function yamlValue(v) {
    if (!v) return "";
    var lead = v.match(/^\s*/)[0];
    var body = v.slice(lead.length);
    var cls = "tk-str";
    if (/^(true|false|null|-?\d+(\.\d+)?)$/.test(body)) cls = "tk-num";
    else if (/^\[/.test(body)) cls = "tk-punc";
    else if (/^\$\{\{/.test(body)) cls = "tk-var";
    else if (/^"/.test(body)) cls = "tk-str";
    else cls = "tk-str";
    return esc(lead) + '<span class="' + cls + '">' + esc(body) + "</span>";
  }
  function yamlLine(line) {
    var m = line.match(/^(\s*)(- )?([A-Za-z_][\w-]*)(:)(.*)$/);
    if (m) {
      return (
        esc(m[1]) +
        (m[2] ? '<span class="tk-punc">- </span>' : "") +
        '<span class="tk-key">' +
        esc(m[3]) +
        '</span><span class="tk-punc">:</span>' +
        yamlValue(m[5])
      );
    }
    var item = line.match(/^(\s*)(- )(.*)$/);
    if (item) return esc(item[1]) + '<span class="tk-punc">- </span>' + yamlValue(item[3]);
    return esc(line);
  }

  // Fill an editor element with numbered, highlighted lines.
  function editorLines(container, text) {
    var html = text
      .split("\n")
      .map(function (line, i) {
        return (
          '<div class="ed-line"><span class="n">' +
          (i + 1) +
          '</span><span class="c">' +
          yamlLine(line) +
          "</span></div>"
        );
      })
      .join("");
    container.innerHTML = html;
  }

  /* ---------- typing ---------- */

  // Wraps every character of `root` text in a span. Lines (elements matching
  // lineSel) end with a virtual newline so the caret walks to the next line.
  function typer(root, stage, opts) {
    opts = opts || {};
    var lineEls = opts.lineSel ? Array.prototype.slice.call(root.querySelectorAll(opts.lineSel)) : [root];
    var chars = [];
    lineEls.forEach(function (lineEl, li) {
      var host = opts.textSel ? lineEl.querySelector(opts.textSel) : lineEl;
      var walker = document.createTreeWalker(host, NodeFilter.SHOW_TEXT);
      var texts = [];
      while (walker.nextNode()) texts.push(walker.currentNode);
      var lineStart = chars.length;
      texts.forEach(function (node) {
        var frag = document.createDocumentFragment();
        for (var i = 0; i < node.data.length; i++) {
          var s = document.createElement("span");
          s.textContent = node.data[i];
          s.style.visibility = "hidden";
          frag.appendChild(s);
          chars.push({ el: s, line: li });
        }
        node.parentNode.replaceChild(frag, node);
      });
      if (opts.lineSel && li < lineEls.length - 1) chars.push({ el: null, line: li, newline: true });
      lineEl.__start = lineStart;
    });
    var numbers = lineEls.map(function (l) {
      return l.querySelector(".n");
    });
    var shown = -1;
    var geo = null;
    function measure() {
      var lines = lineEls.map(function (l) {
        var host = opts.textSel ? l.querySelector(opts.textSel) : l;
        return box(host, stage);
      });
      var pts = chars.map(function (c) {
        if (c.el) {
          var b = box(c.el, stage);
          return { x: b.x + b.w, y: b.y, h: b.h || 22 };
        }
        var next = lines[c.line + 1];
        return { x: next.x, y: next.y, h: next.h || 22 };
      });
      var first = lines[0];
      geo = { start: { x: first.x, y: first.y, h: first.h || 22 }, pts: pts };
      return first.w > 0 || first.x > 0;
    }
    return {
      count: chars.length,
      lineCount: lineEls.length,
      lineStartIndex: function (li) {
        return lineEls[li].__start;
      },
      measure: measure,
      set: function (k) {
        k = Math.max(0, Math.min(chars.length, Math.round(k)));
        if (k === shown) return;
        for (var i = 0; i < chars.length; i++) {
          if (chars[i].el) chars[i].el.style.visibility = i < k ? "" : "hidden";
        }
        if (opts.lineSel) {
          var lastLine = k === 0 ? 0 : chars[k - 1].newline ? chars[k - 1].line + 1 : chars[k - 1].line;
          numbers.forEach(function (n, li) {
            if (n) n.style.visibility = li <= lastLine ? "" : "hidden";
          });
        }
        shown = k;
      },
      caret: function (k) {
        if (!geo) measure();
        k = Math.max(0, Math.min(chars.length, Math.round(k)));
        return k === 0 ? geo.start : geo.pts[k - 1];
      },
    };
  }

  // Piecewise-linear character count over time: [{at, dur, to}] (to = absolute count).
  function schedule(parts) {
    return function (t) {
      var n = 0;
      for (var i = 0; i < parts.length; i++) {
        var p = parts[i];
        if (t < p.at) return n;
        var from = n;
        n = mix(from, p.to, clamp01((t - p.at) / p.dur));
        if (t < p.at + p.dur) return n;
        n = p.to;
      }
      return n;
    };
  }

  /* ---------- scene ---------- */

  var CURSOR_SVG =
    '<svg viewBox="0 0 24 24"><path d="M5 3 L5 19 L9 15 L12 22 L15 20.5 L11.5 14 L18 14 Z"/></svg>';

  function scene(o) {
    var stage = o.stage;
    var cam = o.cam;
    var duration = o.duration;
    var cursorEl = null;
    var rippleEl = null;
    if (o.cursor !== false) {
      var holder = document.createElement("div");
      holder.innerHTML = CURSOR_SVG;
      cursorEl = holder.firstChild;
      cursorEl.setAttribute("class", "rg-cursor");
      rippleEl = document.createElement("div");
      rippleEl.className = "rg-ripple";
      cam.appendChild(rippleEl);
      cam.appendChild(cursorEl);
    }
    var moveEase = ease("power2.inOut");
    var camEase = ease("power2.inOut");
    var outEase = ease("power2.out");
    var ready = false;
    var moves = [];
    var startPoint = null;
    var camKeys = [];
    var springSamples = null;
    var FPS = 60;

    function resolvePoint(p) {
      if (typeof p === "function") return p;
      if (p && p.el) {
        return anchor(p.el, stage, p.ax, p.ay);
      }
      return p;
    }

    // A scene outside its time window is display:none, so it has no layout.
    // Reveal its hidden ancestors just long enough to measure, then restore them.
    function withLayout(fn) {
      var undo = [];
      for (var node = stage; node && node.nodeType === 1; node = node.parentElement) {
        if (getComputedStyle(node).display === "none") {
          undo.push([node, node.style.getPropertyValue("display"), node.style.getPropertyPriority("display")]);
          node.style.setProperty("display", "block", "important");
        }
      }
      try {
        return fn();
      } finally {
        undo.forEach(function (u) {
          if (u[1]) u[0].style.setProperty("display", u[1], u[2]);
          else u[0].style.removeProperty("display");
        });
      }
    }

    function setup() {
      return withLayout(measureAll);
    }

    function measureAll() {
      if (!stage.offsetWidth) return false;
      if (o.measure && o.measure() === false) return false;
      startPoint = resolvePoint(o.cursorStart || { x: W * 0.62, y: H * 0.72 });
      if (typeof startPoint === "function") startPoint = startPoint(0);
      moves = (o.moves || []).map(function (m) {
        return { at: m.at, end: m.at + m.dur, to: resolvePoint(m.to), bend: m.bend == null ? 0.1 : m.bend };
      });
      camKeys = (o.camera || [{ t: 0, z: 1, x: W / 2, y: H / 2 }]).map(function (k) {
        var p = k.at ? resolvePoint(k.at) : { x: k.x, y: k.y };
        return { t: k.t, z: k.z, p: p, ease: k.ease ? ease(k.ease) : camEase };
      });
      // Critically damped spring over the camera aim, integrated once.
      var omega = o.spring || 5.5;
      var frames = Math.ceil(duration * FPS) + 2;
      var s = rawAim(0);
      var v = { x: 0, y: 0 };
      var sub = 4;
      var dt = 1 / (FPS * sub);
      springSamples = [];
      for (var f = 0; f <= frames; f++) {
        springSamples.push({ x: s.x, y: s.y });
        for (var j = 0; j < sub; j++) {
          var aim = rawAim((f * sub + j) * dt);
          v.x += (omega * omega * (aim.x - s.x) - 2 * omega * v.x) * dt;
          v.y += (omega * omega * (aim.y - s.y) - 2 * omega * v.y) * dt;
          s.x += v.x * dt;
          s.y += v.y * dt;
        }
      }
      ready = true;
      return true;
    }

    function pointAt(p, t) {
      var q = typeof p === "function" ? p(t) : p;
      return q;
    }

    function cursorAt(t) {
      var here = startPoint;
      for (var i = 0; i < moves.length; i++) {
        var m = moves[i];
        var target = pointAt(m.to, t);
        if (t < m.at) return here;
        if (t <= m.end) {
          var u = moveEase(clamp01((t - m.at) / (m.end - m.at)));
          var cx = (here.x + target.x) / 2 - (target.y - here.y) * m.bend;
          var cy = (here.y + target.y) / 2 + (target.x - here.x) * m.bend;
          var w = 1 - u;
          return {
            x: w * w * here.x + 2 * w * u * cx + u * u * target.x,
            y: w * w * here.y + 2 * w * u * cy + u * u * target.y,
          };
        }
        here = target;
      }
      return here;
    }

    function keyAt(t) {
      var k = camKeys;
      if (t <= k[0].t) return { z: k[0].z, p: pointAt(k[0].p, t) };
      for (var i = 0; i < k.length - 1; i++) {
        var a = k[i];
        var b = k[i + 1];
        if (t <= b.t) {
          var u = b.ease(clamp01((t - a.t) / Math.max(0.0001, b.t - a.t)));
          var pa = pointAt(a.p, t);
          var pb = pointAt(b.p, t);
          return { z: mix(a.z, b.z, u), p: { x: mix(pa.x, pb.x, u), y: mix(pa.y, pb.y, u) } };
        }
      }
      var last = k[k.length - 1];
      return { z: last.z, p: pointAt(last.p, t) };
    }
    function rawAim(t) {
      return keyAt(t).p;
    }
    function aimAt(t) {
      var p = Math.max(0, Math.min(springSamples.length - 1, t * FPS));
      var i = Math.floor(p);
      var a = springSamples[i];
      var b = springSamples[Math.min(springSamples.length - 1, i + 1)];
      return { x: mix(a.x, b.x, p - i), y: mix(a.y, b.y, p - i) };
    }

    function pressAt(since) {
      if (since < 0 || since >= 0.3) return 0;
      return since < 0.08 ? since / 0.08 : 1 - outEase((since - 0.08) / 0.22);
    }

    function render(t) {
      if (!ready && !setup()) return;
      var k = keyAt(t);
      var z = k.z;
      var c = aimAt(t);
      // Near zoom 1 the spring settles to centre; blend so rest frames are exact.
      var settle = clamp01((z - 1) / 0.08);
      c = { x: mix(W / 2, c.x, settle), y: mix(H / 2, c.y, settle) };
      // Keep the zoomed view inside the window so the frame never shows past its edge.
      var bounds = o.bounds || { x: 150, y: 64, w: 1620, h: 828 };
      var hw = W / (2 * z);
      var hh = H / (2 * z);
      var cx = clampRange(c.x, bounds.x + hw, bounds.x + bounds.w - hw, settle);
      var cy = clampRange(c.y, bounds.y + hh, bounds.y + bounds.h - hh, settle);
      cam.style.transform =
        z === 1
          ? ""
          : "translate(" +
            (W / 2 - cx * z).toFixed(2) +
            "px," +
            (H / 2 - cy * z).toFixed(2) +
            "px) scale(" +
            z.toFixed(4) +
            ")";

      if (cursorEl) {
        var p = cursorAt(t);
        var last = null;
        (o.clicks || []).forEach(function (at) {
          if (t >= at) last = at;
        });
        var since = last == null ? 99 : t - last;
        var press = pressAt(since);
        cursorEl.style.transform =
          "translate(" + p.x.toFixed(2) + "px," + p.y.toFixed(2) + "px) scale(" + (1 - 0.16 * press).toFixed(3) + ")";
        var hidden = o.cursorVisible ? !o.cursorVisible(t) : false;
        cursorEl.style.opacity = hidden ? "0" : "1";
        var ring = since / 0.45;
        if (ring < 1 && !hidden) {
          var cp = cursorAt(last);
          var r = 10 + 34 * outEase(ring);
          rippleEl.style.opacity = (1 - ring).toFixed(3);
          rippleEl.style.width = rippleEl.style.height = (2 * r).toFixed(1) + "px";
          rippleEl.style.transform = "translate(" + (cp.x - r).toFixed(1) + "px," + (cp.y - r).toFixed(1) + "px)";
        } else {
          rippleEl.style.opacity = "0";
        }
      }
      if (o.render) o.render(t, { press: pressAt });
    }

    render(0);
    return { render: render, cursorAt: cursorAt };
  }

  // Registers the scene timeline synchronously (the runtime binds timelines
  // when scripts finish) and builds the scene once fonts are ready, so measured
  // glyph geometry is final. build() returns the object from RG.scene().
  function mount(id, duration, build) {
    var clock = { t: 0 };
    var current = null;
    var tl = gsap.timeline({ paused: true });
    tl.to(
      clock,
      {
        t: duration,
        duration: duration,
        ease: "none",
        onUpdate: function () {
          if (current) current.render(clock.t);
        },
      },
      0,
    );
    window.__timelines[id] = tl;
    var go = function () {
      if (current) return;
      current = build();
      current.render(clock.t);
    };
    if (!document.fonts || document.fonts.status === "loaded") go();
    else document.fonts.ready.then(go);
  }

  /* ---------- icons and window chrome ---------- */

  var ICONS = {
    mark:
      '<svg viewBox="0 0 16 16" class="%c"><path fill="#1f2328" d="M8 0c4.42 0 8 3.58 8 8a8.013 8.013 0 0 1-5.45 7.59c-.4.08-.55-.17-.55-.38 0-.27.01-1.13.01-2.2 0-.75-.25-1.23-.54-1.48 1.78-.2 3.65-.88 3.65-3.95 0-.88-.31-1.59-.82-2.15.08-.2.36-1.02-.08-2.12 0 0-.67-.22-2.2.82-.64-.18-1.32-.27-2-.27-.68 0-1.36.09-2 .27-1.53-1.03-2.2-.82-2.2-.82-.44 1.1-.16 1.92-.08 2.12-.51.56-.82 1.28-.82 2.15 0 3.06 1.86 3.75 3.64 3.95-.23.2-.44.55-.51 1.07-.46.21-1.61.55-2.33-.66-.15-.24-.6-.83-1.23-.82-.67.01-.27.38.01.53.34.19.73.9.82 1.13.16.45.68 1.31 2.69.94 0 .67.01 1.3.01 1.49 0 .21-.15.45-.55.38A7.995 7.995 0 0 1 0 8c0-4.42 3.58-8 8-8Z"/></svg>',
    code: '<svg viewBox="0 0 16 16" class="%c" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M5 4 1.5 8 5 12M11 4l3.5 4L11 12"/></svg>',
    issue: '<svg viewBox="0 0 16 16" class="%c" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="8" cy="8" r="6.2"/><circle cx="8" cy="8" r="1.4" fill="currentColor"/></svg>',
    pr: '<svg viewBox="0 0 16 16" class="%c" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="4" cy="3.5" r="1.8"/><circle cx="4" cy="12.5" r="1.8"/><circle cx="12" cy="12.5" r="1.8"/><path d="M4 5.3v5.4M12 10.7V6a2 2 0 0 0-2-2H7.5M9 2.5 7.4 4 9 5.5"/></svg>',
    play: '<svg viewBox="0 0 16 16" class="%c" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="8" cy="8" r="6.2"/><path d="M6.6 5.4v5.2L10.6 8z" fill="currentColor" stroke="none"/></svg>',
    gear: '<svg viewBox="0 0 16 16" class="%c" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="8" cy="8" r="2.2"/><circle cx="8" cy="8" r="5.6" stroke-dasharray="2.2 1.9"/></svg>',
    lock: '<svg viewBox="0 0 16 16" class="%c" fill="none" stroke="#475569" stroke-width="1.6"><rect x="3" y="7" width="10" height="7" rx="1.5"/><path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2"/></svg>',
    search: '<svg viewBox="0 0 16 16" class="%c" fill="none" stroke="currentColor" stroke-width="1.6"><circle cx="7" cy="7" r="4.6"/><path d="m10.5 10.5 3.5 3.5" stroke-linecap="round"/></svg>',
    check: '<svg viewBox="0 0 16 16" class="%c"><circle cx="8" cy="8" r="7.5" fill="#1a7f37"/><path d="m4.8 8.2 2.2 2.2 4.2-4.6" fill="none" stroke="#fff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>',
    x: '<svg viewBox="0 0 16 16" class="%c"><circle cx="8" cy="8" r="7.5" fill="#d1242f"/><path d="m5.4 5.4 5.2 5.2m0-5.2-5.2 5.2" stroke="#fff" stroke-width="1.8" stroke-linecap="round"/></svg>',
    pending: '<svg viewBox="0 0 16 16" class="%c"><circle cx="8" cy="8" r="6.4" fill="none" stroke="#d4a72c" stroke-width="2.2" stroke-dasharray="26 14"/></svg>',
    dot: '<svg viewBox="0 0 16 16" class="%c"><circle cx="8" cy="8" r="4.5" fill="#bf8700"/></svg>',
    skip: '<svg viewBox="0 0 16 16" class="%c"><circle cx="8" cy="8" r="6.6" fill="none" stroke="#8c959f" stroke-width="1.5"/></svg>',
    commit: '<svg viewBox="0 0 16 16" class="%c" fill="none" stroke="#59636e" stroke-width="1.6"><circle cx="8" cy="8" r="2.8"/><path d="M0 8h5.2M10.8 8H16"/></svg>',
    merged: '<svg viewBox="0 0 16 16" class="%c" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="4" cy="3.5" r="1.8"/><circle cx="4" cy="12.5" r="1.8"/><circle cx="12" cy="8.5" r="1.8"/><path d="M4 5.3v5.4M5.2 4.8c1.2 2.4 3 3.4 5 3.6"/></svg>',
    chevron: '<svg viewBox="0 0 16 16" class="%c" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m4 6 4 4 4-4"/></svg>',
  };
  function icon(name, cls) {
    return (ICONS[name] || "").replace("%c", cls || "ico");
  }

  // Browser bar, GitHub header, and repository tabs, prepended into .win.
  function chrome(win, opts) {
    var tabs = [
      ["code", "Code", null],
      ["issue", "Issues", "3"],
      ["pr", "Pull requests", opts.prs || null],
      ["play", "Actions", null],
      ["gear", "Settings", null],
    ];
    var nav = tabs
      .map(function (tab) {
        return (
          '<div class="tab' +
          (opts.tab === tab[1] ? " on" : "") +
          '">' +
          icon(tab[0]) +
          "<span>" +
          tab[1] +
          "</span>" +
          (tab[2] ? '<span class="gh-count">' + tab[2] + "</span>" : "") +
          "</div>"
        );
      })
      .join("");
    var html =
      '<div class="win-bar"><div class="win-dots"><span></span><span></span><span></span></div>' +
      '<div class="win-url">' +
      icon("lock") +
      "<span><b>github.com</b>" +
      esc(opts.path || "/acme/storefront") +
      "</span></div></div>" +
      '<div class="gh-header">' +
      icon("mark", "gh-mark") +
      '<div class="gh-crumb"><span class="owner">acme</span><span class="sep">/</span><span class="repo">storefront</span></div>' +
      '<div class="spacer"></div><div class="gh-search">' +
      icon("search") +
      "<span>Type / to search</span></div>" +
      '<div class="gh-avatar av-dana">D</div></div>' +
      '<div class="gh-nav">' +
      nav +
      "</div>";
    win.insertAdjacentHTML("afterbegin", html);
  }

  // Pull request title, state, and tabs; returns the inserted element.
  function prHeader(page, opts) {
    var tabs = [
      ["Conversation", "issue", "3"],
      ["Commits", "commit", opts.commits || "1"],
      ["Checks", "play", "1"],
      ["Files changed", "code", "1"],
    ];
    var html =
      '<div class="pr-head"><div class="pr-title"><span>Speed up checkout totals</span><span class="num">#42</span></div>' +
      '<div class="pr-meta"><span class="state open" data-state>' +
      icon("pr", "ico") +
      "<span>Open</span></span>" +
      '<span><b style="color:var(--fg)">dana</b> wants to merge ' +
      (opts.commits || "1") +
      ' commit into <span class="branch">main</span> from <span class="branch">speed-up-checkout</span></span></div></div>' +
      '<div class="pr-tabs">' +
      tabs
        .map(function (tab) {
          return (
            '<div class="tab' +
            (opts.tab === tab[0] ? " on" : "") +
            '" data-tab="' +
            tab[0] +
            '">' +
            icon(tab[1]) +
            "<span>" +
            tab[0] +
            '</span><span class="gh-count">' +
            tab[2] +
            "</span></div>"
          );
        })
        .join("") +
      "</div>";
    var wrap = document.createElement("div");
    wrap.className = "pr-wrap";
    wrap.innerHTML = html;
    page.insertBefore(wrap, page.firstChild);
    return wrap;
  }

  window.RG = {
    prHeader: prHeader,
    icon: icon,
    chrome: chrome,
    W: W,
    H: H,
    clamp01: clamp01,
    mix: mix,
    seg: seg,
    ease: ease,
    val: val,
    lang: lang,
    t: tr,
    applyText: applyText,
    box: box,
    anchor: anchor,
    editorLines: editorLines,
    typer: typer,
    schedule: schedule,
    scene: scene,
    mount: mount,
  };
})();
