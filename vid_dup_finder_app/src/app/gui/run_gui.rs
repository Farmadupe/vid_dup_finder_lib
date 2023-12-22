use std::{cell::RefCell, rc::Rc};

use gdk4::ModifierType;
use gio::prelude::*;
use glib::clone;
use gtk4::{
    prelude::*,
    Application, ApplicationWindow, Box, Button, CheckButton, DebugFlags,
    Orientation::{Horizontal, Vertical},
    ToggleButton,
};
use gtk4::{EventControllerKey, ScrolledWindow};

use super::gui_state::GuiState;
use crate::app::*;

pub fn run_gui(thunks: Vec<ResolutionThunk>) -> Result<(), AppError> {
    if thunks.is_empty() {
        info!("No matches were found. The GUI will not start");
        return Ok(());
    }

    gtk4::init().map_err(|_e| AppError::GuiStartError)?;
    gtk4::set_debug_flags(DebugFlags::all());

    let state: Rc<RefCell<GuiState>> = Rc::new(RefCell::new(GuiState::new(thunks, false)));

    let application = gtk4::Application::builder()
        .application_id("org.hello.there")
        .build();

    application.connect_activate(move |app| application_connect_activate_callback(app, &state));

    application.run_with_args::<&str>(&[]);

    Ok(())
}

fn rerender_gui(
    state: &Rc<RefCell<GuiState>>,
    entries_box: &Box,
    window: &ApplicationWindow,
    idx_label: &gtk4::Label,
) {
    let state = state.borrow();

    while let Some(child) = entries_box.first_child() {
        entries_box.remove(&child);
    }

    idx_label.set_text(&format!(
        "duplicate {} / {}. {}",
        state.current_idx() + 1,
        state.idx_len(),
        state.current_distance()
    ));

    let new_interior = state.render();
    // new_interior.set_height_request(300);
    // new_interior.set_width_request(300);
    entries_box.append(&new_interior);

    window.present()
}

//The following callbacks are defined as their own functions because the body of a clone!() macro
//does not get autoindented by rustfmt and does not get autocompleted by rust-analyzer.
//
//SO they are moved outside to restore this functionality.
fn application_connect_activate_callback(app: &Application, state: &Rc<RefCell<GuiState>>) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("First GTK+ Program")
        .default_width(500)
        .default_height(500)
        .build();

    let idx_label = gtk4::Label::new(Some(""));

    let nav_and_entries = gtk4::Box::new(Vertical, 12);

    let entries_box = Box::new(Horizontal, 6);

    let prev_button = Button::with_label("prev");
    prev_button.connect_clicked(clone!(
        @strong state,
        @strong window,
        @strong entries_box,
        @strong idx_label
    => move |_| {
        state.borrow_mut().prev_thunk();
        rerender_gui(&state, &entries_box, &window, &idx_label);
    }));

    let next_button = Button::with_label("next");

    next_button.connect_clicked(clone!(
        @strong state,
        @strong window,
        @strong entries_box,
        @strong idx_label
    => move |_| {
        state.borrow_mut().next_thunk();
        rerender_gui(&state, &entries_box, &window, &idx_label);
    }));

    let whole_single_button = ToggleButton::with_label("View single");
    whole_single_button.connect_toggled(clone!(
        @strong state,
        @strong window,
        @strong entries_box,
        @strong idx_label
    => move |whole_single_button| {
        let new_single_selected = whole_single_button.is_active();
        state.borrow_mut().set_single_mode(new_single_selected);
        rerender_gui(&state, &entries_box, &window, &idx_label);
    }));
    let state_mode = state.borrow().get_single_mode();
    whole_single_button.set_active(state_mode);

    let native_res_button = ToggleButton::with_label("View in native res");
    native_res_button.connect_toggled(clone!(
        @strong state,
        @strong window,
        @strong entries_box,
        @strong idx_label
    => move |native_res_button| {
        let new_native_res = native_res_button.is_active();
        state.borrow_mut().set_native(new_native_res);
        rerender_gui(&state, &entries_box, &window, &idx_label);
    }));
    let state_mode = state.borrow().get_native();
    native_res_button.set_active(state_mode);

    let view_spatial_button = CheckButton::with_label("View hash");
    view_spatial_button.connect_toggled(clone!(
        @strong state,
        @strong window,
        @strong entries_box,
        @strong idx_label
    => move |view_spatial_button| {


        let new_view_spatial = view_spatial_button.is_active();
        state.borrow_mut().set_view_spatial(new_view_spatial);
        rerender_gui(&state, &entries_box, &window, &idx_label);
    }));

    let view_temporal_button = CheckButton::with_label("View reddened hash");
    view_temporal_button.connect_toggled(clone!(
        @strong state,
        @strong window,
        @strong entries_box,
        @strong idx_label
    => move |view_temporal_button| {
        let new_view_temporal = view_temporal_button.is_active();
        state.borrow_mut().set_view_reddened(new_view_temporal);
        rerender_gui(&state, &entries_box, &window, &idx_label);
    }));

    let view_rebuilt_button = CheckButton::with_label("View images rebuilt from hash");
    view_rebuilt_button.connect_toggled(clone!(
        @strong state,
        @strong window,
        @strong entries_box,
        @strong idx_label
    => move |view_rebuilt_button| {
        let new_view_rebuilt = view_rebuilt_button.is_active();
        state.borrow_mut().set_view_rebuilt(new_view_rebuilt);
        rerender_gui(&state, &entries_box, &window, &idx_label);
    }));

    let cropdetect_button = ToggleButton::with_label("cropdetect");

    cropdetect_button.connect_toggled(clone!(
        @strong state,
        @strong window,
        @strong entries_box,
        @strong idx_label
    => move |cropdetect_button| {
        let new_cropdetect = cropdetect_button.is_active();

        state.borrow_mut().set_view_cropdetect(new_cropdetect);
        rerender_gui(&state, &entries_box, &window, &idx_label);
    }));

    let up_button = Button::with_label("up");
    up_button.connect_clicked(clone!(
        @strong state,
        @strong window,
        @strong entries_box,
        @strong idx_label,

        @strong whole_single_button
    => move |_| {
        state.borrow_mut().decrement_thunk_entry();
        whole_single_button.set_active(true);
        rerender_gui(&state, &entries_box, &window, &idx_label);
    }));

    let down_button = Button::with_label("down");
    down_button.connect_clicked(clone!(
        @strong state,
        @strong window,
        @strong entries_box,
        @strong idx_label,

        @strong whole_single_button
    => move |_| {
        if whole_single_button.is_active() {
            state.borrow_mut().increment_thunk_entry();
        }
        whole_single_button.set_active(true);
        rerender_gui(&state, &entries_box, &window, &idx_label);
    }));

    let updown_box = Box::new(Vertical, 6);
    updown_box.append(&up_button);
    updown_box.append(&down_button);

    let cropdetect_box = Box::new(Vertical, 6);
    cropdetect_box.append(&cropdetect_button);

    let nav_box = Box::new(Horizontal, 6);
    nav_box.append(&prev_button);
    nav_box.append(&next_button);
    nav_box.append(&updown_box);
    nav_box.append(&whole_single_button);
    nav_box.append(&native_res_button);
    nav_box.append(&cropdetect_box);

    let spa_tempo_box = Box::new(Vertical, 4);
    spa_tempo_box.append(&view_spatial_button);
    spa_tempo_box.append(&view_temporal_button);
    spa_tempo_box.append(&view_rebuilt_button);

    nav_box.append(&spa_tempo_box);

    nav_box.append(&idx_label);

    nav_and_entries.append(&nav_box);
    nav_and_entries.append(&entries_box);

    let scroller = ScrolledWindow::builder().child(&nav_and_entries).build();

    window.set_child(Some(&scroller));

    //sender2.send(GuiMessage2::Hello).unwrap();

    //keyboard shortcuts!?
    let k_controller = gtk4::EventControllerKey::new();

    let cb = clone!(
        @strong window,
        @strong state,
        @strong entries_box,
        @strong idx_label,
        @strong whole_single_button,
        @strong cropdetect_button,
        @strong native_res_button,
        @strong up_button,
        @strong down_button
    => move |_controller: &EventControllerKey, keyval: gdk4::Key, keycode: u32, _modifier_state: ModifierType| {

        window_connect_key_press_event_callback(
            &window,
            keyval,
            keycode,
            &state,
            &entries_box,
            &idx_label,
            &whole_single_button,
            &cropdetect_button,
            &native_res_button,
            &up_button,
            &down_button
        )
    });

    k_controller.connect_key_pressed(cb);

    window.add_controller(k_controller);
    // window.connect_key_pressed(clone!(
    //     @strong state,
    //     @strong entries_box,
    //     @strong idx_label,
    //     @strong whole_single_button,
    //     @strong cropdetect_button,
    //     @strong native_res_button,
    //     @strong up_button,
    //     @strong down_button
    // => move |window, key| {

    //     window_connect_key_press_event_callback(
    //         window,
    //         key,
    //         &state,
    //         &entries_box,
    //         &idx_label,
    //         &whole_single_button,
    //         &cropdetect_button,
    //         &inside_out_cropdetect_button,
    //         &native_res_button,
    //         &up_button,
    //         &down_button
    //     )
    // }));

    //window.show_all();

    window.present();

    //worker_thread.join().unwrap();
}

#[allow(clippy::too_many_arguments)]
fn window_connect_key_press_event_callback(
    window: &ApplicationWindow,
    keyval: gdk4::Key,
    _keycode: u32,

    state: &Rc<RefCell<GuiState>>,
    entries_box: &Box,

    idx_label: &gtk4::Label,

    whole_single_button: &ToggleButton,
    cropdetect_button: &ToggleButton,
    native_res_button: &ToggleButton,
    up_button: &Button,
    down_button: &Button,
) -> glib::Propagation {
    use gdk4::Key;

    match keyval {
        Key::Right => {
            state.borrow_mut().next_thunk();
            whole_single_button.set_active(false);
        }
        Key::Left => {
            state.borrow_mut().prev_thunk();
            whole_single_button.set_active(false);
        }

        Key::Home => {
            if cropdetect_button.is_active() {
                cropdetect_button.set_active(false);
            } else {
                cropdetect_button.set_active(true);
            }
        }

        Key::Page_Down => {
            whole_single_button.set_active(!whole_single_button.is_active());
        }

        Key::Insert => {
            native_res_button.set_active(!native_res_button.is_active());
        }

        Key::KP_Subtract | Key::minus => {
            state.borrow_mut().zoom_out();
            native_res_button.set_active(false);
        }

        Key::KP_Add | Key::equal => {
            state.borrow_mut().zoom_in();
            native_res_button.set_active(false);
        }

        Key::KP_Divide => {
            state.borrow_mut().set_native(true);
        }

        Key::KP_Multiply => {
            state.borrow_mut().set_native(false);
        }

        Key::Up => {
            up_button.emit_clicked();
        }
        Key::Down => {
            down_button.emit_clicked();
        }

        Key::comma => {
            whole_single_button.set_active(false);
            state.borrow_mut().press_key(keyval);
        }
        _ => {
            state.borrow_mut().press_key(keyval);
        }
    }
    rerender_gui(state, entries_box, window, idx_label);

    glib::Propagation::Proceed
}
