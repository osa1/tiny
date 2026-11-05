use termbox_simple::Termbox;

fn output(tb: &mut Termbox) -> String {
    String::from_utf8(tb.take_test_output()).unwrap()
}

fn cells(tb: &Termbox) -> String {
    tb.get_front_buffer().cells.iter().map(|c| c.ch).collect()
}

#[test]
fn wide_cells_use_column_coordinates_and_overwrite_both_halves() {
    for overwrite_x in [1, 2] {
        let mut tb = Termbox::init_test(5, 1);
        tb.change_cell(0, 0, 'A', 0, 0);
        tb.change_cell(1, 0, '日', 0, 0);
        tb.change_cell(3, 0, 'B', 0, 0);
        tb.present();
        assert_eq!(cells(&tb), "A日 B ");
        assert!(output(&mut tb).contains("A日B"));

        tb.change_cell(overwrite_x, 0, 'X', 0, 0);
        tb.present();
        let emitted = output(&mut tb);
        if overwrite_x == 1 {
            assert_eq!(cells(&tb), "AX B ");
            assert!(emitted.contains("X "), "{emitted:?}");
        } else {
            assert_eq!(cells(&tb), "A XB ");
            assert!(emitted.contains(" X"), "{emitted:?}");
        }
    }
}

#[test]
fn wide_replacement_repaints_the_other_half_of_an_overlapping_glyph() {
    let mut tb = Termbox::init_test(6, 1);
    tb.change_cell(1, 0, 'a', 0, 0);
    tb.change_cell(2, 0, '日', 0, 0);
    tb.change_cell(4, 0, 'Z', 0, 0);
    tb.present();
    output(&mut tb);
    tb.change_cell(1, 0, '中', 0, 0);
    tb.present();
    assert_eq!(cells(&tb), " 中  Z ");
    let emitted = output(&mut tb);
    assert!(emitted.contains("中 "), "{emitted:?}");

    tb.change_cell(1, 0, 'a', 0, 0);
    tb.present();
    assert_eq!(cells(&tb), " a  Z ");
    assert!(output(&mut tb).contains("a "));
}

#[test]
fn wide_right_edge_and_resize_use_a_single_column_replacement() {
    let mut tb = Termbox::init_test(4, 1);
    tb.change_cell(0, 0, 'A', 0, 0);
    tb.change_cell(1, 0, '日', 0, 0);
    tb.change_cell(3, 0, '中', 0, 0);
    tb.present();
    assert_eq!(cells(&tb), "A日 �");
    assert!(output(&mut tb).contains("A日�"));

    tb.set_buffer_size(2, 1);
    tb.present();
    assert_eq!(cells(&tb), "A�");
    assert!(output(&mut tb).contains("A�"));
    tb.set_buffer_size(4, 1);
    tb.present();
    assert_eq!(cells(&tb), "A�  ");
    assert!(output(&mut tb).contains("A�"));
}

#[test]
fn combining_marks_are_emitted_updated_removed_and_kept_on_restyle() {
    let mut tb = Termbox::init_test(2, 1);
    tb.change_cell(1, 0, 'か', 0, 0);
    tb.append_combining(1, 0, '\u{3099}', 0, 0);
    tb.present();
    assert!(output(&mut tb).contains("�\u{3099}"));

    tb.change_cell(1, 0, 'か', 0, 0);
    tb.append_combining(1, 0, '\u{309a}', 0, 0);
    tb.present();
    assert!(output(&mut tb).contains("�\u{309a}"));

    tb.set_cell_attributes(1, 0, 1, 0);
    tb.present();
    let emitted = output(&mut tb);
    assert!(emitted.find('�').unwrap() < emitted.find('\u{309a}').unwrap());

    tb.change_cell(1, 0, 'か', 0, 0);
    tb.present();
    let emitted = output(&mut tb);
    assert!(emitted.contains('�'));
    assert!(!emitted.contains('\u{309a}'));

    tb.clear();
    tb.change_cell(0, 0, '\u{3099}', 0, 0);
    tb.present();
    assert!(output(&mut tb).contains("◌\u{3099}"));
}

#[test]
fn combining_character_style_is_written_after_the_base() {
    let mut tb = Termbox::init_test(4, 1);
    tb.change_cell(0, 0, 'か', 0, 0);
    tb.append_combining(0, 0, '\u{3099}', 1, 0);
    tb.present();
    let emitted = output(&mut tb);
    let base = emitted.find('か').unwrap();
    let mark = emitted.find('\u{3099}').unwrap();
    assert!(
        base < mark && emitted[base + 'か'.len_utf8()..mark].contains("\x1b["),
        "{emitted:?}"
    );
}
