use ndarray::prelude::*;
use rustdct::DctPlanner;

////////////////////////////////////////////////////////////////////
// 2D OPS

#[allow(dead_code)]
pub fn dct_2d(matrix: &Array2<f64>) -> Array2<f64> {
    let mut matrix = matrix.clone();

    //first check that the supplied matrix is square
    assert!(matrix.is_square());
    let (x_len, _y_len) = matrix.dim();

    //setup the DCT.....
    let mut planner = DctPlanner::new();
    let dct = planner.plan_dct2(x_len);

    //perform round 1 of the DCT (on rows):
    matrix.rows_mut().into_iter().for_each(|mut row| {
        dct.process_dct2(row.as_slice_mut().expect("unreachable"));
    });

    //now tranpose...
    matrix = transpose_2d(matrix);

    //perform round 1 of the DCT (on cols):
    matrix.rows_mut().into_iter().for_each(|mut row| {
        dct.process_dct2(row.as_slice_mut().expect("unreachable"));
    });

    //now tranpose...
    matrix = transpose_2d(matrix);

    //and finally, normalize (has no effect, but may be useful in the future if further processing is required.)
    // for val in matrix.iter_mut() {
    //     *val *= 4f64 / (x_len as f64 * y_len as f64);
    // }
    brute_force_normalize_2d(&mut matrix, -1.0, 1.0);

    matrix
}

#[allow(dead_code)]
pub fn idct_2d(matrix: &Array2<f64>) -> Array2<f64> {
    let mut matrix = matrix.clone();

    //first check that the supplied matrix is square
    assert!(matrix.is_square());
    let (x_len, _y_len) = matrix.dim();

    //setup the DCT.....
    let mut planner = DctPlanner::new();
    let dct = planner.plan_dct3(x_len);

    //perform round 1 of the DCT (on rows):
    matrix.rows_mut().into_iter().for_each(|mut row| {
        dct.process_dct3(row.as_slice_mut().expect("unreachable"));
    });

    //now tranpose...
    matrix = transpose_2d(matrix);

    //perform round 1 of the DCT (on cols):
    matrix.rows_mut().into_iter().for_each(|mut row| {
        dct.process_dct3(row.as_slice_mut().expect("unreachable"));
    });

    //now tranpose...
    matrix = transpose_2d(matrix);

    brute_force_normalize_2d(&mut matrix, 0.0, 255.0);

    matrix
}

//rustdct requires the data to have row major alignment (e.g a stride of {WIDTH, 1}), however
//ndarray's transposition tools transpose by changing stride to {1, WIDTH} instead of shuffling
//data in memory.
//This function tranposes by shuffling in memory.
fn transpose_2d(matrix: Array2<f64>) -> Array2<f64> {
    Array::from_shape_vec(
        matrix.raw_dim(),
        matrix.reversed_axes().iter().copied().collect(),
    )
    .expect("unreachable")
}

fn brute_force_normalize_2d(matrix: &mut Array2<f64>, new_min: f64, new_max: f64) {
    let new_range = new_max - new_min;
    //brute force normalize in the range -1..+1
    let (curr_max, curr_min) = matrix.iter().fold((-1e9f64, 1e9f64), |(max, min), curr| {
        (max.max(*curr), min.min(*curr))
    });

    let curr_range = curr_max - curr_min;
    let scaling_factor = new_range / curr_range;
    for val in matrix.iter_mut() {
        let new_val = *val * scaling_factor;
        *val = new_val;
    }
}

////////////////////////////////////////////////////////////////////
// 3D OPS

pub fn dct_3d(matrix: &Array3<f64>) -> Array3<f64> {
    let mut matrix = matrix.clone();
    //first check that the supplied matrix is cube
    let (x_len, y_len, z_len) = matrix.dim();
    assert!({ x_len == y_len && x_len == z_len });

    //setup the DCT.....
    let mut planner = DctPlanner::new();
    let dct = planner.plan_dct2(x_len);

    //round 1
    matrix.rows_mut().into_iter().for_each(|mut row| {
        dct.process_dct2(row.as_slice_mut().expect("unreachable"));
    });

    //round 2
    matrix = transpose_3d_this_way(&matrix);
    matrix.rows_mut().into_iter().for_each(|mut row| {
        dct.process_dct2(row.as_slice_mut().expect("unreachable"));
    });

    //round 3
    matrix = transpose_3d_that_way(&matrix);
    matrix.rows_mut().into_iter().for_each(|mut row| {
        dct.process_dct2(row.as_slice_mut().expect("unreachable"));
    });

    //restore orientation
    matrix = transpose_3d_that_way(&matrix);
    matrix = transpose_3d_this_way(&matrix);

    //normalize
    //brute_force_normalize_3d(&mut matrix, -1.0, 1.0);

    matrix
}

fn transpose_3d_this_way(matrix: &Array3<f64>) -> Array3<f64> {
    let mut transposed_view = matrix.view();
    transposed_view.swap_axes(2, 1);
    let transposed_matrix =
        Array::from_shape_vec(matrix.raw_dim(), transposed_view.iter().copied().collect())
            .expect("unreachable");

    transposed_matrix
}

fn transpose_3d_that_way(matrix: &Array3<f64>) -> Array3<f64> {
    let mut transposed_view = matrix.view();
    transposed_view.swap_axes(2, 0);
    let transposed_matrix =
        Array::from_shape_vec(matrix.raw_dim(), transposed_view.iter().copied().collect())
            .expect("unreachable");

    transposed_matrix
}
