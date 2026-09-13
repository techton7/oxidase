(function() {
    const mod = window.__OXIDASE__?.modules?.["__MODULE_HASH__"];
    if (!mod) {
        console.error("[oxidase]: Module '__MODULE_HASH__' not found. Cannot start watcher.");
        dioxus.send({ __bindgen_err: "MODULE_NOT_FOUND" });
        return;
    }
    if (!window.__OXIDASE__) window.__OXIDASE__ = { modules: {}, watchers: new Map() };
    if (!window.__OXIDASE__.watchers) window.__OXIDASE__.watchers = new Map();
    let __raf_pending = null;
    let __raf_id = null;
    const emit = (val) => {
        __raf_pending = val;
        if (__raf_id === null) {
            __raf_id = requestAnimationFrame(() => {
                __raf_id = null;
                dioxus.send(__raf_pending);
            });
        }
    };
    const payload = __PAYLOAD__;
    const rawCleanup = mod.__JS_NAME__(...payload, emit);
    const cleanup = () => {
        if (__raf_id !== null) {
            cancelAnimationFrame(__raf_id);
            __raf_id = null;
        }
        if (typeof rawCleanup === "function") {
            rawCleanup();
        }
    };
    window.__OXIDASE__.watchers.set(__SUB_ID__, cleanup);
})();
