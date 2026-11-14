use almahrya_core::SessionId;

#[test]
fn session_id_round_trips_through_string() {
    let id = SessionId::new();
    assert_eq!(id.to_string().parse::<SessionId>().unwrap(), id);
}

#[test]
fn session_ids_are_unique() {
    assert_ne!(SessionId::new(), SessionId::new());
}
