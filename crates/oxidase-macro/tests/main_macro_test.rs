use oxidase_macro::main;

// Mock the oxidase namespace so the rewritten call resolves in this test crate
mod oxidase {
    use std::sync::atomic::{AtomicBool, Ordering};
    pub static CALLED: AtomicBool = AtomicBool::new(false);
    pub fn launch<F>(_app: F) {
        CALLED.store(true, Ordering::SeqCst);
    }
}

#[allow(dead_code)]
mod dioxus {
    pub fn launch<F>(_app: F) {
        panic!("dioxus::launch should have been rewritten to ::oxidase::launch!");
    }
}

#[test]
fn test_main_macro_compiles_without_launch() {
    #[main]
    fn plain_main() {
        println!("no launch statement");
    }
    plain_main();
}

#[test]
fn test_main_macro_rewrites_dioxus_launch() {
    fn dummy_app() {}

    #[main]
    fn run_app() {
        let _prefix = 42;
        dioxus::launch(dummy_app);
    }

    run_app();
    assert!(oxidase::CALLED.load(std::sync::atomic::Ordering::SeqCst));
}
