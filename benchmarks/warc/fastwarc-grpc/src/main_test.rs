use super::*;

#[test]
fn recoverable_error_does_not_stop_counting_batch() {
    let response = pb::ParseWarcResponse {
        kind: Some(pb::parse_warc_response::Kind::Batch(pb::RecordBatch {
            items: vec![
                pb::ParseWarcResponse {
                    kind: Some(pb::parse_warc_response::Kind::RecordError(pb::RecordError {
                        recoverable: true,
                        message: "invalid HTTP header".into(),
                        ..Default::default()
                    })),
                },
                pb::ParseWarcResponse {
                    kind: Some(pb::parse_warc_response::Kind::RecordEnd(pb::RecordEnd { payload_length: 42 })),
                },
            ],
        })),
    };
    let (mut count, mut bytes) = (0, 0);
    count_records(response, &mut count, &mut bytes).unwrap();
    assert_eq!((count, bytes), (1, 42));
}

#[test]
fn fatal_error_stops_counting() {
    let response = pb::ParseWarcResponse {
        kind: Some(pb::parse_warc_response::Kind::RecordError(pb::RecordError {
            recoverable: false,
            message: "invalid WARC header".into(),
            ..Default::default()
        })),
    };
    let (mut count, mut bytes) = (0, 0);
    let error = count_records(response, &mut count, &mut bytes).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!((count, bytes), (0, 0));
}
