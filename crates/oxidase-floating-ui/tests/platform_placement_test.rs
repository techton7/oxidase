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
                    )
                    .into(),
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
