use blitz_dom::{Attribute, BaseDocument, DocumentConfig, NodeId, QualName, local_name, ns};
use oxidase_floating_ui::compute_position;
use oxidase_floating_ui::geometry::{ElementOrVirtual, Placement};
use oxidase_floating_ui::middleware::{
    Flip, FlipOptions, Offset, OffsetOptions, OffsetOptionsValues, Shift, ShiftOptions,
};
use oxidase_floating_ui::platform::NativeFloatingPlatform;
use oxidase_floating_ui::types::ComputePositionConfig;

fn setup_test_doc(
    window_size: (u32, u32),
    ref_style: &str,
    float_style: &str,
) -> (BaseDocument, NodeId, NodeId) {
    let mut doc = BaseDocument::new(DocumentConfig::default());
    {
        let mut vp = doc.viewport_mut();
        vp.window_size = window_size;
        vp.hidpi_scale = 1.0;
    }
    let doc_root = doc.root_node().id;
    let root_id = {
        let mut mutator = doc.mutate();
        let html_id = mutator.create_element(
            QualName::new(None, ns!(), local_name!("html")),
            vec![
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("id")),
                    value: "app-root".into(),
                },
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("style")),
                    value: format!(
                        "width: {}px; height: {}px; margin: 0; padding: 0; position: relative; display: block;",
                        window_size.0, window_size.1
                    ),
                },
            ],
        );
        mutator.append_children(doc_root, &[html_id]);
        html_id
    };

    let ref_id = {
        let mut mutator = doc.mutate();
        let rid = mutator.create_element(
            QualName::new(None, ns!(), local_name!("button")),
            vec![
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("id")),
                    value: "ref".into(),
                },
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("style")),
                    value: ref_style.into(),
                },
            ],
        );
        mutator.append_children(root_id, &[rid]);
        rid
    };

    let float_id = {
        let mut mutator = doc.mutate();
        let fid = mutator.create_element(
            QualName::new(None, ns!(), local_name!("div")),
            vec![
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("id")),
                    value: "float".into(),
                },
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("style")),
                    value: float_style.into(),
                },
            ],
        );
        mutator.append_children(root_id, &[fid]);
        fid
    };

    doc.resolve(0.0);
    (doc, ref_id, float_id)
}

#[test]
fn test_platform_base_placements() {
    // Reference: x=200, y=200, w=100, h=50
    // Floating: w=60, h=40
    let (doc, ref_id, float_id) = setup_test_doc(
        (1000, 800),
        "position: absolute; left: 200px; top: 200px; width: 100px; height: 50px; display: block;",
        "position: absolute; left: 0px; top: 0px; width: 60px; height: 40px; display: block;",
    );

    let platform = NativeFloatingPlatform::from_base(doc);

    // 1. Bottom: x = 200 + (100 - 60)/2 = 220, y = 200 + 50 = 250
    let res_bottom = compute_position(
        ElementOrVirtual::Element(&ref_id),
        &float_id,
        ComputePositionConfig::new(&platform).placement(Placement::Bottom),
    );
    assert_eq!(res_bottom.placement, Placement::Bottom);
    assert_eq!(res_bottom.x, 220.0);
    assert_eq!(res_bottom.y, 250.0);

    // 2. Top: x = 220, y = 200 - 40 = 160
    let res_top = compute_position(
        ElementOrVirtual::Element(&ref_id),
        &float_id,
        ComputePositionConfig::new(&platform).placement(Placement::Top),
    );
    assert_eq!(res_top.placement, Placement::Top);
    assert_eq!(res_top.x, 220.0);
    assert_eq!(res_top.y, 160.0);

    // 3. Left: x = 200 - 60 = 140, y = 200 + (50 - 40)/2 = 205
    let res_left = compute_position(
        ElementOrVirtual::Element(&ref_id),
        &float_id,
        ComputePositionConfig::new(&platform).placement(Placement::Left),
    );
    assert_eq!(res_left.placement, Placement::Left);
    assert_eq!(res_left.x, 140.0);
    assert_eq!(res_left.y, 205.0);

    // 4. Right: x = 200 + 100 = 300, y = 205
    let res_right = compute_position(
        ElementOrVirtual::Element(&ref_id),
        &float_id,
        ComputePositionConfig::new(&platform).placement(Placement::Right),
    );
    assert_eq!(res_right.placement, Placement::Right);
    assert_eq!(res_right.x, 300.0);
    assert_eq!(res_right.y, 205.0);
}

#[test]
fn test_platform_offset_middleware() {
    // Reference: x=200, y=200, w=100, h=50
    // Floating: w=60, h=40
    let (doc, ref_id, float_id) = setup_test_doc(
        (1000, 800),
        "position: absolute; left: 200px; top: 200px; width: 100px; height: 50px; display: block;",
        "position: absolute; left: 0px; top: 0px; width: 60px; height: 40px; display: block;",
    );

    let platform = NativeFloatingPlatform::from_base(doc);

    // Offset: main_axis = 15.0, cross_axis = 10.0
    let offset_mw = Offset::new(OffsetOptions::Values(
        OffsetOptionsValues::default()
            .main_axis(15.0)
            .cross_axis(10.0),
    ));

    // Bottom: base = (220, 250) -> main_axis adds to Y, cross_axis adds to X
    let res_bottom = compute_position(
        ElementOrVirtual::Element(&ref_id),
        &float_id,
        ComputePositionConfig::new(&platform)
            .placement(Placement::Bottom)
            .middleware(vec![Box::new(offset_mw.clone())]),
    );
    assert_eq!(res_bottom.x, 230.0); // 220 + 10
    assert_eq!(res_bottom.y, 265.0); // 250 + 15

    // Top: base = (220, 160) -> main_axis subtracts from Y, cross_axis adds to X
    let res_top = compute_position(
        ElementOrVirtual::Element(&ref_id),
        &float_id,
        ComputePositionConfig::new(&platform)
            .placement(Placement::Top)
            .middleware(vec![Box::new(offset_mw.clone())]),
    );
    assert_eq!(res_top.x, 230.0); // 220 + 10
    assert_eq!(res_top.y, 145.0); // 160 - 15

    // Right: base = (300, 205) -> main_axis adds to X, cross_axis adds to Y
    let res_right = compute_position(
        ElementOrVirtual::Element(&ref_id),
        &float_id,
        ComputePositionConfig::new(&platform)
            .placement(Placement::Right)
            .middleware(vec![Box::new(offset_mw.clone())]),
    );
    assert_eq!(res_right.x, 315.0); // 300 + 15
    assert_eq!(res_right.y, 215.0); // 205 + 10

    // Left: base = (140, 205) -> main_axis subtracts from X, cross_axis adds to Y
    let res_left = compute_position(
        ElementOrVirtual::Element(&ref_id),
        &float_id,
        ComputePositionConfig::new(&platform)
            .placement(Placement::Left)
            .middleware(vec![Box::new(offset_mw)]),
    );
    assert_eq!(res_left.x, 125.0); // 140 - 15
    assert_eq!(res_left.y, 215.0); // 205 + 10
}

#[test]
fn test_platform_flip_middleware() {
    // Reference placed near the top boundary: y=10.
    // Floating height = 40. Top placement would yield y = 10 - 40 = -30 (overflow < 0).
    // Viewport: 800 x 600.
    let (doc, ref_id, float_id) = setup_test_doc(
        (800, 600),
        "position: absolute; left: 200px; top: 10px; width: 100px; height: 50px; display: block;",
        "position: absolute; left: 0px; top: 0px; width: 60px; height: 40px; display: block;",
    );

    let platform = NativeFloatingPlatform::from_base(doc);

    let flip_mw = Flip::new(FlipOptions::default());

    // Request Top placement -> must flip to Bottom
    let res = compute_position(
        ElementOrVirtual::Element(&ref_id),
        &float_id,
        ComputePositionConfig::new(&platform)
            .placement(Placement::Top)
            .middleware(vec![Box::new(flip_mw)]),
    );

    assert_eq!(
        res.placement,
        Placement::Bottom,
        "Top placement overflowing boundary must flip to Bottom"
    );
    assert_eq!(res.x, 220.0);
    assert_eq!(res.y, 60.0); // 10 + 50
}

#[test]
fn test_platform_shift_middleware() {
    // Reference placed near the left boundary: x=5, w=30.
    // Floating width = 80.
    // Placement::Bottom center alignment gives: x = 5 + (30 - 80)/2 = -20 (overflows left < 0).
    // Viewport: 800 x 600.
    let (doc, ref_id, float_id) = setup_test_doc(
        (800, 600),
        "position: absolute; left: 5px; top: 100px; width: 30px; height: 40px; display: block;",
        "position: absolute; left: 0px; top: 0px; width: 80px; height: 50px; display: block;",
    );

    let platform = NativeFloatingPlatform::from_base(doc);

    // Shift without limiter or with default limiter
    let shift_mw = Shift::new(ShiftOptions::default());

    let res = compute_position(
        ElementOrVirtual::Element(&ref_id),
        &float_id,
        ComputePositionConfig::new(&platform)
            .placement(Placement::Bottom)
            .middleware(vec![Box::new(shift_mw)]),
    );

    assert_eq!(
        res.placement,
        Placement::Bottom,
        "Shift must preserve placement"
    );
    assert_eq!(
        res.x, 0.0,
        "Shift must clamp overflowing x to viewport left boundary 0.0"
    );
    assert_eq!(res.y, 140.0); // 100 + 40
}

#[test]
fn test_platform_composed_pipeline() {
    // Reference near top-left: x=5, y=15, w=40, h=50.
    // Floating: w=80, h=40.
    // Preferred placement: Top.
    // - Offset main_axis = 10: Top would be y = 15 - 40 - 10 = -35.
    // - Flip detects top overflow (-35 < 0) -> inverts to Bottom: y = 15 + 50 + 10 = 75.
    // - Bottom center x = 5 + (40 - 80)/2 = -15.
    // - Shift detects left overflow (-15 < 0) -> clamps x to 0.
    let (doc, ref_id, float_id) = setup_test_doc(
        (800, 600),
        "position: absolute; left: 5px; top: 15px; width: 40px; height: 50px; display: block;",
        "position: absolute; left: 0px; top: 0px; width: 80px; height: 40px; display: block;",
    );

    let platform = NativeFloatingPlatform::from_base(doc);

    let offset_mw = Offset::new(OffsetOptions::Values(
        OffsetOptionsValues::default().main_axis(10.0),
    ));
    let flip_mw = Flip::new(FlipOptions::default());
    let shift_mw = Shift::new(ShiftOptions::default());

    let res = compute_position(
        ElementOrVirtual::Element(&ref_id),
        &float_id,
        ComputePositionConfig::new(&platform)
            .placement(Placement::Top)
            .middleware(vec![
                Box::new(offset_mw),
                Box::new(flip_mw),
                Box::new(shift_mw),
            ]),
    );

    assert_eq!(
        res.placement,
        Placement::Bottom,
        "Pipeline must flip Top to Bottom when overflowing top"
    );
    assert_eq!(
        res.y, 75.0,
        "Pipeline y must reflect flipped bottom placement + main_axis offset"
    );
    assert_eq!(
        res.x, 0.0,
        "Pipeline x must be shifted into view at viewport boundary"
    );
}

#[test]
fn test_double_correction_invariants() {
    use oxidase_floating_ui::compute_coords_from_placement;
    use oxidase_floating_ui::geometry::{ElementRects, Rect, Strategy};
    use oxidase_floating_ui::types::{FloatingPlatform, GetElementRectsArgs, Platform};

    // Invariant 1: compute_coords_from_placement produces identical results regardless of floating.x / floating.y input.
    let reference = Rect {
        x: 150.0,
        y: 220.0,
        width: 100.0,
        height: 40.0,
    };

    let all_placements = [
        Placement::Top,
        Placement::TopStart,
        Placement::TopEnd,
        Placement::Right,
        Placement::RightStart,
        Placement::RightEnd,
        Placement::Bottom,
        Placement::BottomStart,
        Placement::BottomEnd,
        Placement::Left,
        Placement::LeftStart,
        Placement::LeftEnd,
    ];

    let rtl_options = [None, Some(false), Some(true)];

    let floating_variations = [
        Rect {
            x: 0.0,
            y: 0.0,
            width: 80.0,
            height: 60.0,
        },
        Rect {
            x: 50.0,
            y: 120.0,
            width: 80.0,
            height: 60.0,
        },
        Rect {
            x: -300.0,
            y: -450.0,
            width: 80.0,
            height: 60.0,
        },
        Rect {
            x: 12345.0,
            y: -98765.0,
            width: 80.0,
            height: 60.0,
        },
    ];

    for &placement in &all_placements {
        for &rtl in &rtl_options {
            let canonical_rects = ElementRects {
                reference,
                floating: floating_variations[0],
            };
            let canonical_coords = compute_coords_from_placement(&canonical_rects, placement, rtl);

            for &varied_floating in &floating_variations[1..] {
                let varied_rects = ElementRects {
                    reference,
                    floating: varied_floating,
                };
                let varied_coords = compute_coords_from_placement(&varied_rects, placement, rtl);
                assert_eq!(
                    canonical_coords,
                    varied_coords,
                    "compute_coords_from_placement must ignore floating.x/y for placement={:?}, rtl={:?}: expected {:?}, got {:?}",
                    placement,
                    rtl,
                    canonical_coords,
                    varied_coords
                );
            }
        }
    }

    // Invariant 2: NativeFloatingPlatform::get_element_rects produces floating.x == 0.0 and floating.y == 0.0 for Strategy::Fixed
    let (doc, ref_id, float_id) = setup_test_doc(
        (1000, 800),
        "position: absolute; left: 150px; top: 220px; width: 100px; height: 40px; display: block;",
        "position: fixed; left: 400px; top: 600px; width: 80px; height: 60px; display: block;",
    );

    let platform = NativeFloatingPlatform::from_base(doc);

    // 2a. FloatingPlatform trait
    let rects_from_fp = FloatingPlatform::get_element_rects(&platform, "ref", "float")
        .expect("FloatingPlatform::get_element_rects must succeed");
    assert_eq!(
        rects_from_fp.floating.x, 0.0,
        "FloatingPlatform floating.x must be 0.0 to prevent double-correction under Strategy::Fixed"
    );
    assert_eq!(
        rects_from_fp.floating.y, 0.0,
        "FloatingPlatform floating.y must be 0.0 to prevent double-correction under Strategy::Fixed"
    );
    assert_eq!(rects_from_fp.floating.width, 80.0);
    assert_eq!(rects_from_fp.floating.height, 60.0);

    // 2b. Platform<NodeId, ()> trait
    let rects_from_node_platform = Platform::<NodeId, ()>::get_element_rects(
        &platform,
        GetElementRectsArgs {
            reference: ElementOrVirtual::Element(&ref_id),
            floating: &float_id,
            strategy: Strategy::Fixed,
        },
    );
    assert_eq!(
        rects_from_node_platform.floating.x, 0.0,
        "Platform<NodeId> floating.x must be 0.0 to prevent double-correction under Strategy::Fixed"
    );
    assert_eq!(
        rects_from_node_platform.floating.y, 0.0,
        "Platform<NodeId> floating.y must be 0.0 to prevent double-correction under Strategy::Fixed"
    );
    assert_eq!(rects_from_node_platform.floating.width, 80.0);
    assert_eq!(rects_from_node_platform.floating.height, 60.0);

    // 2c. Platform<String, ()> trait
    let rects_from_string_platform = Platform::<String, ()>::get_element_rects(
        &platform,
        GetElementRectsArgs {
            reference: ElementOrVirtual::Element(&"ref".to_string()),
            floating: &"float".to_string(),
            strategy: Strategy::Fixed,
        },
    );
    assert_eq!(
        rects_from_string_platform.floating.x, 0.0,
        "Platform<String> floating.x must be 0.0 to prevent double-correction under Strategy::Fixed"
    );
    assert_eq!(
        rects_from_string_platform.floating.y, 0.0,
        "Platform<String> floating.y must be 0.0 to prevent double-correction under Strategy::Fixed"
    );
    assert_eq!(rects_from_string_platform.floating.width, 80.0);
    assert_eq!(rects_from_string_platform.floating.height, 60.0);
}

// FM-OX-3: VirtualElement (context_element: None & Some) + HideStrategy::{ReferenceHidden, Escaped}
// on both Platform<String, ()> and Platform<NodeId, ()>.
#[test]
fn test_fm_ox_3_virtual_element_hide_reference_hidden_and_escaped() {
    use oxidase_floating_ui::geometry::{ClientRectObject, DefaultVirtualElement, Rect, Strategy};
    use oxidase_floating_ui::middleware::{HIDE_NAME, Hide, HideData, HideOptions, HideStrategy};
    use std::rc::Rc;

    let mut doc = BaseDocument::new(DocumentConfig::default());
    {
        let mut vp = doc.viewport_mut();
        vp.window_size = (800, 600);
        vp.hidpi_scale = 1.0;
    }
    let doc_root = doc.root_node().id;
    let html_id = {
        let mut mutator = doc.mutate();
        let html = mutator.create_element(
            QualName::new(None, ns!(), local_name!("html")),
            vec![
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("id")),
                    value: "app-root".into(),
                },
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("style")),
                    value: "width: 800px; height: 600px; margin: 0; padding: 0; position: relative; display: block;".into(),
                },
            ],
        );
        mutator.append_children(doc_root, &[html]);
        html
    };

    // Scroll container occupying (0, 0, 200, 200) inside the 800x600 viewport.
    let scroll_id = {
        let mut mutator = doc.mutate();
        let sc = mutator.create_element(
            QualName::new(None, ns!(), local_name!("div")),
            vec![
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("id")),
                    value: "scroll-clip".into(),
                },
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("style")),
                    value: "position: absolute; left: 0px; top: 0px; width: 200px; height: 200px; overflow: scroll; display: block;".into(),
                },
            ],
        );
        mutator.append_children(html_id, &[sc]);
        sc
    };

    // Context element and floating element inside the 200x200 scroll container.
    let ctx_node_id = {
        let mut mutator = doc.mutate();
        let cid = mutator.create_element(
            QualName::new(None, ns!(), local_name!("div")),
            vec![
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("id")),
                    value: "ctx-el".into(),
                },
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("style")),
                    value: "position: absolute; left: 10px; top: 10px; width: 20px; height: 20px; display: block;".into(),
                },
            ],
        );
        mutator.append_children(scroll_id, &[cid]);
        cid
    };

    let float_node_id = {
        let mut mutator = doc.mutate();
        let fid = mutator.create_element(
            QualName::new(None, ns!(), local_name!("div")),
            vec![
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("id")),
                    value: "float".into(),
                },
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("style")),
                    value: "position: absolute; left: 0px; top: 0px; width: 80px; height: 40px; display: block;".into(),
                },
            ],
        );
        mutator.append_children(scroll_id, &[fid]);
        fid
    };

    doc.resolve(0.0);
    let platform = NativeFloatingPlatform::from_base(doc);
    let float_str = "float".to_string();

    // 1. Virtual point inside viewport (at x=350, y=100, outside the 200x200 scroll container)
    //    with context_element: None -> must clip against viewport (not float's scroll container)
    //    and report reference_hidden = Some(false) and escaped = Some(false).
    let in_vp_rect = ClientRectObject::from(Rect::new(350.0, 100.0, 10.0, 10.0));
    let off_vp_rect = ClientRectObject::from(Rect::new(-120.0, 100.0, 10.0, 10.0));

    // 1a. Platform<String, ()> — in-viewport ReferenceHidden & Escaped
    let virt_str_in = DefaultVirtualElement::<String>::new(Rc::new({
        let r = in_vp_rect.clone();
        move || r.clone()
    }));
    let res_str_in_ref = compute_position::<String, ()>(
        ElementOrVirtual::VirtualElement(Box::new(virt_str_in.clone())),
        &float_str,
        ComputePositionConfig::new(&platform)
            .placement(Placement::Bottom)
            .strategy(Strategy::Fixed)
            .middleware(vec![Box::new(Hide::new(
                HideOptions::default().strategy(HideStrategy::ReferenceHidden),
            ))]),
    );
    let hide_str_in_ref: HideData = res_str_in_ref
        .middleware_data
        .get_as(HIDE_NAME)
        .expect("HideData present");
    assert_eq!(hide_str_in_ref.reference_hidden, Some(false));

    let res_str_in_esc = compute_position::<String, ()>(
        ElementOrVirtual::VirtualElement(Box::new(virt_str_in)),
        &float_str,
        ComputePositionConfig::new(&platform)
            .placement(Placement::Bottom)
            .strategy(Strategy::Fixed)
            .middleware(vec![Box::new(Hide::new(
                HideOptions::default().strategy(HideStrategy::Escaped),
            ))]),
    );
    let hide_str_in_esc: HideData = res_str_in_esc
        .middleware_data
        .get_as(HIDE_NAME)
        .expect("HideData present");
    assert_eq!(hide_str_in_esc.escaped, Some(false));

    // 1b. Platform<String, ()> — off-viewport ReferenceHidden & Escaped
    let virt_str_off = DefaultVirtualElement::<String>::new(Rc::new({
        let r = off_vp_rect.clone();
        move || r.clone()
    }));
    let res_str_off_ref = compute_position::<String, ()>(
        ElementOrVirtual::VirtualElement(Box::new(virt_str_off.clone())),
        &float_str,
        ComputePositionConfig::new(&platform)
            .placement(Placement::Bottom)
            .strategy(Strategy::Fixed)
            .middleware(vec![Box::new(Hide::new(
                HideOptions::default().strategy(HideStrategy::ReferenceHidden),
            ))]),
    );
    let hide_str_off_ref: HideData = res_str_off_ref
        .middleware_data
        .get_as(HIDE_NAME)
        .expect("HideData present");
    assert_eq!(hide_str_off_ref.reference_hidden, Some(true));

    let res_str_off_esc = compute_position::<String, ()>(
        ElementOrVirtual::VirtualElement(Box::new(virt_str_off)),
        &float_str,
        ComputePositionConfig::new(&platform)
            .placement(Placement::Bottom)
            .strategy(Strategy::Fixed)
            .middleware(vec![Box::new(Hide::new(
                HideOptions::default().strategy(HideStrategy::Escaped),
            ))]),
    );
    let hide_str_off_esc: HideData = res_str_off_esc
        .middleware_data
        .get_as(HIDE_NAME)
        .expect("HideData present");
    assert_eq!(hide_str_off_esc.escaped, Some(true));

    // 2a. Platform<NodeId, ()> — in-viewport ReferenceHidden & Escaped
    let virt_node_in = DefaultVirtualElement::<NodeId>::new(Rc::new({
        let r = in_vp_rect.clone();
        move || r.clone()
    }));
    let res_node_in_ref = compute_position::<NodeId, ()>(
        ElementOrVirtual::VirtualElement(Box::new(virt_node_in.clone())),
        &float_node_id,
        ComputePositionConfig::new(&platform)
            .placement(Placement::Bottom)
            .strategy(Strategy::Fixed)
            .middleware(vec![Box::new(Hide::new(
                HideOptions::default().strategy(HideStrategy::ReferenceHidden),
            ))]),
    );
    let hide_node_in_ref: HideData = res_node_in_ref
        .middleware_data
        .get_as(HIDE_NAME)
        .expect("HideData present");
    assert_eq!(hide_node_in_ref.reference_hidden, Some(false));

    let res_node_in_esc = compute_position::<NodeId, ()>(
        ElementOrVirtual::VirtualElement(Box::new(virt_node_in)),
        &float_node_id,
        ComputePositionConfig::new(&platform)
            .placement(Placement::Bottom)
            .strategy(Strategy::Fixed)
            .middleware(vec![Box::new(Hide::new(
                HideOptions::default().strategy(HideStrategy::Escaped),
            ))]),
    );
    let hide_node_in_esc: HideData = res_node_in_esc
        .middleware_data
        .get_as(HIDE_NAME)
        .expect("HideData present");
    assert_eq!(hide_node_in_esc.escaped, Some(false));

    // 2b. Platform<NodeId, ()> — off-viewport ReferenceHidden & Escaped
    let virt_node_off = DefaultVirtualElement::<NodeId>::new(Rc::new({
        let r = off_vp_rect.clone();
        move || r.clone()
    }));
    let res_node_off_ref = compute_position::<NodeId, ()>(
        ElementOrVirtual::VirtualElement(Box::new(virt_node_off.clone())),
        &float_node_id,
        ComputePositionConfig::new(&platform)
            .placement(Placement::Bottom)
            .strategy(Strategy::Fixed)
            .middleware(vec![Box::new(Hide::new(
                HideOptions::default().strategy(HideStrategy::ReferenceHidden),
            ))]),
    );
    let hide_node_off_ref: HideData = res_node_off_ref
        .middleware_data
        .get_as(HIDE_NAME)
        .expect("HideData present");
    assert_eq!(hide_node_off_ref.reference_hidden, Some(true));

    let res_node_off_esc = compute_position::<NodeId, ()>(
        ElementOrVirtual::VirtualElement(Box::new(virt_node_off)),
        &float_node_id,
        ComputePositionConfig::new(&platform)
            .placement(Placement::Bottom)
            .strategy(Strategy::Fixed)
            .middleware(vec![Box::new(Hide::new(
                HideOptions::default().strategy(HideStrategy::Escaped),
            ))]),
    );
    let hide_node_off_esc: HideData = res_node_off_esc
        .middleware_data
        .get_as(HIDE_NAME)
        .expect("HideData present");
    assert_eq!(hide_node_off_esc.escaped, Some(true));

    // 3. context_element: Some(...) still clips to its scroll container (200x200).
    //    Point at (350, 100) is inside the 800x600 viewport, but outside the 200x200 container -> hidden!
    let virt_str_ctx = DefaultVirtualElement::<String>::new(Rc::new({
        let r = in_vp_rect.clone();
        move || r.clone()
    }))
    .context_element("ctx-el".to_string());
    let res_str_ctx = compute_position::<String, ()>(
        ElementOrVirtual::VirtualElement(Box::new(virt_str_ctx)),
        &float_str,
        ComputePositionConfig::new(&platform)
            .placement(Placement::Bottom)
            .strategy(Strategy::Fixed)
            .middleware(vec![Box::new(Hide::new(
                HideOptions::default().strategy(HideStrategy::ReferenceHidden),
            ))]),
    );
    let hide_str_ctx: HideData = res_str_ctx
        .middleware_data
        .get_as(HIDE_NAME)
        .expect("HideData present");
    assert_eq!(
        hide_str_ctx.reference_hidden,
        Some(true),
        "context_element: Some(...) must still clip to enclosing scroll container on Platform<String>"
    );

    let virt_node_ctx = DefaultVirtualElement::<NodeId>::new(Rc::new(move || in_vp_rect.clone()))
        .context_element(ctx_node_id);
    let res_node_ctx = compute_position::<NodeId, ()>(
        ElementOrVirtual::VirtualElement(Box::new(virt_node_ctx)),
        &float_node_id,
        ComputePositionConfig::new(&platform)
            .placement(Placement::Bottom)
            .strategy(Strategy::Fixed)
            .middleware(vec![Box::new(Hide::new(
                HideOptions::default().strategy(HideStrategy::ReferenceHidden),
            ))]),
    );
    let hide_node_ctx: HideData = res_node_ctx
        .middleware_data
        .get_as(HIDE_NAME)
        .expect("HideData present");
    assert_eq!(
        hide_node_ctx.reference_hidden,
        Some(true),
        "context_element: Some(...) must still clip to enclosing scroll container on Platform<NodeId>"
    );
}
