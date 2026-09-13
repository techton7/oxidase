(function() {
    if (!window.__OXIDASE__?.query) {
        dioxus.send({ ok: false, error: "MODULE_UNAVAILABLE" });
        return;
    }
    window.__OXIDASE__.query("__MODULE_HASH__", "__JS_NAME__", __PAYLOAD__, dioxus.send, true);
})();
