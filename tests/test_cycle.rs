#![allow(dead_code)]

mod cycle;

// #[test]
// fn test_enum_serialization() {
//     let capability = cycle::models::Capability::ApiKeysManage;
//     let serialized = serde_json::to_string(&capability).unwrap();
//     assert_eq!(serialized, "\"api-keys-manage\"");
//     let capability = serde_json::from_str::<cycle::models::Capability>(&serialized).unwrap();
//     assert_eq!(capability, cycle::models::Capability::ApiKeysManage);
// }

#[tokio::test]
async fn test_resource_operation() {
    let api = cycle::Api::new(std::env::var("CYCLE_KEY").unwrap());
    let body = api.get_account().await.unwrap();
    println!("Body: {:?}", body);
    // cycle::resources::account::get_account().await.unwrap();
    // CYCLE_KEY=secret_f8MyA06omqOVnL9pegpWsaQqPnyz3zHApdR4WgIIjQeKIkSl2QSMk8TUVPLh
}
