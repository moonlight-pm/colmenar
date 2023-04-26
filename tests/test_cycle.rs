#![allow(dead_code)]

mod cycle;

#[test]
fn test_deref() {
    // A model with a salient member should deref to its fields.
    // let account_state = cycle::models::AccountState {
    //     salient: cycle::models::AccountStateSalient {
    //         current: "Foo".to_string(),
    //     },
    // };
    // assert_eq!(account_state.current, "Foo");
}
