use data_modelling_api::hello_modelling;

#[test]
fn test_hello_modelling() {
    let result = hello_modelling();
    assert_eq!(result, "Modelling Rust module initialized");
}
