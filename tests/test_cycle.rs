#![allow(dead_code)]

mod cycle;

#[test]
fn test_enum_serialization() {
    let capability = cycle::models::Capability::ApiKeysManage;
    let serialized = serde_json::to_string(&capability).unwrap();
    assert_eq!(serialized, "\"api-keys-manage\"");
    let capability = serde_json::from_str::<cycle::models::Capability>(&serialized).unwrap();
    assert_eq!(capability, cycle::models::Capability::ApiKeysManage);
}
