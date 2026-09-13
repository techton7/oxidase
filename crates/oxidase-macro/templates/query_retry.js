(async function() {
    const mod = window.__OXIDASE__?.modules?.["__MODULE_HASH__"];
    if (!mod) {
        dioxus.send({ ok: false, error: "MODULE_UNAVAILABLE" });
        return;
    }
    try {
        const payload = __PAYLOAD__;
        const result = await mod.__JS_NAME__(...payload);
        dioxus.send({ ok: true, data: result });
    } catch (err) {
        dioxus.send({ ok: false, error: err.message || String(err), stack: err.stack });
    }
})();
