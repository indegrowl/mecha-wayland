use gles::{Budget, Device, Error};

#[test]
fn the_device_opens_a_gles3_context_or_reports_there_is_none() {
    match Device::try_open(Budget::default()) {
        Ok(d) => assert!(
            d.version().starts_with("OpenGL ES 3."),
            "not a GLES 3 context: {}",
            d.version()
        ),
        Err(Error::NoDevice) => eprintln!("skip: no render node"),
        Err(e) => panic!("the device did not open: {e:?}"),
    }
}

#[test]
fn a_dropped_device_can_be_opened_again() {
    let Ok(first) = Device::try_open(Budget::default()) else {
        eprintln!("skip: no render node");
        return;
    };
    drop(first);
    let second = Device::try_open(Budget::default()).expect("open again");
    assert!(second.version().starts_with("OpenGL ES 3."));
}

#[test]
fn the_budget_defaults_to_four_and_two() {
    assert_eq!(
        Budget::default(),
        Budget {
            mono_pages: 4,
            color_pages: 2
        }
    );
}
