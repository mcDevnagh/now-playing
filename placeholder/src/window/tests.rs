use super::*;

#[test]
fn same_size_fills_the_screen() {
    assert_eq!(
        placement((1920, 1080), (1920, 1080)),
        Rect::new(0, 0, 1920, 1080)
    );
    assert_eq!(
        placement((3840, 2160), (3840, 2160)),
        Rect::new(0, 0, 3840, 2160)
    );
}

#[test]
fn smaller_is_centred_at_its_own_size() {
    assert_eq!(
        placement((1920, 1080), (400, 300)),
        Rect::new(760, 390, 400, 300)
    );
    assert_eq!(
        placement((3840, 2160), (400, 300)),
        Rect::new(1720, 930, 400, 300)
    );
}

#[test]
fn larger_is_cropped_evenly() {
    assert_eq!(
        placement((1920, 1080), (3840, 2160)),
        Rect::new(-960, -540, 3840, 2160)
    );
    // Wider only: centred across, cropped at the sides.
    assert_eq!(
        placement((1920, 1080), (2000, 100)),
        Rect::new(-40, 490, 2000, 100)
    );
}

#[test]
fn odd_differences_round_down() {
    // One spare pixel: it goes right and below.
    assert_eq!(placement((101, 101), (100, 100)), Rect::new(0, 0, 100, 100));
    // One pixel too many: it's cropped left and above.
    assert_eq!(
        placement((100, 100), (101, 101)),
        Rect::new(-1, -1, 101, 101)
    );
}
