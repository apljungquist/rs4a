use rs4a_vlt::responses::{parse_data, Device, Loan, NewLoan, ParseError};

#[test]
fn invalid_json_returns_invalid_json_error() {
    assert!(matches!(
        parse_data::<Vec<()>>(r#""#),
        Err(ParseError::InvalidJson(_))
    ));
}

#[test]
fn wrong_data_returns_schema_mismatch_error() {
    assert!(matches!(
        parse_data::<Vec<()>>(r#"{"success":true}"#),
        Err(ParseError::SchemaMismatch(_))
    ));
    assert!(matches!(
        parse_data::<Vec<()>>(r#"{"success":true, "data":null}"#),
        Err(ParseError::SchemaMismatch(_))
    ));
    assert!(matches!(
        parse_data::<Vec<()>>(r#"{"success":true, "data":{}}"#),
        Err(ParseError::SchemaMismatch(_))
    ));
}

#[test]
fn no_success_returns_remote_error() {
    assert!(matches!(
        parse_data::<Vec<()>>(r#"{"success":false}"#),
        Err(ParseError::Remote)
    ));
}

#[test]
fn can_deserialize_get_devices_responses() {
    parse_data::<Vec<Device>>(include_str!(
        "responses/get_devices_without_portcast_device.json"
    ))
    .unwrap();
    parse_data::<Vec<Device>>(include_str!(
        "responses/get_devices_with_portcast_device.json"
    ))
    .unwrap();
}

#[test]
fn can_deserialize_get_loans_responses() {
    parse_data::<Vec<Loan>>(include_str!("responses/get_loans_empty.json")).unwrap();
    parse_data::<Vec<Loan>>(include_str!("responses/get_loans_non_empty.json")).unwrap();
    parse_data::<Vec<Loan>>(include_str!("responses/get_loans_radar.json")).unwrap();
}

#[test]
fn can_deserialize_post_cancel_responses() {
    parse_data::<()>(include_str!("responses/post_cancel.json")).unwrap();
}

#[test]
fn can_deserialize_post_loans_responses() {
    parse_data::<NewLoan>(include_str!("responses/post_loans_hours.json")).unwrap();
    parse_data::<NewLoan>(include_str!("responses/post_loans_days.json")).unwrap();
}
