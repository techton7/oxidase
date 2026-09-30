(async function() {
    if (!window.__OXIDASE__?.query) {
        dioxus.send({ ok: false, error: "MODULE_UNAVAILABLE" });
        return;
    }
    await window.__OXIDASE__.query("__MODULE_HASH__", "__JS_NAME__", __PAYLOAD__, (val) => dioxus.send(val), true);
})();

