use std::path::PathBuf;

use tuner_cal::Conversion;
use tuner_core::BinDocument;
use tuner_xdf::{CategoryReferenceMode, NumericKind, RawValue, XdfDocument};

#[test]
#[ignore = "requires the local-only SCGa05 calibration BIN/XDF fixture"]
fn supplied_calibration_fixture_normalizes_without_mapping_errors() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let xdf = XdfDocument::load(root.join("../../Test bin and xdf/SCGa05_cal.xdf")).unwrap();
    let bin = BinDocument::load(root.join("../../Test bin and xdf/SCGa05_cal.bin")).unwrap();

    assert_eq!(xdf.parameters.len(), 2_915);
    assert_eq!(xdf.categories.len(), 57);
    assert_eq!(
        xdf.category_reference_mode,
        CategoryReferenceMode::OneBasedPosition
    );
    assert!(xdf
        .parameters
        .iter()
        .flat_map(|parameter| parameter.category_memberships.iter())
        .any(|membership| {
            membership.category_index == 57
                && membership.resolved_category_index == Some(56)
                && membership.category_name.is_some()
        }));
    assert!(!xdf
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "missing-category"));
    assert_eq!(
        xdf.parameters
            .iter()
            .map(|parameter| parameter.axes.len())
            .sum::<usize>(),
        8_745
    );
    assert_eq!(
        xdf.parameters
            .iter()
            .flat_map(|parameter| parameter.axes.iter())
            .filter(|axis| {
                axis.storage
                    .as_ref()
                    .is_some_and(|storage| storage.numeric_kind == NumericKind::Ieee754Binary32)
            })
            .count(),
        19
    );
    let axes = xdf
        .parameters
        .iter()
        .flat_map(|parameter| parameter.axes.iter())
        .collect::<Vec<_>>();
    assert_eq!(
        axes.iter()
            .filter(|axis| axis.storage.as_ref().is_some_and(|storage| {
                storage.raw_type_flags == 0x06
                    && storage.numeric_kind == NumericKind::Integer
                    && storage.column_major
            }))
            .count(),
        2_896
    );
    assert_eq!(
        axes.iter()
            .filter(|axis| axis.storage.as_ref().is_some_and(|storage| {
                storage.raw_type_flags == 0x10006
                    && storage.numeric_kind == NumericKind::Ieee754Binary32
                    && storage.column_major
            }))
            .count(),
        19
    );
    assert!(xdf
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "unsupported-storage"));

    let report = xdf.validate_against_document(&bin);
    assert!(report.is_valid(), "{} mapping issues", report.issue_count());
}

#[test]
#[ignore = "requires the local-only SCGa05 calibration BIN/XDF fixture"]
fn supplied_reverse_torque_table_raw_cells_match_bin_bytes_and_column_major_order() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let xdf = XdfDocument::load(root.join("../../Test bin and xdf/SCGa05_cal.xdf")).unwrap();
    let bin = BinDocument::load(root.join("../../Test bin and xdf/SCGa05_cal.bin")).unwrap();
    let parameter = xdf
        .parameters
        .iter()
        .find(|parameter| parameter.title == "Driver Pedal Torque Request (Reverse)")
        .expect("reverse torque table should be present");

    assert_eq!(parameter.layout.address, 0x189DC);
    assert_eq!(parameter.layout.element_width_bits, 16);
    assert_eq!(parameter.layout.row_stride_bits, 16);
    assert_eq!(parameter.layout.column_stride_bits, 192);
    assert_eq!(parameter.layout.storage.raw_type_flags, 0x06);
    assert!(parameter.layout.storage.column_major);

    // These are the first four physical columns in the BIN, expressed as the
    // table's logical rows. The values are intentionally asymmetric so a
    // transposition or endian mistake cannot pass unnoticed.
    let expected = [
        [
            0, 983, 1966, 5243, 8192, 12452, 16384, 20644, 25559, 30802, 32768, 32768,
        ],
        [
            0, 1638, 3277, 6226, 9175, 13107, 17039, 20972, 25887, 30802, 32768, 32768,
        ],
        [
            0, 1966, 3604, 6226, 8847, 12124, 15729, 19005, 23265, 27525, 32768, 32768,
        ],
        [
            0, 1966, 3604, 6226, 8847, 12124, 15729, 19005, 23593, 27853, 32768, 32768,
        ],
    ];
    for (column, values) in expected.iter().enumerate() {
        for (row, value) in values.iter().copied().enumerate() {
            assert_eq!(
                parameter.read_raw_cell(&bin, row, column).unwrap(),
                RawValue::Unsigned(value),
                "raw cell at row {row}, column {column}"
            );
        }
    }
}

#[test]
#[ignore = "requires the local-only SCGa05 calibration BIN/XDF fixture"]
fn supplied_setpoint_map_axes_match_declared_storage_and_conversion() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let xdf = XdfDocument::load(root.join("../../Test bin and xdf/SCGa05_cal.xdf")).unwrap();
    let bin = BinDocument::load(root.join("../../Test bin and xdf/SCGa05_cal.bin")).unwrap();
    let parameter = xdf
        .parameters
        .iter()
        .find(|parameter| parameter.title == "Setpoint map for port flap")
        .expect("setpoint map should be present");
    let x = parameter.axis_definition(0).unwrap();
    let y = parameter.axis_definition(1).unwrap();
    assert_eq!(
        (x.address, x.count, x.element_width_bits, x.stride_bits),
        (Some(0x1336), 10, 8, 8)
    );
    assert_eq!(
        (y.address, y.count, y.element_width_bits, y.stride_bits),
        (Some(0x12D58), 10, 16, 16)
    );
    for (axis_index, axis) in [(0, x), (1, y)] {
        let address = axis.address.unwrap();
        let width = axis.element_width_bits / 8;
        let conversion = Conversion::parse(axis.conversion.as_deref().unwrap_or("X")).unwrap();
        for index in 0..axis.count {
            let expected = bin
                .read_uint(
                    address + index * (axis.stride_bits as usize / 8),
                    width,
                    axis.endianness,
                )
                .unwrap();
            assert_eq!(
                parameter.read_raw_axis(&bin, axis_index, index).unwrap(),
                RawValue::Unsigned(expected)
            );
            assert_eq!(
                parameter
                    .read_engineering_axis(&bin, axis_index, index)
                    .unwrap(),
                conversion.evaluate(expected as f64).unwrap()
            );
        }
    }
}
