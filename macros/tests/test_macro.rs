use mqtt_typed_client_macros::mqtt_topic;

#[mqtt_topic("test/topic/{huy}")] 
struct TestStruct {
    huy: u32,
    payload: String,
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_macro_compiles() {

    }
} 