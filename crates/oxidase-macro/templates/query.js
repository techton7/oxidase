(async function() {
    if (!window.__OXIDASE__?.query) {
        dioxus.send({ ok: false, error: "MODULE_NOT_FOUND" });
        return;
    }
    await window.__OXIDASE__.query("__MODULE_HASH__", "__JS_NAME__", __PAYLOAD__, (val) => dioxus.send(val), false);
})();

