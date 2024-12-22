fn main() {
    #[cfg(all(target_family = "unix", feature = "gui_slint"))]
    slint_build::compile("ui/main_window.slint").unwrap();
}
