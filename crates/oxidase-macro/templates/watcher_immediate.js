(function() {
    const mod = window.__OXIDASE__?.modules?.["__MODULE_HASH__"];
    if (!mod) {
        console.error("[oxidase]: Module '__MODULE_HASH__' not found. Cannot start watcher.");
        dioxus.send({ __bindgen_err: "MODULE_NOT_FOUND" });
        return;
    }
    if (!window.__OXIDASE__) window.__OXIDASE__ = { modules: {}, watchers: new Map() };
    if (!window.__OXIDASE__.watchers) window.__OXIDASE__.watchers = new Map();
    const emit = (val) => dioxus.send(val);
    const payload = __PAYLOAD__;
    const cleanup = mod.__JS_NAME__(...payload, emit);
    window.__OXIDASE__.watchers.set(__SUB_ID__, cleanup);
})();
