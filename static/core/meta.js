(function () {
  function collectMeta() {
    const result = {
      ts: Date.now(),
      env: {},
      webgl: null,
      canvas: null,
      storage: {},
    };

    try {
      result.env = {
        ua: navigator.userAgent || null,
        lang: navigator.language || null,
        langs: navigator.languages || null,
        tz: Intl.DateTimeFormat().resolvedOptions().timeZone || null,
        screen: {
          w: screen.width,
          h: screen.height,
          aw: screen.availWidth,
          ah: screen.availHeight,
          dpr: window.devicePixelRatio || 1,
        },
      };
    } catch {}

    try {
      const canvas = document.createElement("canvas");
      const gl =
        canvas.getContext("webgl") ||
        canvas.getContext("experimental-webgl");

      if (gl) {
        const dbg = gl.getExtension("WEBGL_debug_renderer_info");
        result.webgl = {
          vendor: dbg
            ? gl.getParameter(dbg.UNMASKED_VENDOR_WEBGL)
            : null,
          renderer: dbg
            ? gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL)
            : null,
          version: gl.getParameter(gl.VERSION),
          shading: gl.getParameter(gl.SHADING_LANGUAGE_VERSION),
        };
      }
    } catch {}

    try {
      const c = document.createElement("canvas");
      c.width = 200;
      c.height = 50;
      const ctx = c.getContext("2d");

      ctx.textBaseline = "top";
      ctx.font = "14px Arial";
      ctx.fillStyle = "#f60";
      ctx.fillRect(0, 0, 200, 50);
      ctx.fillStyle = "#069";
      ctx.fillText("challenge-fp", 2, 2);
      ctx.fillStyle = "rgba(102, 204, 0, 0.7)";
      ctx.fillText("challenge-fp", 4, 4);

      const data = c.toDataURL();
      let hash = 0;
      for (let i = 0; i < data.length; i++) {
        hash = ((hash << 5) - hash) + data.charCodeAt(i);
        hash |= 0;
      }

      result.canvas = {
        hash,
        len: data.length,
      };
    } catch {}

    /* =========================
     * Storage 可用性
     * ========================= */
    try {
      const k = "__fp_test__";
      localStorage.setItem(k, "1");
      localStorage.removeItem(k);
      result.storage.localStorage = true;
    } catch {
      result.storage.localStorage = false;
    }

    try {
      document.cookie = "fp_test=1;path=/";
      result.storage.cookie =
        document.cookie.indexOf("fp_test=") !== -1;
    } catch {
      result.storage.cookie = false;
    }

    window.__challenge_fp__ = result;
    return result;
  }

  function base64EncodeUtf8(value) {
    try {
      return btoa(unescape(encodeURIComponent(value)));
    } catch (err) {
      console.warn("Failed to base64 encode meta payload:", err);
      return "";
    }
  }

  function collectBase64() {
    const result = collectMeta();
    return base64EncodeUtf8(JSON.stringify(result));
  }

  function send(endpoint) {
    const payload = collectBase64();
    if (!payload) {
      return Promise.resolve();
    }
    console.log(payload);
    return fetch(endpoint || "/__cowcatwaf/challenge/fp", {
      method: "POST",
      body: payload,
      keepalive: true,
    });
  }

  window.__cowcat_meta__ = {
    collect: collectMeta,
    collectBase64: collectBase64,
    send: send,
  };
})();
  
