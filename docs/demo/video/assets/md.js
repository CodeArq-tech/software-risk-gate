/* Minimal GitHub-flavoured Markdown renderer for the Risk Gate report.
   It covers exactly the constructs risk-gate emits (headings, nested bullets,
   inline code, bold, emphasis, an HTML marker comment, and one Mermaid
   flowchart), so the captured report renders the way GitHub shows it. */
(function () {
  "use strict";

  function esc(s) {
    return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }
  function inline(s) {
    var parts = s.split("`");
    return parts
      .map(function (part, i) {
        if (i % 2 === 1) return "<code>" + esc(part) + "</code>";
        var t = esc(part).replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
        return t;
      })
      .join("");
  }

  // Mermaid flowchart LR -> SVG using Mermaid's default (light) theme colours.
  function flowchart(src) {
    var nodes = {};
    var order = [];
    var edges = [];
    src.split("\n").forEach(function (line) {
      var n = line.match(/^\s*(\w+)\["([^"]*)"\]\s*$/);
      if (n) {
        nodes[n[1]] = { id: n[1], label: n[2].replace(/&quot;/g, '"').replace(/&amp;/g, "&") };
        order.push(n[1]);
        return;
      }
      var e = line.match(/^\s*(\w+)\s*-->\|"([^"]*)"\|\s*(\w+)\s*$/);
      if (e) edges.push({ from: e[1], label: e[2], to: e[3] });
    });
    // Rank = longest path from a root; rows keep declaration order within a rank.
    var rank = {};
    order.forEach(function (id) {
      rank[id] = 0;
    });
    for (var pass = 0; pass < order.length; pass++) {
      edges.forEach(function (e) {
        if (rank[e.to] < rank[e.from] + 1) rank[e.to] = rank[e.from] + 1;
      });
    }
    var cols = {};
    order.forEach(function (id) {
      (cols[rank[id]] = cols[rank[id]] || []).push(id);
    });
    var NW = 250;
    var NH = 58;
    var GAPX = 150;
    var GAPY = 44;
    var maxRows = 0;
    Object.keys(cols).forEach(function (c) {
      maxRows = Math.max(maxRows, cols[c].length);
    });
    var height = maxRows * NH + (maxRows - 1) * GAPY + 20;
    var ncols = Object.keys(cols).length;
    var width = ncols * NW + (ncols - 1) * GAPX + 20;
    Object.keys(cols).forEach(function (c) {
      var ids = cols[c];
      var colH = ids.length * NH + (ids.length - 1) * GAPY;
      ids.forEach(function (id, i) {
        nodes[id].x = 10 + Number(c) * (NW + GAPX);
        nodes[id].y = (height - colH) / 2 + i * (NH + GAPY);
      });
    });
    // A child of a single-parent chain sits on its parent's row.
    edges.forEach(function (e) {
      var siblings = cols[rank[e.to]];
      if (siblings.length === 1) nodes[e.to].y = nodes[e.from].y;
    });
    var svg =
      '<svg class="mm" viewBox="0 0 ' +
      width +
      " " +
      height +
      '" width="' +
      width +
      '" height="' +
      height +
      '"><defs><marker id="mm-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="8" markerHeight="8" orient="auto"><path d="M0 0L10 5L0 10z" fill="#333333"/></marker></defs>';
    edges.forEach(function (e, i) {
      var a = nodes[e.from];
      var b = nodes[e.to];
      var x1 = a.x + NW;
      var y1 = a.y + NH / 2;
      var x2 = b.x - 2;
      var y2 = b.y + NH / 2;
      var mx = (x1 + x2) / 2;
      var d = "M" + x1 + " " + y1 + " C" + mx + " " + y1 + " " + mx + " " + y2 + " " + x2 + " " + y2;
      svg += '<path class="mm-edge" data-edge="' + i + '" d="' + d + '" fill="none" stroke="#333333" stroke-width="2" marker-end="url(#mm-arrow)"/>';
      var lx = mx;
      var ly = (y1 + y2) / 2;
      svg +=
        '<g class="mm-label"><rect x="' +
        (lx - 34) +
        '" y="' +
        (ly - 14) +
        '" width="68" height="28" rx="3" fill="#e8e8e8" fill-opacity="0.85"/><text x="' +
        lx +
        '" y="' +
        (ly + 6) +
        '" text-anchor="middle" font-size="16" fill="#333">' +
        esc(e.label) +
        "</text></g>";
    });
    order.forEach(function (id) {
      var n = nodes[id];
      svg +=
        '<g class="mm-node" data-node="' +
        id +
        '"><rect x="' +
        n.x +
        '" y="' +
        n.y +
        '" width="' +
        NW +
        '" height="' +
        NH +
        '" rx="5" fill="#ECECFF" stroke="#9370DB" stroke-width="1.5"/><text x="' +
        (n.x + NW / 2) +
        '" y="' +
        (n.y + NH / 2 + 6) +
        '" text-anchor="middle" font-size="17" fill="#333">' +
        esc(n.label) +
        "</text></g>";
    });
    return svg + "</svg>";
  }

  function render(md) {
    var lines = md.split("\n");
    var out = [];
    var inList = false;
    var inSub = false;
    var fence = null;
    function closeLists() {
      if (inSub) out.push("</ul></li>");
      else if (inList) out.push("</li>");
      if (inList) out.push("</ul>");
      inList = false;
      inSub = false;
    }
    for (var i = 0; i < lines.length; i++) {
      var line = lines[i];
      if (fence !== null) {
        if (/^```/.test(line)) {
          out.push('<div class="md-mermaid">' + flowchart(fence.join("\n")) + "</div>");
          fence = null;
        } else fence.push(line);
        continue;
      }
      if (/^```mermaid/.test(line)) {
        closeLists();
        fence = [];
        continue;
      }
      if (/^<!--.*-->$/.test(line)) continue;
      var h = line.match(/^(#{2,3}) (.*)$/);
      if (h) {
        closeLists();
        var tag = h[1].length === 2 ? "h2" : "h3";
        out.push("<" + tag + ' data-sec="' + esc(h[2]) + '">' + inline(h[2]) + "</" + tag + ">");
        continue;
      }
      var sub = line.match(/^  - (.*)$/);
      if (sub && inList) {
        if (!inSub) {
          out.push("<ul>");
          inSub = true;
        }
        out.push("<li>" + inline(sub[1]) + "</li>");
        continue;
      }
      var li = line.match(/^- (.*)$/);
      if (li) {
        if (inSub) {
          out.push("</ul></li>");
          inSub = false;
        } else if (inList) out.push("</li>");
        if (!inList) {
          out.push("<ul>");
          inList = true;
        }
        out.push("<li>" + inline(li[1]));
        continue;
      }
      if (line.trim() === "") {
        closeLists();
        continue;
      }
      closeLists();
      var em = line.match(/^_(.*)_$/);
      out.push("<p>" + (em ? "<em>" + inline(em[1]) + "</em>" : inline(line)) + "</p>");
    }
    closeLists();
    return out.join("");
  }

  window.RGMD = { render: render };
})();
