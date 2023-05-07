#![allow(dead_code)]

mod cycle;

use cycle::*;
use serde_json::json;

#[test]
fn test_enum_serialization() {
    let capability = Capability::ApiKeysManage;
    let serialized = serde_json::to_string(&capability).unwrap();
    assert_eq!(serialized, "\"api-keys-manage\"");
    let capability = serde_json::from_str::<Capability>(&serialized).unwrap();
    assert_eq!(capability, Capability::ApiKeysManage);
}

#[tokio::test]
async fn test_resource_operation() {
    let api = Api::new(
        std::env::var("CYCLE_KEY").unwrap(),
        std::env::var("CYCLE_HUB").unwrap(),
    );
    let request = CreateEnvironmentRequest::new(
        "test",
        "test",
        CreateEnvironmentRequestAbout::new("test"),
        Features::new(false),
        None,
    );
    assert_eq!(
        serde_json::to_value(&request).unwrap(),
        json!({
            "name": "test",
            "cluster": "test",
            "about": {
                "description": "test"
            },
            "features": {
                "legacy_networking": false
            },
            "stack": null,
        })
    );
    // api.create_environment(request).await.unwrap();
    match api.create_environment(request).await {
        Ok(_) => {}
        Err(e) => {
            println!("Error: {e:#?}");
        }
    }
    let body = api
        .get_environments(None, None, None, Some(vec!["test".to_string()]), None)
        .await
        .unwrap();
    assert_eq!(body.data.len(), 1);
    api.remove_environment(body.data[0].id.clone())
        .await
        .unwrap();
}
