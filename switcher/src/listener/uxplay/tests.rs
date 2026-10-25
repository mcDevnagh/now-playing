use super::*;

fn edges(values: &[bool]) -> Vec<Option<bool>> {
    let mut edges = UxplayEdges::default();
    values.iter().map(|&open| edges.update(open)).collect()
}

#[test]
fn starts_and_stops() {
    assert_eq!(
        edges(&[false, true, false, true]),
        [None, Some(true), Some(false), Some(true)]
    );
}

/// Mirroring already on when the switcher connects isn't a start, but its
/// end is a stop.
#[test]
fn the_first_value_is_a_baseline() {
    assert_eq!(edges(&[true, false]), [None, Some(false)]);
    assert_eq!(edges(&[false]), [None]);
}

#[test]
fn repeats_are_not_edges() {
    assert_eq!(
        edges(&[false, false, true, true, false, false]),
        [None, None, Some(true), None, Some(false), None]
    );
}

#[test]
fn shown_without_a_launch_id() {
    let app = App::new(None, Some(3 << 32 | 0x0200_0000)).unwrap();
    assert_eq!(
        without_launch(app),
        App {
            gamescope: 3,
            launch: None,
        }
    );
}
