(function() {
    if (!window.__OXIDASE__?.watch) {
        dioxus.send({ __bindgen_err: "MODULE_NOT_FOUND" });
        return;
    }
    window.__OXIDASE__.watch("raf", "__MODULE_HASH__", "__JS_NAME__", __PAYLOAD__, __SUB_ID__, dioxus.send);
})();
