// ==UserScript==
// @name         Feader: Get Cookies
// @namespace    com.frankie.feader
// @version      0.1.0
// @description  Extract cookies from the current page for use in Feader XPath source cookie field
// @author       Feader
// @match        *://*/*
// @grant        GM_setClipboard
// @grant        GM_registerMenuCommand
// ==/UserScript==

(function () {
  "use strict";

  const PANEL_ID = "feader-cookie-panel";

  function domainKey() {
    return location.hostname.replace(/^www\./, "");
  }

  function envVarName() {
    return "FEADER_COOKIE_" + domainKey().toUpperCase().replace(/[.-]/g, "_");
  }

  function getCookies() {
    return document.cookie
      .split(";")
      .map((c) => c.trim())
      .filter((c) => c.length > 0)
      .map((pair) => {
        const eq = pair.indexOf("=");
        if (eq === -1) return { name: pair, value: "" };
        return {
          name: pair.slice(0, eq).trim(),
          value: pair.slice(eq + 1),
        };
      });
  }

  function toJSON(cookies) {
    const obj = {};
    for (const c of cookies) {
      obj[c.name] = c.value;
    }
    return JSON.stringify(obj, null, 2);
  }

  function toHeader(cookies) {
    return cookies.map((c) => `${c.name}=${c.value}`).join("; ");
  }

  function removePanel() {
    const el = document.getElementById(PANEL_ID);
    if (el) el.remove();
  }

  function showPanel(html) {
    removePanel();
    const panel = document.createElement("div");
    panel.id = PANEL_ID;
    panel.innerHTML = html;
    Object.assign(panel.style, {
      position: "fixed",
      top: "12px",
      right: "12px",
      zIndex: "2147483647",
      background: "#1e1e2e",
      color: "#cdd6f4",
      fontFamily: "ui-monospace, SF Mono, monospace",
      fontSize: "13px",
      lineHeight: "1.5",
      padding: "16px",
      borderRadius: "12px",
      boxShadow: "0 8px 32px rgba(0,0,0,0.4)",
      maxWidth: "480px",
      maxHeight: "80vh",
      overflowY: "auto",
      border: "1px solid #45475a",
    });
    document.body.appendChild(panel);
  }

  function copyAndFlash(text, label) {
    GM_setClipboard(text, "text");
    showToast(`Copied: ${label}`);
  }

  function showToast(msg) {
    const toast = document.createElement("div");
    toast.textContent = msg;
    Object.assign(toast.style, {
      position: "fixed",
      bottom: "20px",
      left: "50%",
      transform: "translateX(-50%)",
      zIndex: "2147483648",
      background: "#a6e3a1",
      color: "#1e1e2e",
      padding: "8px 20px",
      borderRadius: "8px",
      fontSize: "14px",
      fontFamily: "system-ui, sans-serif",
      fontWeight: "600",
      boxShadow: "0 4px 12px rgba(0,0,0,0.3)",
      transition: "opacity 0.3s",
    });
    document.body.appendChild(toast);
    setTimeout(() => {
      toast.style.opacity = "0";
      setTimeout(() => toast.remove(), 300);
    }, 1500);
  }

  function renderPanel() {
    const cookies = getCookies();
    const json = toJSON(cookies);
    const header = toHeader(cookies);
    const varname = envVarName();

    if (cookies.length === 0) {
      showPanel(`
        <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:8px">
          <strong style="color:#f5c2e7">Feader Cookie Extractor</strong>
          <button id="feader-close" style="background:none;border:none;color:#6c7086;cursor:pointer;font-size:16px">&times;</button>
        </div>
        <p style="color:#f38ba8">No cookies found for ${domainKey()}</p>
        <p style="color:#6c7086;font-size:11px">HttpOnly cookies are not accessible via document.cookie</p>
      `);
    } else {
      showPanel(`
        <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:12px">
          <strong style="color:#f5c2e7">Feader Cookie Extractor</strong>
          <button id="feader-close" style="background:none;border:none;color:#6c7086;cursor:pointer;font-size:16px">&times;</button>
        </div>
        <div style="color:#a6adc8;margin-bottom:12px">
          ${cookies.length} cookie(s) for <span style="color:#89b4fa">${domainKey()}</span>
        </div>

        <div style="margin-bottom:10px">
          <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:4px">
            <span style="color:#94e2d5;font-size:11px">JSON format (for XPathSelectors.cookie)</span>
            <button class="feader-copy" data-text="${escapeAttr(json)}" data-label="JSON">Copy</button>
          </div>
          <pre style="background:#181825;padding:8px;border-radius:6px;overflow-x:auto;margin:0;font-size:11px;max-height:160px;overflow-y:auto;color:#cdd6f4">${escapeHTML(json)}</pre>
        </div>

        <div style="margin-bottom:10px">
          <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:4px">
            <span style="color:#94e2d5;font-size:11px">Raw header format</span>
            <button class="feader-copy" data-text="${escapeAttr(header)}" data-label="Raw header">Copy</button>
          </div>
          <pre style="background:#181825;padding:8px;border-radius:6px;overflow-x:auto;margin:0;font-size:11px;color:#cdd6f4">${escapeHTML(header)}</pre>
        </div>

        <div style="margin-bottom:4px">
          <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:4px">
            <span style="color:#94e2d5;font-size:11px">Env var export</span>
            <button class="feader-copy" data-text="${escapeAttr(`export ${varname}='${json}'`)}" data-label="Env var">Copy</button>
          </div>
          <pre style="background:#181825;padding:8px;border-radius:6px;overflow-x:auto;margin:0;font-size:11px;color:#cdd6f4">export ${varname}='${escapeHTML(json)}'</pre>
        </div>

        <div style="margin-top:12px;padding-top:8px;border-top:1px solid #45475a;color:#6c7086;font-size:11px">
          Reference in XPath config as: <code style="color:#f9e2af;background:#181825;padding:1px 4px;border-radius:3px">"$${varname}"</code>
        </div>
      `);
    }

    document.getElementById("feader-close")?.addEventListener("click", removePanel);

    document.querySelectorAll(".feader-copy").forEach((btn) => {
      btn.addEventListener("click", function () {
        copyAndFlash(this.dataset.text, this.dataset.label);
      });
      Object.assign(btn.style, {
        background: "#45475a",
        color: "#cdd6f4",
        border: "none",
        padding: "2px 10px",
        borderRadius: "4px",
        cursor: "pointer",
        fontSize: "11px",
        fontWeight: "600",
      });
    });
  }

  function escapeHTML(s) {
    return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }

  function escapeAttr(s) {
    return s.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/'/g, "&#39;");
  }

  function togglePanel() {
    if (document.getElementById(PANEL_ID)) {
      removePanel();
    } else {
      renderPanel();
    }
  }

  // Register menu command
  GM_registerMenuCommand("Feader: Extract Cookies", togglePanel);

  // Keyboard shortcut: Ctrl+Shift+C (Cmd+Shift+C on Mac)
  document.addEventListener("keydown", function (e) {
    if (e.key === "C" && e.shiftKey && (e.ctrlKey || e.metaKey) && !e.altKey) {
      // Don't trigger on input/textarea
      const tag = document.activeElement?.tagName?.toLowerCase();
      if (tag === "input" || tag === "textarea" || tag === "select") return;
      e.preventDefault();
      togglePanel();
    }
  });
})();
