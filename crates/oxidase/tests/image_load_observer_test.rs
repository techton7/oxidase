use oxidase::dom::{Document, ImageLoadOptions, ImageLoadStatus, watch_image_load};
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(not(feature = "native"))]
#[test]
fn mock_image_observation_supports_status_cache_replacement_and_cleanup() {
    assert_eq!(ImageLoadStatus::default(), ImageLoadStatus::Loading);
    assert_eq!(
        ImageLoadOptions::default(),
        ImageLoadOptions {
            crossorigin: None,
            referrerpolicy: None,
        }
    );

    let document = Document::mock();
    document.clear_image_load_status("asset.svg");
    let statuses = Rc::new(RefCell::new(Vec::new()));
    let first_statuses = statuses.clone();
    let guard = watch_image_load(
        "asset.svg",
        ImageLoadOptions {
            crossorigin: Some("anonymous".into()),
            referrerpolicy: Some("no-referrer".into()),
        },
        move |status| first_statuses.borrow_mut().push(status),
    )
    .expect("mock watcher");
    assert_eq!(&*statuses.borrow(), &[ImageLoadStatus::Loading]);

    document.set_image_load_status("asset.svg", ImageLoadStatus::Loaded);
    assert_eq!(
        &*statuses.borrow(),
        &[ImageLoadStatus::Loading, ImageLoadStatus::Loaded]
    );

    let cached = Rc::new(RefCell::new(Vec::new()));
    let cached_statuses = cached.clone();
    let cached_guard = watch_image_load("asset.svg", ImageLoadOptions::default(), move |status| {
        cached_statuses.borrow_mut().push(status)
    })
    .expect("cached mock watcher");
    assert_eq!(&*cached.borrow(), &[ImageLoadStatus::Loaded]);

    document.set_image_load_status("asset.svg", ImageLoadStatus::Loading);
    document.set_image_load_status("asset.svg", ImageLoadStatus::Error);
    assert_eq!(
        cached.borrow().last().copied(),
        Some(ImageLoadStatus::Error)
    );

    drop(guard);
    let previous = statuses.borrow().clone();
    document.set_image_load_status("asset.svg", ImageLoadStatus::Loaded);
    assert_eq!(*statuses.borrow(), previous);
    drop(cached_guard);
    let previous = cached.borrow().clone();
    document.set_image_load_status("asset.svg", ImageLoadStatus::Loaded);
    assert_eq!(*cached.borrow(), previous);
    document.clear_image_load_status("asset.svg");
}

#[cfg(feature = "native")]
mod native {
    use super::*;
    use blitz_dom::{BaseDocument, DocumentConfig};
    use blitz_traits::net::{Bytes, NetHandler, NetProvider, Request};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct ManualNetProvider {
        requests: Mutex<Vec<(String, Box<dyn NetHandler>)>>,
    }

    impl NetProvider for ManualNetProvider {
        fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
            self.requests
                .lock()
                .unwrap()
                .push((request.url.to_string(), handler));
        }
    }

    const PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 4,
        0, 0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 100, 248, 15, 0, 1, 5,
        1, 1, 39, 24, 227, 102, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    const SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect width="1" height="1"/></svg>"#;

    fn take_request(
        provider: &ManualNetProvider,
        suffix: &str,
    ) -> Box<dyn NetHandler> {
        let mut requests = provider.requests.lock().unwrap();
        let index = requests
            .iter()
            .position(|(url, _)| url.ends_with(suffix))
            .unwrap_or_else(|| panic!("missing image request ending in {suffix}"));
        requests.remove(index).1
    }

    #[test]
    fn native_image_observation_uses_real_resource_pipeline_and_cache() {
        let provider = Arc::new(ManualNetProvider::default());
        let base = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig {
            base_url: Some("https://example.test/".into()),
            net_provider: Some(provider.clone()),
            ..Default::default()
        })));
        let document = Document::from_base(base.clone());

        Document::with_current(document.clone(), || {
            let statuses = Rc::new(RefCell::new(Vec::new()));
            let callback_statuses = statuses.clone();
            let guard = watch_image_load("first.png", ImageLoadOptions::default(), move |status| {
                callback_statuses.borrow_mut().push(status)
            })
            .expect("native watcher");

            let request = take_request(&provider, "first.png");
            request.bytes(
                "https://example.test/first.png".into(),
                Bytes::copy_from_slice(PNG),
            );
            base.borrow_mut().resolve(0.0);
            document.pump_hosted_observers();
            assert_eq!(
                &*statuses.borrow(),
                &[ImageLoadStatus::Loading, ImageLoadStatus::Loaded]
            );

            let cached = Rc::new(RefCell::new(Vec::new()));
            let callback_cached = cached.clone();
            let cached_guard =
                watch_image_load("first.png", ImageLoadOptions::default(), move |status| {
                    callback_cached.borrow_mut().push(status)
                })
                .expect("cached watcher");
            document.pump_hosted_observers();
            assert_eq!(
                &*cached.borrow(),
                &[ImageLoadStatus::Loading, ImageLoadStatus::Loaded]
            );
            assert!(provider.requests.lock().unwrap().is_empty());

            drop(guard);
            drop(cached_guard);

            let errors = Rc::new(RefCell::new(Vec::new()));
            let callback_errors = errors.clone();
            let error_guard =
                watch_image_load("missing.svg", ImageLoadOptions::default(), move |status| {
                    callback_errors.borrow_mut().push(status)
                })
                .expect("error watcher");
            let request = take_request(&provider, "missing.svg");
            request.error(
                "https://example.test/missing.svg".into(),
                "not found".into(),
            );
            base.borrow_mut().resolve(0.0);
            document.pump_hosted_observers();
            assert_eq!(
                &*errors.borrow(),
                &[ImageLoadStatus::Loading, ImageLoadStatus::Error]
            );
            drop(error_guard);

            let svg = Rc::new(RefCell::new(Vec::new()));
            let svg_statuses = svg.clone();
            let svg_guard = watch_image_load(
                "vector.svg",
                ImageLoadOptions::default(),
                move |status| svg_statuses.borrow_mut().push(status),
            )
            .expect("SVG watcher");
            take_request(&provider, "vector.svg").bytes(
                "https://example.test/vector.svg".into(),
                Bytes::copy_from_slice(SVG),
            );
            base.borrow_mut().resolve(0.0);
            document.pump_hosted_observers();
            assert_eq!(
                &*svg.borrow(),
                &[ImageLoadStatus::Loading, ImageLoadStatus::Loaded]
            );
            drop(svg_guard);

            let data_url = "data:image/png;base64,ignored";
            let data = Rc::new(RefCell::new(Vec::new()));
            let data_statuses = data.clone();
            let data_guard = watch_image_load(
                data_url,
                ImageLoadOptions::default(),
                move |status| data_statuses.borrow_mut().push(status),
            )
            .expect("data URL watcher");
            take_request(&provider, "ignored").bytes(
                data_url.into(),
                Bytes::copy_from_slice(PNG),
            );
            base.borrow_mut().resolve(0.0);
            document.pump_hosted_observers();
            assert_eq!(
                &*data.borrow(),
                &[ImageLoadStatus::Loading, ImageLoadStatus::Loaded]
            );
            drop(data_guard);

            let replacement = Rc::new(RefCell::new(Vec::new()));
            let replacement_statuses = replacement.clone();
            let replacement_guard = watch_image_load(
                "before.svg",
                ImageLoadOptions::default(),
                move |status| replacement_statuses.borrow_mut().push(status),
            )
            .expect("replacement watcher");
            let preload_id = base
                .borrow()
                .root_node()
                .children
                .last()
                .copied()
                .expect("preloader node");
            base.borrow_mut().mutate().set_attribute(
                preload_id,
                blitz_dom::QualName::new(
                    None,
                    blitz_dom::ns!(),
                    blitz_dom::local_name!("src"),
                ),
                "after.svg",
            );
            take_request(&provider, "before.svg").bytes(
                "https://example.test/before.svg".into(),
                Bytes::copy_from_slice(SVG),
            );
            base.borrow_mut().resolve(0.0);
            document.pump_hosted_observers();
            assert_eq!(&*replacement.borrow(), &[ImageLoadStatus::Loading]);
            take_request(&provider, "after.svg").bytes(
                "https://example.test/after.svg".into(),
                Bytes::copy_from_slice(SVG),
            );
            base.borrow_mut().resolve(0.0);
            document.pump_hosted_observers();
            assert_eq!(
                &*replacement.borrow(),
                &[ImageLoadStatus::Loading, ImageLoadStatus::Loaded]
            );
            drop(replacement_guard);

            let dropped = Rc::new(RefCell::new(Vec::new()));
            let dropped_statuses = dropped.clone();
            let dropped_guard = watch_image_load(
                "dropped.png",
                ImageLoadOptions::default(),
                move |status| dropped_statuses.borrow_mut().push(status),
            )
            .expect("dropped watcher");
            drop(dropped_guard);
            take_request(&provider, "dropped.png").bytes(
                "https://example.test/dropped.png".into(),
                Bytes::copy_from_slice(PNG),
            );
            base.borrow_mut().resolve(0.0);
            document.pump_hosted_observers();
            assert_eq!(&*dropped.borrow(), &[ImageLoadStatus::Loading]);
        });
    }

    #[test]
    fn native_events_are_bound_to_their_originating_document() {
        let first_provider = Arc::new(ManualNetProvider::default());
        let second_provider = Arc::new(ManualNetProvider::default());
        let first_base = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig {
            base_url: Some("https://first.test/".into()),
            net_provider: Some(first_provider.clone()),
            ..Default::default()
        })));
        let second_base = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig {
            base_url: Some("https://second.test/".into()),
            net_provider: Some(second_provider.clone()),
            ..Default::default()
        })));
        let first_document = Document::from_base(first_base.clone());
        let second_document = Document::from_base(second_base.clone());
        let first = Rc::new(RefCell::new(Vec::new()));
        let second = Rc::new(RefCell::new(Vec::new()));

        let first_callback = first.clone();
        let first_guard = Document::with_current(first_document.clone(), || {
            watch_image_load("same.png", ImageLoadOptions::default(), move |status| {
                first_callback.borrow_mut().push(status)
            })
            .expect("first document watcher")
        });
        let second_callback = second.clone();
        let second_guard = Document::with_current(second_document.clone(), || {
            watch_image_load("same.png", ImageLoadOptions::default(), move |status| {
                second_callback.borrow_mut().push(status)
            })
            .expect("second document watcher")
        });

        take_request(&first_provider, "same.png").bytes(
            "https://first.test/same.png".into(),
            Bytes::copy_from_slice(PNG),
        );
        first_base.borrow_mut().resolve(0.0);
        first_document.pump_hosted_observers();
        assert_eq!(
            &*first.borrow(),
            &[ImageLoadStatus::Loading, ImageLoadStatus::Loaded]
        );
        assert_eq!(&*second.borrow(), &[ImageLoadStatus::Loading]);
        first_document.pump_hosted_observers();
        assert_eq!(
            &*first.borrow(),
            &[ImageLoadStatus::Loading, ImageLoadStatus::Loaded]
        );

        drop(first_guard);
        drop(second_guard);
    }
}
