(function() {
    if (!window.__OXIDASE__) window.__OXIDASE__ = { modules: {}, watchers: new Map() };
    if (!window.__OXIDASE__.modules) window.__OXIDASE__.modules = {};
    if (!window.__OXIDASE__.watchers) window.__OXIDASE__.watchers = new Map();

    if (!window.__OXIDASE__.command) {
        window.__OXIDASE__.command = function(modHash, fnName, payload) {
            const mod = window.__OXIDASE__.modules[modHash];
            if (!mod) {
                console.warn("[oxidase]: Module '" + modHash + "' not found. Browser context may have reloaded.");
                return;
            }
            try {
                mod[fnName](...payload);
            } catch (e) {
                console.error("[oxidase Command Error in " + fnName + "]:", e);
            }
        };
    }

    if (!window.__OXIDASE__.query) {
        window.__OXIDASE__.query = async function(modHash, fnName, payload, send, isRetry) {
            const mod = window.__OXIDASE__.modules[modHash];
            if (!mod) {
                send({ ok: false, error: isRetry ? "MODULE_UNAVAILABLE" : "MODULE_NOT_FOUND" });
                return;
            }
            try {
                const result = await mod[fnName](...payload);
                send({ ok: true, data: result });
            } catch (err) {
                send({ ok: false, error: err?.message || String(err), stack: err?.stack });
            }
        };
    }

    if (!window.__OXIDASE__.watch) {
        window.__OXIDASE__.watch = function(mode, modHash, fnName, payload, subId, send) {
            const mod = window.__OXIDASE__.modules[modHash];
            if (!mod) {
                console.error("[oxidase]: Module '" + modHash + "' not found. Cannot start watcher.");
                send({ __bindgen_err: "MODULE_NOT_FOUND" });
                return;
            }
            let emit = send;
            let cancelTimer = null;

            if (mode === "raf") {
                let pending = null;
                let rafId = null;
                const hasRaf = typeof window !== "undefined" && typeof window.requestAnimationFrame === "function";
                emit = function(val) {
                    pending = val;
                    if (rafId === null) {
                        if (hasRaf) {
                            rafId = window.requestAnimationFrame(function() {
                                rafId = null;
                                send(pending);
                            });
                        } else {
                            rafId = setTimeout(function() {
                                rafId = null;
                                send(pending);
                            }, 16);
                        }
                    }
                };
                cancelTimer = function() {
                    if (rafId !== null) {
                        if (hasRaf) {
                            window.cancelAnimationFrame(rafId);
                        } else {
                            clearTimeout(rafId);
                        }
                        rafId = null;
                    }
                };
            }

            const rawCleanup = mod[fnName](...payload, emit);
            const cleanup = function() {
                if (cancelTimer) cancelTimer();
                if (typeof rawCleanup === "function") {
                    rawCleanup();
                }
            };
            window.__OXIDASE__.watchers.set(subId, cleanup);
        };
    }
    if (!window.__OXIDASE__.modules["__MODULE_HASH__"]) {
        window.__OXIDASE__.modules["__MODULE_HASH__"] = (function() {
            __INLINED_JS__
            return { __EXPORT_KEYS__ };
        })();
    }
})();
