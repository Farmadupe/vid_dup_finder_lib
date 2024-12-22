pub mod autocrop_frames;
pub mod darkest_frame;
pub mod frame_change;
mod utils;

#[cfg(test)]
mod test;

///////development tweakables
//proportion of all-white/all black pix needed in video before attemptign motion detect crop
//const MIN_SATURATED_PIX: f64 = 0.1;
